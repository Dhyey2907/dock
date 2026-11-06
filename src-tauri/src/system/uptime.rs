//! Time since the PC started, and whether Windows Update is waiting for a restart.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UptimeState {
    /// Seconds since Windows started.
    #[ts(type = "number")]
    pub seconds: u64,
    /// An update is installed and needs a restart to finish.
    pub restart_pending: bool,
}

/// Registry keys (under HKLM) that exist only while a restart is pending.
pub const RESTART_KEYS: [&str; 2] = [
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired",
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending",
];

/// A restart is pending when any of the keys exists.
pub fn restart_pending(exists: impl Fn(&str) -> bool) -> bool {
    RESTART_KEYS.iter().any(|key| exists(key))
}

#[cfg(windows)]
pub fn read() -> UptimeState {
    use windows::Win32::System::SystemInformation::GetTickCount64;
    // SAFETY: a plain query.
    let millis = unsafe { GetTickCount64() };
    UptimeState {
        seconds: millis / 1000,
        restart_pending: restart_pending(key_exists),
    }
}

#[cfg(windows)]
fn key_exists(path: &str) -> bool {
    use windows::{
        core::PCWSTR,
        Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY,
        },
    };
    let wide = super::to_wide(path);
    let mut key = HKEY::default();
    // SAFETY: opening a key read-only with a NUL-terminated path; closed straight away.
    unsafe {
        let opened = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide.as_ptr()),
            Some(0),
            KEY_READ | KEY_WOW64_64KEY,
            &mut key,
        );
        if opened.is_ok() {
            let _ = RegCloseKey(key);
            true
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
pub fn read() -> UptimeState {
    UptimeState {
        seconds: 0,
        restart_pending: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_restart_key_means_pending() {
        assert!(!restart_pending(|_| false));
        assert!(restart_pending(|k| k.ends_with("RebootRequired")));
        assert!(restart_pending(|k| k.ends_with("RebootPending")));
    }
}
