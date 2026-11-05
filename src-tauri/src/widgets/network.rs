//! The network you're on: its name, whether it's Wi-Fi, signal bars and whether it reaches the
//! internet. Windows reports every change (`NetworkStatusChanged`); nothing polls.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkState {
    pub connected: bool,
    /// Can reach the internet (not just the local network).
    pub internet: bool,
    pub wifi: bool,
    /// Wi-Fi network or connection name.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 0–5 bars, for Wi-Fi.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bars: Option<u8>,
}

impl NetworkState {
    pub fn offline() -> Self {
        Self {
            connected: false,
            internet: false,
            wifi: false,
            name: None,
            bars: None,
        }
    }
}

#[cfg(windows)]
pub fn read() -> NetworkState {
    use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return NetworkState::offline();
    };
    let level = profile
        .GetNetworkConnectivityLevel()
        .unwrap_or(NetworkConnectivityLevel::None);
    if level == NetworkConnectivityLevel::None {
        return NetworkState::offline();
    }
    let wifi = profile.IsWlanConnectionProfile().unwrap_or(false);
    let name = if wifi {
        profile
            .WlanConnectionProfileDetails()
            .and_then(|d| d.GetConnectedSsid())
            .ok()
            .map(|s| s.to_string())
    } else {
        None
    }
    .or_else(|| profile.ProfileName().ok().map(|s| s.to_string()))
    .filter(|n| !n.is_empty());
    let bars = if wifi {
        profile
            .GetSignalBars()
            .ok()
            .and_then(|b| b.Value().ok())
            .map(|b| b.min(5))
    } else {
        None
    };
    NetworkState {
        connected: true,
        internet: level == NetworkConnectivityLevel::InternetAccess,
        wifi,
        name,
        bars,
    }
}

#[cfg(not(windows))]
pub fn read() -> NetworkState {
    NetworkState::offline()
}

/// Call `changed` whenever the connection changes. The registration lasts for the app's life.
#[cfg(windows)]
pub fn watch(changed: impl Fn() + Send + Sync + 'static) {
    use windows::Networking::Connectivity::{NetworkInformation, NetworkStatusChangedEventHandler};
    let handler = NetworkStatusChangedEventHandler::new(move |_| {
        changed();
        Ok(())
    });
    if let Err(error) = NetworkInformation::NetworkStatusChanged(&handler) {
        tracing::warn!(%error, "could not watch the network");
    }
}

#[cfg(not(windows))]
pub fn watch(_changed: impl Fn() + Send + Sync + 'static) {}
