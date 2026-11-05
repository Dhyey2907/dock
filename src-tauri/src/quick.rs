//! Quick settings: Wi-Fi and Bluetooth on/off (the Windows Radios API) and the built-in
//! screen's brightness (WMI), for the status popup. Each part is `None` on a PC without it (no
//! Bluetooth radio, a desktop monitor that WMI can't dim). Night light, airplane mode and energy
//! saver have no public API; the popup opens Windows' own Quick Settings for those.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuickState {
    /// Wi-Fi radio on, if this PC has one.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi: Option<bool>,
    /// Bluetooth radio on, if this PC has one.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bluetooth: Option<bool>,
    /// Built-in screen brightness, 0 to 100, if Windows can set it.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RadioKind {
    Wifi,
    Bluetooth,
}

pub fn current() -> QuickState {
    #[cfg(windows)]
    {
        let radios = win::radios().unwrap_or_default();
        QuickState {
            wifi: radios.wifi,
            bluetooth: radios.bluetooth,
            brightness: win::brightness().ok(),
        }
    }
    #[cfg(not(windows))]
    QuickState::default()
}

pub fn set_radio(kind: RadioKind, on: bool) -> AppResult<()> {
    #[cfg(windows)]
    return win::set_radio(kind, on);
    #[cfg(not(windows))]
    {
        let _ = (kind, on);
        Err(AppError::Platform("no radios on this platform".into()))
    }
}

