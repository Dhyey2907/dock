//! Win+1 … Win+9 open the apps on the dock in order, like they do on the Windows taskbar.
//! Windows keeps those keys for its own taskbar, so they can't be registered as shortcuts;
//! a low-level keyboard hook sees them first instead. While the Windows taskbar is in use
//! (switched to from the dock), the keys go to Windows as usual.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

use tauri::AppHandle;

/// Whether Win+number opens dock apps (the "number shortcuts" setting).
static ENABLED: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

/// The app number for a key code, for the digit keys 1 to 9 (top row or keypad).
pub fn digit(vk: u32) -> Option<usize> {
    match vk {
        0x31..=0x39 => Some((vk - 0x30) as usize),
        0x61..=0x69 => Some((vk - 0x60) as usize),
        _ => None,
    }
}

/// Turn Win+number on or off; the hook is put in the first time it's turned on.
pub fn set_enabled(app: &AppHandle, on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
    if on && APP.set(app.clone()).is_ok() {
        #[cfg(windows)]
        std::thread::Builder::new()
            .name("win-number-keys".into())
            .spawn(win::run)
            .map(drop)
            .unwrap_or_else(|error| tracing::warn!(%error, "Win+number keys not set"));
    }
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::Ordering;

    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
            },
            WindowsAndMessaging::{
                CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
                WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
            },
        },
    };

    use super::{digit, APP, ENABLED};

    fn down(vk: VIRTUAL_KEY) -> bool {
        // SAFETY: reads the key state; no pointers involved.
        (unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000) != 0
    }

    /// A key nobody uses, pressed and let go, so letting go of Win doesn't open Start.
    fn mask_start_menu() {
        const UNASSIGNED: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);
        let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: UNASSIGNED,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
        // SAFETY: a local array of well-formed inputs.
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::SeqCst) {
            let message = wparam.0 as u32;
            // SAFETY: for WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT for this call.
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let injected = info.flags.0 & LLKHF_INJECTED.0 != 0;
            let pressed = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
            if pressed && !injected {
                if let (Some(n), Some(app)) = (digit(info.vkCode), APP.get()) {
                    let win = down(VK_LWIN) || down(VK_RWIN);
                    let others = down(VK_SHIFT) || down(VK_CONTROL) || down(VK_MENU);
                    if win && !others && !crate::shell::using_windows_taskbar(app) {
                        mask_start_menu();
                        let app = app.clone();
                        // Off the hook thread: the hook must return quickly.
                        std::thread::spawn(move || crate::apps::service::open_nth(&app, n));
                        return LRESULT(1);
                    }
                }
            }
        }
        // SAFETY: passing the event on, as every hook must.
        unsafe { CallNextHookEx(Some(HHOOK::default()), code, wparam, lparam) }
    }

    /// Put the hook in and pump messages for it (a low-level hook needs a message loop).
    pub fn run() {
        // SAFETY: installs a hook whose callback lives as long as the process; the loop below
        // keeps this thread alive and pumping.
        unsafe {
            if let Err(error) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) {
                tracing::warn!(%error, "Win+number keys not set");
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::digit;

    #[test]
    fn digits_one_to_nine_from_either_keyboard_row() {
        assert_eq!(digit(0x31), Some(1));
        assert_eq!(digit(0x39), Some(9));
        assert_eq!(digit(0x61), Some(1));
        assert_eq!(digit(0x30), None);
        assert_eq!(digit(0x41), None);
    }
}
