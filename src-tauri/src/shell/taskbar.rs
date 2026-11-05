//! Hiding the Windows taskbar while the dock runs, and always putting it back.
//!
//! Before the first change, the taskbar's original auto-hide state is written to a small file.
//! Restoring (on quit, on a panic, on sign-out) sets that state back, shows the taskbar again
//! and deletes the file. If the dock crashes, the file survives: the next launch keeps the
//! *original* state instead of saving our own auto-hide as if it were the user's.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};

use crate::{error::AppResult, settings::TaskbarMode};

const FILE_NAME: &str = "taskbar-restore.json";
/// `ABS_AUTOHIDE` from shellapi.h.
pub const AUTOHIDE: u32 = 0x1;

/// What the taskbar looked like before the dock changed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    /// `ABM_GETSTATE` result.
    original_state: u32,
}

/// The three things the dock does to the taskbar, so the logic can be tested without one.
pub trait TaskbarApi {
    fn state(&self) -> u32;
    fn set_state(&self, state: u32);
    /// Show or hide every taskbar (primary and secondary screens).
    fn set_visible(&self, visible: bool);
}

/// Hide the taskbar the way `mode` says, saving its original state first if it isn't saved.
pub fn apply(api: &impl TaskbarApi, file: &Path, mode: TaskbarMode) -> AppResult<()> {
    if read(file).is_none() {
        let saved = Saved {
            original_state: api.state(),
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Written whole or not at all: a half-written file would lose the original.
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&saved)?)?;
        std::fs::rename(&tmp, file)?;
    }
    api.set_state(api.state() | AUTOHIDE);
    api.set_visible(mode == TaskbarMode::AutoHide);
    Ok(())
}

/// Put the taskbar back as it was. Safe to call any number of times.
pub fn restore(api: &impl TaskbarApi, file: &Path) {
    if !file.exists() {
        return;
    }
    // An unreadable file still means the dock changed the taskbar: at least show it again.
    match read(file) {
        Some(saved) => api.set_state(saved.original_state),
        None => tracing::warn!("the taskbar restore file is unreadable; showing the taskbar"),
    }
    api.set_visible(true);
    if let Err(error) = std::fs::remove_file(file) {
        tracing::warn!(%error, "could not delete the taskbar restore file");
    }
}

fn read(file: &Path) -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

// ---------- The real taskbar ----------

/// The restore file's path, set once at startup so the panic hook can find it.
static FILE: OnceLock<PathBuf> = OnceLock::new();
/// One change at a time: quit, panic and settings can race.
static LOCK: Mutex<()> = Mutex::new(());

pub fn init(data_dir: &Path) {
    let _ = FILE.set(data_dir.join(FILE_NAME));
    install_panic_hook(restore_system);
}

/// A panic aborts in release builds; run `restore` first so the user isn't left without a
/// taskbar. The previous hook (the panic message) still runs after it.
fn install_panic_hook(restore: fn()) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}

pub fn apply_system(mode: TaskbarMode) {
    let Some(file) = FILE.get() else { return };
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(windows)]
    if let Err(error) = apply(&win::System, file, mode) {
        tracing::warn!(%error, "could not hide the Windows taskbar");
    }
    #[cfg(not(windows))]
    let _ = (file, mode);
}

