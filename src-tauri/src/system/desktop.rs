//! Desktop icons on or off (Explorer's own "Show desktop icons" toggle), and turning the screens
//! off.

/// Explorer's menu command behind View, Show desktop icons. Sent to the desktop's view, it
/// toggles the icons.
#[cfg(windows)]
const TOGGLE_DESKTOP_ICONS: usize = 0x7402;

/// The desktop's view window (SHELLDLL_DefView) and its icon list (FolderView).
#[cfg(windows)]
fn desktop_view() -> Option<(windows::Win32::Foundation::HWND, windows::Win32::Foundation::HWND)> {
    use windows::{
        core::w,
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::{EnumWindows, FindWindowExW, FindWindowW},
        },
    };
    // SAFETY: window lookups by class name; the callback only writes through the pointer it is
    // given, which outlives `EnumWindows`.
    unsafe {
        let mut view = FindWindowW(w!("Progman"), None)
            .ok()
            .and_then(|progman| FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None).ok())
            .filter(|h| !h.is_invalid());
        if view.is_none() {
            // With a slideshow or some wallpaper apps the view lives under a WorkerW window.
            unsafe extern "system" fn find(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
                // SAFETY: `found` is the address of the `Option<HWND>` below.
                unsafe {
                    if let Ok(view) = FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) {
                        if !view.is_invalid() {
                            *(found.0 as *mut Option<HWND>) = Some(view);
                            return false.into();
                        }
                    }
                }
                true.into()
            }
            let mut found: Option<HWND> = None;
            let _ = EnumWindows(Some(find), LPARAM(&mut found as *mut Option<HWND> as isize));
            view = found;
        }
        let view = view?;
        let list = FindWindowExW(Some(view), None, w!("SysListView32"), None).ok()?;
        Some((view, list))
    }
}

/// Whether the desktop shows its icons; `None` if the desktop can't be found (Explorer isn't
/// running).
#[cfg(windows)]
pub fn icons_shown() -> Option<bool> {
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    let (_, list) = desktop_view()?;
    // SAFETY: a visibility query on a live window.
    Some(unsafe { IsWindowVisible(list) }.as_bool())
}

#[cfg(not(windows))]
pub fn icons_shown() -> Option<bool> {
    None
}

/// Show or hide the desktop icons. Returns what they are afterwards.
#[cfg(windows)]
pub fn set_icons_shown(show: bool) -> crate::error::AppResult<bool> {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{SendMessageW, WM_COMMAND},
    };
    let (view, _) = desktop_view()
        .ok_or_else(|| crate::error::AppError::Platform("the desktop isn't available".into()))?;
    if icons_shown() != Some(show) {
        // SAFETY: Explorer's documented-by-use toggle command, sent to its own view window.
        unsafe {
            SendMessageW(
                view,
                WM_COMMAND,
                Some(WPARAM(TOGGLE_DESKTOP_ICONS)),
                Some(LPARAM(0)),
            );
        }
    }
    Ok(icons_shown().unwrap_or(show))
}

#[cfg(not(windows))]
pub fn set_icons_shown(show: bool) -> crate::error::AppResult<bool> {
    Ok(show)
}

/// Turn the screens off (as the power button setting would). Moving the mouse or pressing a key
/// wakes them. A short wait first, so the click that asked doesn't wake them straight away.
#[cfg(windows)]
pub fn screens_off() {
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{SendMessageTimeoutW, SC_MONITORPOWER, SMTO_ABORTIFHUNG, WM_SYSCOMMAND},
    };
    std::thread::sleep(std::time::Duration::from_millis(600));
    // 2 = off. Broadcast with a timeout, so one hung window can't block the dock.
    const OFF: isize = 2;
    let broadcast = HWND(0xffff as *mut core::ffi::c_void);
    // SAFETY: a documented system command broadcast to top-level windows.
    unsafe {
        let _ = SendMessageTimeoutW(
            broadcast,
            WM_SYSCOMMAND,
            WPARAM(SC_MONITORPOWER as usize),
            LPARAM(OFF),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}

#[cfg(not(windows))]
pub fn screens_off() {}
