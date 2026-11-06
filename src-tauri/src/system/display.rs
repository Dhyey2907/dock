//! How Windows uses more than one screen: extend, duplicate, or one screen only (the Win+P
//! choices).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DisplayMode {
    /// PC screen only.
    PcOnly,
    /// The same picture on every screen.
    Duplicate,
    /// One big desktop across the screens.
    Extend,
    /// Second screen only.
    SecondOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DisplayState {
    /// `None` when Windows doesn't say (a single screen, or a custom layout).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<DisplayMode>,
    /// How many screens are connected right now.
    pub screens: u32,
}

/// Windows' topology flag for each mode (the same bits for `SetDisplayConfig`'s SDC_TOPOLOGY_*
/// and `QueryDisplayConfig`'s DISPLAYCONFIG_TOPOLOGY_*).
pub fn topology_flag(mode: DisplayMode) -> u32 {
    match mode {
        DisplayMode::PcOnly => 1,
        DisplayMode::Duplicate => 2,
        DisplayMode::Extend => 4,
        DisplayMode::SecondOnly => 8,
    }
}

pub fn mode_from_topology(flag: i32) -> Option<DisplayMode> {
    match flag {
        1 => Some(DisplayMode::PcOnly),
        2 => Some(DisplayMode::Duplicate),
        4 => Some(DisplayMode::Extend),
        8 => Some(DisplayMode::SecondOnly),
        _ => None,
    }
}

#[cfg(windows)]
pub fn read() -> DisplayState {
    use windows::Win32::Devices::Display::{
        GetDisplayConfigBufferSizes, QueryDisplayConfig, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_TOPOLOGY_ID, QDC_DATABASE_CURRENT,
    };
    let screens = super::brightness::screen_count();
    let (mut paths, mut modes) = (0u32, 0u32);
    // SAFETY: buffers sized by the first call, then filled by the second.
    let mode = unsafe {
        if GetDisplayConfigBufferSizes(QDC_DATABASE_CURRENT, &mut paths, &mut modes).is_err() {
            None
        } else {
            let mut path_info = vec![DISPLAYCONFIG_PATH_INFO::default(); paths as usize];
            let mut mode_info = vec![DISPLAYCONFIG_MODE_INFO::default(); modes as usize];
            let mut topology = DISPLAYCONFIG_TOPOLOGY_ID::default();
            let result = QueryDisplayConfig(
                QDC_DATABASE_CURRENT,
                &mut paths,
                path_info.as_mut_ptr(),
                &mut modes,
                mode_info.as_mut_ptr(),
                Some(&mut topology),
            );
            if result.is_ok() {
                mode_from_topology(topology.0)
            } else {
                None
            }
        }
    };
    DisplayState { mode, screens }
}

#[cfg(not(windows))]
pub fn read() -> DisplayState {
    DisplayState {
        mode: None,
        screens: 1,
    }
}

/// Switch to `mode`, as Win+P does.
#[cfg(windows)]
pub fn set(mode: DisplayMode) -> crate::error::AppResult<()> {
    use windows::Win32::Devices::Display::{SetDisplayConfig, SDC_APPLY, SET_DISPLAY_CONFIG_FLAGS};
    let flags = SDC_APPLY | SET_DISPLAY_CONFIG_FLAGS(topology_flag(mode));
    // SAFETY: a topology-only change; Windows supplies the paths itself.
    let result = unsafe { SetDisplayConfig(None, None, flags) };
    if result == 0 {
        Ok(())
    } else {
        Err(crate::error::AppError::Platform(format!(
            "display mode change failed ({result})"
        )))
    }
}

#[cfg(not(windows))]
pub fn set(_mode: DisplayMode) -> crate::error::AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_map_to_windows_topology_flags_and_back() {
        for mode in [
            DisplayMode::PcOnly,
            DisplayMode::Duplicate,
            DisplayMode::Extend,
            DisplayMode::SecondOnly,
        ] {
            assert_eq!(mode_from_topology(topology_flag(mode) as i32), Some(mode));
        }
        assert_eq!(topology_flag(DisplayMode::Extend), 4);
        assert_eq!(mode_from_topology(16), None);
        assert_eq!(mode_from_topology(0), None);
    }
}
