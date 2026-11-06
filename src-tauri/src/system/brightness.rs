//! Brightness for external screens, over DDC/CI (the monitor's own control channel, VCP code
//! 0x10). A built-in laptop screen is set through Windows instead (the Status popup); screens
//! that don't speak DDC/CI are listed as unsupported rather than hidden.

use serde::Serialize;
use ts_rs::TS;

/// The VCP code for luminance.
#[cfg(windows)]
const VCP_BRIGHTNESS: u8 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorBrightness {
    /// Stable for as long as the screens stay connected the same way ("\\.\DISPLAY2#0").
    pub id: String,
    /// The monitor's own description, or "Screen 2".
    pub name: String,
    /// 0 to 100; `None` when the monitor doesn't support DDC/CI brightness.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    pub primary: bool,
}

/// A raw DDC value as a percent of the monitor's maximum.
pub fn to_percent(current: u32, maximum: u32) -> u8 {
    if maximum == 0 {
        return 0;
    }
    ((u64::from(current.min(maximum)) * 100 + u64::from(maximum) / 2) / u64::from(maximum)) as u8
}

/// A percent as the monitor's raw value.
pub fn from_percent(percent: u8, maximum: u32) -> u32 {
    ((u64::from(percent.min(100)) * u64::from(maximum) + 50) / 100) as u32
}

/// A friendlier name than Windows' placeholder.
pub fn monitor_name(description: &str, index: usize) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Generic PnP Monitor") {
        format!("Screen {}", index + 1)
    } else {
        trimmed.to_string()
    }
}

/// One physical monitor, opened. Closed when dropped.
#[cfg(windows)]
struct Physical {
    id: String,
    description: String,
    primary: bool,
    monitors: Vec<windows::Win32::Devices::Display::PHYSICAL_MONITOR>,
    index: usize,
}

#[cfg(windows)]
impl Physical {
    fn handle(&self) -> windows::Win32::Foundation::HANDLE {
        self.monitors[self.index].hPhysicalMonitor
    }
}

#[cfg(windows)]
impl Drop for Physical {
    fn drop(&mut self) {
        // Each group closes its handles once, through its first entry.
        if self.index == 0 {
            // SAFETY: handles from GetPhysicalMonitorsFromHMONITOR, destroyed exactly once.
            let _ = unsafe { windows::Win32::Devices::Display::DestroyPhysicalMonitors(&self.monitors) };
        }
    }
}

/// Every screen Windows shows a desktop on.
#[cfg(windows)]
fn screens() -> Vec<(windows::Win32::Graphics::Gdi::HMONITOR, String, bool)> {
    use windows::Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW},
    };
    unsafe extern "system" fn each(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        found: LPARAM,
    ) -> windows::core::BOOL {
        // SAFETY: `found` points at the Vec below, alive for the whole enumeration.
        unsafe {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
                let list = &mut *(found.0 as *mut Vec<(HMONITOR, String, bool)>);
                list.push((
                    monitor,
                    super::wide(&info.szDevice),
                    info.monitorInfo.dwFlags & 1 == 1,
                ));
            }
        }
        true.into()
    }
    let mut found: Vec<(HMONITOR, String, bool)> = Vec::new();
    // SAFETY: the callback writes only to `found`.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    found
}

#[cfg(windows)]
pub fn screen_count() -> u32 {
    screens().len() as u32
}

#[cfg(not(windows))]
pub fn screen_count() -> u32 {
    1
}

#[cfg(windows)]
fn physical_monitors() -> Vec<Physical> {
    use windows::Win32::Devices::Display::{
        GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
    };
    let mut all = Vec::new();
    for (monitor, device, primary) in screens() {
        let mut count = 0u32;
        // SAFETY: the array is sized from the count Windows reports.
        unsafe {
            if GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut count).is_err() || count == 0 {
                continue;
            }
            let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(monitor, &mut monitors).is_err() {
                continue;
            }
            for index in 0..monitors.len() {
                // A packed struct: copy the name out before reading it.
                let description = monitors[index].szPhysicalMonitorDescription;
                all.push(Physical {
                    id: format!("{device}#{index}"),
                    description: super::wide(&description),
                    primary,
                    monitors: monitors.clone(),
                    index,
                });
            }
        }
    }
    all
}

/// Current and maximum brightness, if the monitor answers.
#[cfg(windows)]
fn read_raw(monitor: &Physical) -> Option<(u32, u32)> {
    use windows::Win32::Devices::Display::GetVCPFeatureAndVCPFeatureReply;
    let (mut current, mut maximum) = (0u32, 0u32);
    // SAFETY: a DDC/CI query on an open physical monitor handle.
    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(
            monitor.handle(),
            VCP_BRIGHTNESS,
            None,
            &mut current,
            Some(&mut maximum),
        )
    };
    (ok != 0 && maximum > 0).then_some((current, maximum))
}

/// Every screen and its brightness. Slow (DDC/CI answers take tens of milliseconds a screen), so
/// call it off the main thread.
#[cfg(windows)]
pub fn list() -> Vec<MonitorBrightness> {
    physical_monitors()
        .iter()
        .enumerate()
        .map(|(i, monitor)| MonitorBrightness {
            id: monitor.id.clone(),
            name: monitor_name(&monitor.description, i),
            percent: read_raw(monitor).map(|(current, maximum)| to_percent(current, maximum)),
            primary: monitor.primary,
        })
        .collect()
}

#[cfg(not(windows))]
pub fn list() -> Vec<MonitorBrightness> {
    Vec::new()
}

/// Set one screen's brightness, 0 to 100.
#[cfg(windows)]
pub fn set(id: &str, percent: u8) -> crate::error::AppResult<()> {
    use crate::error::AppError;
    use windows::Win32::Devices::Display::SetVCPFeature;
    let monitors = physical_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| AppError::InvalidInput("that screen isn't connected".into()))?;
    let (_, maximum) = read_raw(monitor).ok_or_else(|| AppError::Platform("no DDC/CI brightness".into()))?;
    // SAFETY: a DDC/CI write on an open physical monitor handle.
    let ok = unsafe { SetVCPFeature(monitor.handle(), VCP_BRIGHTNESS, from_percent(percent, maximum)) };
    if ok == 0 {
        return Err(AppError::Platform("the screen didn't take the change".into()));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set(_id: &str, _percent: u8) -> crate::error::AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_raw_values_and_percent() {
        assert_eq!(to_percent(50, 100), 50);
        assert_eq!(to_percent(128, 255), 50);
        assert_eq!(to_percent(300, 100), 100);
        assert_eq!(to_percent(5, 0), 0);
        assert_eq!(from_percent(50, 255), 128);
        assert_eq!(from_percent(100, 100), 100);
        assert_eq!(from_percent(150, 100), 100);
        assert_eq!(from_percent(0, 80), 0);
    }

    #[test]
    fn names_placeholder_monitors_by_position() {
        assert_eq!(monitor_name("Generic PnP Monitor", 1), "Screen 2");
        assert_eq!(monitor_name("  ", 0), "Screen 1");
        assert_eq!(monitor_name("DELL U2720Q", 0), "DELL U2720Q");
    }
}