pub fn set_brightness(level: u8) -> AppResult<()> {
    if level > 100 {
        return Err(AppError::InvalidInput("brightness".into()));
    }
    #[cfg(windows)]
    return win::set_brightness(level);
    #[cfg(not(windows))]
    Err(AppError::Platform("no brightness on this platform".into()))
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{w, BSTR},
        Devices::Radios::{Radio, RadioAccessStatus, RadioKind as WinKind, RadioState},
        Win32::System::{
            Variant::VARIANT,
            Com::{
                CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Wmi::{
                IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY,
                WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
            },
        },
    };

    use super::{AppError, AppResult, QuickState, RadioKind};

    fn platform(what: &str) -> impl Fn(windows::core::Error) -> AppError + '_ {
        move |e| AppError::Platform(format!("{what}: {e}"))
    }

    pub fn radios() -> AppResult<QuickState> {
        let list = Radio::GetRadiosAsync()
            .and_then(|op| op.get())
            .map_err(platform("could not list radios"))?;
        let mut state = QuickState::default();
        for radio in list {
            let on = radio.State().map(|s| s == RadioState::On).unwrap_or(false);
            match radio.Kind() {
                // Several adapters of one kind: on if any is.
                Ok(WinKind::WiFi) => state.wifi = Some(state.wifi.unwrap_or(false) || on),
                Ok(WinKind::Bluetooth) => state.bluetooth = Some(state.bluetooth.unwrap_or(false) || on),
                _ => {}
            }
        }
        Ok(state)
    }

    pub fn set_radio(kind: RadioKind, on: bool) -> AppResult<()> {
        let access = Radio::RequestAccessAsync()
            .and_then(|op| op.get())
            .map_err(platform("could not ask for radio access"))?;
        if access != RadioAccessStatus::Allowed {
            return Err(AppError::Platform("Windows didn't allow changing radios".into()));
        }
        let want = match kind {
            RadioKind::Wifi => WinKind::WiFi,
            RadioKind::Bluetooth => WinKind::Bluetooth,
        };
        let list = Radio::GetRadiosAsync()
            .and_then(|op| op.get())
            .map_err(platform("could not list radios"))?;
        let mut found = false;
        for radio in list.into_iter().filter(|r| r.Kind().ok() == Some(want)) {
            found = true;
            let status = radio
                .SetStateAsync(if on { RadioState::On } else { RadioState::Off })
                .and_then(|op| op.get())
                .map_err(platform("could not switch the radio"))?;
            if status != RadioAccessStatus::Allowed {
                return Err(AppError::Platform("Windows didn't allow switching the radio".into()));
            }
        }
        if found {
            Ok(())
        } else {
            Err(AppError::InvalidInput("no such radio".into()))
        }
    }

    /// The WMI namespace that knows about laptop screens.
    fn connect() -> AppResult<IWbemServices> {
        crate::apps::win::com_init();
        // SAFETY: COM calls on objects owned here; COM is initialized on this thread above.
        unsafe {
            let locator: IWbemLocator =
                CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(platform("no WMI"))?;
            let services = locator
                .ConnectServer(
                    &BSTR::from("ROOT\\WMI"),
                    &BSTR::new(),
                    &BSTR::new(),
                    &BSTR::new(),
                    0,
                    &BSTR::new(),
                    None,
                )
                .map_err(platform("could not reach WMI"))?;
            // 10 and 0: RPC_C_AUTHN_WINNT and RPC_C_AUTHZ_NONE.
            CoSetProxyBlanket(
                &services,
                10,
                0,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
            .map_err(platform("could not set WMI security"))?;
            Ok(services)
        }
    }

    /// The first object `query` finds.
    fn first(services: &IWbemServices, query: &str) -> AppResult<IWbemClassObject> {
        // SAFETY: COM calls on objects owned here; `found` has room for the one object asked for.
        unsafe {
            let flags = WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0);
            let list = services
                .ExecQuery(&BSTR::from("WQL"), &BSTR::from(query), flags, None)
                .map_err(platform("WMI query failed"))?;
            let mut found = [None];
            let mut count = 0;
            list.Next(WBEM_INFINITE, &mut found, &mut count)
                .ok()
                .map_err(platform("WMI query failed"))?;
            found[0]
                .take()
                .ok_or_else(|| AppError::Platform("this screen's brightness can't be set".into()))
        }
    }

    pub fn brightness() -> AppResult<u8> {
        let services = connect()?;
        let screen = first(&services, "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active=TRUE")?;
        let mut value = VARIANT::default();
        // SAFETY: reading one property into a local VARIANT.
        unsafe { screen.Get(w!("CurrentBrightness"), 0, &mut value, None, None) }
            .map_err(platform("could not read brightness"))?;
        let level = i32::try_from(&value).map_err(platform("odd brightness value"))?;
        Ok(level.clamp(0, 100) as u8)
    }

    pub fn set_brightness(level: u8) -> AppResult<()> {
        let services = connect()?;
        let screen = first(&services, "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active=TRUE")?;
        // SAFETY: COM calls on objects owned here; every out-parameter is a local of the right type.
        unsafe {
            let mut path = VARIANT::default();
            screen
                .Get(w!("__PATH"), 0, &mut path, None, None)
                .map_err(platform("could not find the screen"))?;
            let path = BSTR::try_from(&path).map_err(platform("could not find the screen"))?;
            let mut class = None;
            services
                .GetObject(
                    &BSTR::from("WmiMonitorBrightnessMethods"),
                    WBEM_GENERIC_FLAG_TYPE(0),
                    None,
                    Some(&mut class),
                    None,
                )
                .map_err(platform("could not reach brightness"))?;
            let class: IWbemClassObject =
                class.ok_or_else(|| AppError::Platform("could not reach brightness".into()))?;
            let mut signature = None;
            class
                .GetMethod(w!("WmiSetBrightness"), 0, &mut signature, std::ptr::null_mut())
                .map_err(platform("could not reach brightness"))?;
            let signature: IWbemClassObject =
                signature.ok_or_else(|| AppError::Platform("could not reach brightness".into()))?;
            let args = signature.SpawnInstance(0).map_err(platform("could not set brightness"))?;
            args.Put(w!("Timeout"), 0, &VARIANT::from(1i32), 0)
                .map_err(platform("could not set brightness"))?;
            args.Put(w!("Brightness"), 0, &VARIANT::from(i32::from(level)), 0)
                .map_err(platform("could not set brightness"))?;
            services
                .ExecMethod(
                    &path,
                    &BSTR::from("WmiSetBrightness"),
                    WBEM_GENERIC_FLAG_TYPE(0),
                    None,
                    &args,
                    None,
                    None,
                )
                .map_err(platform("could not set brightness"))
        }
    }
}
