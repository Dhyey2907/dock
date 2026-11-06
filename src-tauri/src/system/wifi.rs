//! Wi-Fi details: the network's name, signal strength and link speed, this PC's address on it,
//! and (asked once, when the popup opens) the public address the internet sees.

use std::net::{IpAddr, UdpSocket};

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WifiDetails {
    pub connected: bool,
    pub wifi: bool,
    pub internet: bool,
    /// Network name (or the connection's name, when wired).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Signal quality, 0 to 100.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    /// Link speed in megabits a second.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_mbps: Option<u32>,
    /// This PC's address on the local network.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
}

/// The address this PC would use to reach the internet. Connecting a UDP socket sends nothing;
/// it only makes Windows pick the route.
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_unspecified()).then_some(address)
}

/// A public address answer, trimmed and checked to really be an address.
pub fn parse_public_ip(body: &str) -> Option<IpAddr> {
    body.trim().parse().ok()
}

/// Signal bars (0 to 5) as a rough percent, when Windows won't give the real figure.
pub fn bars_to_percent(bars: u8) -> u8 {
    (bars.min(5) * 20).min(100)
}

/// The connected Wi-Fi network's name, signal and link speed from the WLAN service.
#[cfg(windows)]
fn wlan() -> Option<(Option<String>, u8, u32)> {
    use windows::Win32::{
        Foundation::HANDLE,
        NetworkManagement::WiFi::{
            wlan_interface_state_connected, wlan_intf_opcode_current_connection, WlanCloseHandle,
            WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface,
            WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
        },
    };
    // SAFETY: WLAN API calls with out-pointers it fills; everything it allocates is freed with
    // WlanFreeMemory and the client handle is closed.
    unsafe {
        let (mut version, mut client) = (0u32, HANDLE::default());
        if WlanOpenHandle(2, None, &mut version, &mut client) != 0 {
            return None;
        }
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        let mut result = None;
        if WlanEnumInterfaces(client, None, &mut list) == 0 && !list.is_null() {
            let count = (*list).dwNumberOfItems as usize;
            let first = (*list).InterfaceInfo.as_ptr();
            for i in 0..count {
                let info = &*first.add(i);
                if info.isState != wlan_interface_state_connected {
                    continue;
                }
                let mut size = 0u32;
                let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
                let queried = WlanQueryInterface(
                    client,
                    &info.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    None,
                    &mut size,
                    &mut data,
                    None,
                );
                if queried == 0 && !data.is_null() {
                    let connection = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let association = &connection.wlanAssociationAttributes;
                    let ssid = &association.dot11Ssid;
                    let length = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
                    let name = String::from_utf8_lossy(&ssid.ucSSID[..length]).trim().to_string();
                    result = Some((
                        (!name.is_empty()).then_some(name),
                        association.wlanSignalQuality.min(100) as u8,
                        // Kilobits a second.
                        association.ulRxRate.max(association.ulTxRate) / 1000,
                    ));
                    WlanFreeMemory(data);
                    break;
                }
            }
            WlanFreeMemory(list as *const core::ffi::c_void);
        }
        WlanCloseHandle(client, None);
        result
    }
}

#[cfg(not(windows))]
fn wlan() -> Option<(Option<String>, u8, u32)> {
    None
}

/// Everything but the public address (that needs the internet; see `public_ip`).
pub fn read() -> WifiDetails {
    let network = crate::widgets::network::read();
    let mut details = WifiDetails {
        connected: network.connected,
        wifi: network.wifi,
        internet: network.internet,
        name: network.name.clone(),
        signal: network.bars.map(bars_to_percent),
        link_mbps: None,
        local_ip: None,
    };
    if network.connected {
        details.local_ip = local_ip().map(|ip| ip.to_string());
    }
    if network.wifi {
        if let Some((name, signal, mbps)) = wlan() {
            // Newer Windows hides the name from apps without location access; keep the one
            // the network status gave then.
            details.name = name.or(details.name);
            details.signal = Some(signal);
            details.link_mbps = (mbps > 0).then_some(mbps);
        }
    }
    details
}

/// The public address, from one request. `None` when offline (or the service is down).
pub async fn public_ip() -> Option<String> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("Dock (Windows desktop)")
        .build()
        .ok()?;
    let body = http
        .get("https://api.ipify.org")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    parse_public_ip(&body).map(|ip| ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_addresses_are_checked() {
        assert_eq!(
            parse_public_ip(" 203.0.113.7\n"),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            parse_public_ip("2001:db8::1"),
            Some("2001:db8::1".parse().unwrap())
        );
        assert_eq!(parse_public_ip("<html>rate limited</html>"), None);
        assert_eq!(parse_public_ip(""), None);
    }

    #[test]
    fn bars_become_a_rough_percent() {
        assert_eq!(bars_to_percent(0), 0);
        assert_eq!(bars_to_percent(3), 60);
        assert_eq!(bars_to_percent(5), 100);
        assert_eq!(bars_to_percent(9), 100);
    }
}
