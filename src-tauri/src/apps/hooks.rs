//! Listening to Windows for the dock's app features, each on a thread of its own with its own
//! message loop:
//! - a shell hook window (`RegisterShellHookWindow`) hears windows flashing for attention
//!   (`HSHELL_FLASH`) and answers where a minimizing window should shrink to
//!   (`HSHELL_GETMINRECT`): its app's icon on the dock.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, OnceLock,
    },
};

use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
            RegisterShellHookWindow, RegisterWindowMessageW, TranslateMessage, MSG, SHELLHOOKINFO,
            WINDOW_EX_STYLE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

use crate::shell::placement::Rect;

// Shell hook codes (WinUser.h); the flash and "rude" codes carry the high bit.
const HSHELL_WINDOWDESTROYED: usize = 2;
const HSHELL_WINDOWACTIVATED: usize = 4;
const HSHELL_GETMINRECT: usize = 5;
const HSHELL_FLASH: usize = 0x8006;
const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;

/// Windows flashing for attention now.
static FLASHING: Mutex<Option<HashSet<isize>>> = Mutex::new(None);
/// Where each window minimizes to (its app's icon on a dock), in screen pixels.
static MIN_RECTS: Mutex<Option<HashMap<isize, Rect>>> = Mutex::new(None);
static SHELL_CHANGED: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
static SHELL_MESSAGE: AtomicU32 = AtomicU32::new(0);

pub fn is_flashing(hwnd: isize) -> bool {
    FLASHING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|set| set.contains(&hwnd))
}

/// What a shell hook message changes in the flashing set: `Some(true)` starts, `Some(false)`
/// stops, `None` leaves it.
pub fn flash_change(code: usize) -> Option<bool> {
    match code {
        HSHELL_FLASH => Some(true),
        HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED | HSHELL_WINDOWDESTROYED => Some(false),
        _ => None,
    }
}

/// Replace where windows minimize to; empty turns the shrinking off (Windows' own applies).
pub fn set_min_rects(rects: HashMap<isize, Rect>) {
    *MIN_RECTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(rects);
}

fn min_rect(hwnd: isize) -> Option<RECT> {
    let rects = MIN_RECTS.lock().unwrap_or_else(|e| e.into_inner());
    let r = rects.as_ref()?.get(&hwnd)?;
    Some(RECT {
        left: r.x,
        top: r.y,
        right: r.x + r.width as i32,
        bottom: r.y + r.height as i32,
    })
}

unsafe extern "system" fn shell_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let shell = SHELL_MESSAGE.load(Ordering::SeqCst);
    if shell != 0 && msg == shell {
        let code = wparam.0;
        if code == HSHELL_GETMINRECT {
            // SAFETY: for this code lParam points to a SHELLHOOKINFO the system owns for the
            // duration of the (sent) message.
            let info = unsafe { &mut *(lparam.0 as *mut SHELLHOOKINFO) };
            if let Some(rect) = min_rect(info.hwnd.0 as isize) {
                info.rc = rect;
                return LRESULT(1);
            }
            return LRESULT(0);
        }
        if let Some(on) = flash_change(code) {
            let window = lparam.0;
            let changed = {
                let mut set = FLASHING.lock().unwrap_or_else(|e| e.into_inner());
                let set = set.get_or_insert_with(HashSet::new);
                if on {
                    set.insert(window)
                } else {
                    set.remove(&window)
                }
            };
            if changed {
                if let Some(changed) = SHELL_CHANGED.get() {
                    changed();
                }
            }
        }
        return LRESULT(0);
    }
    // SAFETY: default handling for every other message.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Start listening to the shell; `changed` is called when a window starts or stops flashing.
pub fn start_shell_hook(changed: impl Fn() + Send + Sync + 'static) {
    if SHELL_CHANGED.set(Box::new(changed)).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("shell-hook".into()).spawn(|| {
        // SAFETY: a hidden window of our own class, created and pumped on this thread for the
        // life of the app.
        unsafe {
            let Ok(instance) = GetModuleHandleW(None) else {
                return;
            };
            let class = WNDCLASSW {
                lpfnWndProc: Some(shell_proc),
                hInstance: instance.into(),
                lpszClassName: w!("DockShellHook"),
                ..Default::default()
            };
            RegisterClassW(&class);
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0),
                w!("DockShellHook"),
                w!("Dock shell hook"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            ) else {
                tracing::warn!("could not create the shell hook window");
                return;
            };
            SHELL_MESSAGE.store(RegisterWindowMessageW(w!("SHELLHOOK")), Ordering::SeqCst);
            if !RegisterShellHookWindow(hwnd).as_bool() {
                tracing::warn!("could not listen for flashing windows");
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_codes() {
        assert_eq!(flash_change(HSHELL_FLASH), Some(true));
        assert_eq!(flash_change(HSHELL_WINDOWACTIVATED), Some(false));
        assert_eq!(flash_change(HSHELL_RUDEAPPACTIVATED), Some(false));
        assert_eq!(flash_change(HSHELL_WINDOWDESTROYED), Some(false));
        assert_eq!(flash_change(1), None);
    }
}
