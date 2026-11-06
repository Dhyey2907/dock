//! Listening to Windows for the dock's app features, each on a thread of its own with its own
//! message loop:
//! - a shell hook window (`RegisterShellHookWindow`) hears windows flashing for attention
//!   (`HSHELL_FLASH`) and answers where a minimizing window should shrink to
//!   (`HSHELL_GETMINRECT`): its app's icon on the dock;
//! - a low-level keyboard hook takes over Alt+Tab, only while the dock's own switcher is on.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex, OnceLock,
    },
};

use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VIRTUAL_KEY, VK_ESCAPE, VK_LMENU, VK_MENU, VK_RMENU, VK_SHIFT, VK_TAB,
            },
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
                PostThreadMessageW, RegisterClassW, RegisterShellHookWindow, RegisterWindowMessageW,
                SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN,
                LLKHF_INJECTED, MSG, SHELLHOOKINFO, WH_KEYBOARD_LL, WINDOW_EX_STYLE, WM_KEYDOWN, WM_KEYUP,
                WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
            },
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

// ---------- Alt+Tab ----------

/// What the keyboard did to the switcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchKey {
    /// Alt+Tab: open, or move to the next window.
    Next,
    /// Alt+Shift+Tab.
    Previous,
    /// Alt let go: switch to the chosen window. Carries the Alt key to release.
    Commit(u16),
    /// Esc.
    Cancel,
}

/// What a key event means while the hook is on. `active` is whether the switcher is open.
/// Returns the switcher's move and whether to swallow the key.
pub fn switch_key(vk: u16, down: bool, alt: bool, shift: bool, active: bool) -> (Option<SwitchKey>, bool) {
    let is_alt = [VK_MENU.0, VK_LMENU.0, VK_RMENU.0].contains(&vk);
    if vk == VK_TAB.0 && down && alt {
        let key = if shift {
            SwitchKey::Previous
        } else {
            SwitchKey::Next
        };
        return (Some(key), true);
    }
    if !active {
        return (None, false);
    }
    if vk == VK_ESCAPE.0 && down {
        return (Some(SwitchKey::Cancel), true);
    }
    if is_alt && !down {
        // Swallowed: the switcher releases Alt itself, after a harmless key (see
        // `win::release_alt_quietly`), so the app in front doesn't open its menu bar.
        return (Some(SwitchKey::Commit(vk)), true);
    }
    // Tab let go, or anything else typed while switching.
    (None, vk == VK_TAB.0)
}

static SWITCH_SINK: Mutex<Option<std::sync::mpsc::Sender<SwitchKey>>> = Mutex::new(None);
static SWITCH_ACTIVE: AtomicBool = AtomicBool::new(false);
/// The keyboard hook thread, while the hook is on (0 when off).
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_KEYBOARD_LL lParam points to a KBDLLHOOKSTRUCT for this call.
        let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if k.flags.0 & LLKHF_INJECTED.0 == 0 {
            let message = wparam.0 as u32;
            let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
            let up = message == WM_KEYUP || message == WM_SYSKEYUP;
            if down || up {
                let alt = k.flags.0 & LLKHF_ALTDOWN.0 != 0;
                // SAFETY: a plain key state query.
                let shift = unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } < 0;
                let active = SWITCH_ACTIVE.load(Ordering::SeqCst);
                let (key, swallow) = switch_key(k.vkCode as u16, down, alt, shift, active);
                if let Some(key) = key {
                    SWITCH_ACTIVE.store(
                        matches!(key, SwitchKey::Next | SwitchKey::Previous),
                        Ordering::SeqCst,
                    );
                    if let Some(sink) = SWITCH_SINK.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                        let _ = sink.send(key);
                    }
                }
                if swallow {
                    return LRESULT(1);
                }
            }
        }
    }
    // SAFETY: passing the event on to the next hook.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Take over Alt+Tab (`on`), sending its keys to `sink` on another thread; or give it back.
pub fn set_alt_tab(on: bool, sink: impl Fn(SwitchKey) + Send + 'static) {
    let running = HOOK_THREAD.load(Ordering::SeqCst);
    if !on {
        if running != 0 {
            HOOK_THREAD.store(0, Ordering::SeqCst);
            SWITCH_ACTIVE.store(false, Ordering::SeqCst);
            // SAFETY: asks our own hook thread to leave its message loop.
            let _ = unsafe { PostThreadMessageW(running, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        return;
    }
    if running != 0 {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<SwitchKey>();
    *SWITCH_SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    let _ = std::thread::Builder::new()
        .name("switcher-keys".into())
        .spawn(move || {
            while let Ok(key) = rx.recv() {
                sink(key);
            }
        });
    let _ = std::thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(|| {
            // SAFETY: a low-level hook installed and removed on this thread, which pumps messages
            // (low-level hooks are called through the installing thread's message loop).
            unsafe {
                HOOK_THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
                let instance = GetModuleHandleW(None).ok();
                let Ok(hook) =
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), instance.map(Into::into), 0)
                else {
                    tracing::warn!("could not take over Alt+Tab");
                    HOOK_THREAD.store(0, Ordering::SeqCst);
                    return;
                };
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(hook);
                *SWITCH_SINK.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
        });
}

/// The Alt key a commit carries, for releasing it.
pub fn alt_key(vk: u16) -> VIRTUAL_KEY {
    VIRTUAL_KEY(vk)
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

    #[test]
    fn alt_tab_keys() {
        let tab = VK_TAB.0;
        // Alt+Tab opens (or moves on) and is swallowed; plain Tab is left alone.
        assert_eq!(
            switch_key(tab, true, true, false, false),
            (Some(SwitchKey::Next), true)
        );
        assert_eq!(
            switch_key(tab, true, true, true, true),
            (Some(SwitchKey::Previous), true)
        );
        assert_eq!(switch_key(tab, true, false, false, false), (None, false));
        // While open: Tab-ups swallowed, Esc cancels, letting go of Alt commits.
        assert_eq!(switch_key(tab, false, true, false, true), (None, true));
        assert_eq!(
            switch_key(VK_ESCAPE.0, true, true, false, true),
            (Some(SwitchKey::Cancel), true)
        );
        assert_eq!(
            switch_key(VK_LMENU.0, false, false, false, true),
            (Some(SwitchKey::Commit(VK_LMENU.0)), true)
        );
        // Not switching: Alt and Esc pass through.
        assert_eq!(switch_key(VK_LMENU.0, false, false, false, false), (None, false));
        assert_eq!(switch_key(VK_ESCAPE.0, true, false, false, false), (None, false));
    }
}