pub fn restore_system() {
    let Some(file) = FILE.get() else { return };
    // A panic while holding the lock must still restore.
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(windows)]
    restore(&win::System, file);
    #[cfg(not(windows))]
    let _ = file;
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{w, BOOL},
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::{
                Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA},
                WindowsAndMessaging::{
                    EnumWindows, FindWindowW, GetClassNameW, ShowWindow, SW_HIDE, SW_SHOWNA,
                },
            },
        },
    };

    pub struct System;

    fn primary() -> Option<HWND> {
        // SAFETY: a plain lookup by class name.
        unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok()
    }

    /// The primary taskbar plus one per extra screen.
    fn all() -> Vec<HWND> {
        unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
            let mut class = [0u16; 32];
            // SAFETY: `class` is a local buffer of the given length.
            let len = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
            if String::from_utf16_lossy(&class[..len]) == "Shell_SecondaryTrayWnd" {
                // SAFETY: `list` is the `Vec` passed below, alive for the whole enumeration.
                unsafe { (*(list.0 as *mut Vec<HWND>)).push(hwnd) };
            }
            true.into()
        }
        let mut list: Vec<HWND> = primary().into_iter().collect();
        // SAFETY: the callback only pushes into `list`, which outlives the call.
        let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut list as *mut _ as isize)) };
        list
    }

    fn data(hwnd: HWND) -> APPBARDATA {
        APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..Default::default()
        }
    }

    impl super::TaskbarApi for System {
        fn state(&self) -> u32 {
            let Some(tray) = primary() else { return 0 };
            let mut data = data(tray);
            // SAFETY: a query with a correctly sized APPBARDATA.
            unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 }
        }

        fn set_state(&self, state: u32) {
            let Some(tray) = primary() else { return };
            let mut data = data(tray);
            data.lParam = LPARAM(state as isize);
            // SAFETY: the documented way to set the taskbar's auto-hide state.
            unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
        }

        fn set_visible(&self, visible: bool) {
            for tray in all() {
                // SAFETY: showing or hiding Explorer's own top-level taskbar windows.
                let _ = unsafe { ShowWindow(tray, if visible { SW_SHOWNA } else { SW_HIDE }) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct Fake {
        state: Cell<u32>,
        visible: Cell<bool>,
    }

    impl Fake {
        fn new(state: u32) -> Self {
            Self {
                state: Cell::new(state),
                visible: Cell::new(true),
            }
        }
    }

    impl TaskbarApi for Fake {
        fn state(&self) -> u32 {
            self.state.get()
        }
        fn set_state(&self, state: u32) {
            self.state.set(state);
        }
        fn set_visible(&self, visible: bool) {
            self.visible.set(visible);
        }
    }

    const ALWAYS_ON_TOP: u32 = 0x2;

    #[test]
    fn auto_hide_mode_auto_hides_and_keeps_it_visible() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(ALWAYS_ON_TOP);
        apply(&fake, &dir.path().join(FILE_NAME), TaskbarMode::AutoHide).unwrap();
        assert_eq!(fake.state.get(), ALWAYS_ON_TOP | AUTOHIDE);
        assert!(fake.visible.get());
    }

    #[test]
    fn hidden_mode_hides_it() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(0);
        apply(&fake, &dir.path().join(FILE_NAME), TaskbarMode::Hidden).unwrap();
        assert!(!fake.visible.get());
    }

    #[test]
    fn restore_puts_back_the_original_and_deletes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        let fake = Fake::new(ALWAYS_ON_TOP);
        apply(&fake, &file, TaskbarMode::Hidden).unwrap();
        restore(&fake, &file);
        assert_eq!(fake.state.get(), ALWAYS_ON_TOP);
        assert!(fake.visible.get());
        assert!(!file.exists());
    }

    #[test]
    fn switching_modes_never_overwrites_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        let fake = Fake::new(0);
        apply(&fake, &file, TaskbarMode::AutoHide).unwrap();
        apply(&fake, &file, TaskbarMode::Hidden).unwrap();
        apply(&fake, &file, TaskbarMode::AutoHide).unwrap();
        restore(&fake, &file);
        assert_eq!(fake.state.get(), 0);
    }

    #[test]
    fn after_a_crash_the_next_launch_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        let first = Fake::new(0);
        apply(&first, &file, TaskbarMode::AutoHide).unwrap();
        // Crash: no restore. Windows remembers auto-hide; the next launch sees it.
        let second = Fake::new(first.state.get());
        apply(&second, &file, TaskbarMode::AutoHide).unwrap();
        restore(&second, &file);
        assert_eq!(second.state.get(), 0);
    }

    #[test]
    fn an_unreadable_file_still_shows_the_taskbar() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        std::fs::write(&file, b"{\"origin").unwrap();
        let fake = Fake::new(AUTOHIDE);
        fake.visible.set(false);
        restore(&fake, &file);
        assert!(fake.visible.get());
        assert!(!file.exists());
    }

    #[test]
    fn saving_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        apply(&Fake::new(0), &file, TaskbarMode::Hidden).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE_NAME]);
    }

    #[test]
    fn a_panic_restores_before_the_process_dies() {
        static RESTORED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        install_panic_hook(|| RESTORED.store(true, std::sync::atomic::Ordering::SeqCst));
        let _ = std::panic::catch_unwind(|| panic!("boom"));
        // Put the default hook back for the other tests.
        let _ = std::panic::take_hook();
        assert!(RESTORED.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn restore_without_a_file_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(AUTOHIDE);
        fake.visible.set(false);
        restore(&fake, &dir.path().join(FILE_NAME));
        assert_eq!(fake.state.get(), AUTOHIDE);
        assert!(!fake.visible.get());
    }
}
