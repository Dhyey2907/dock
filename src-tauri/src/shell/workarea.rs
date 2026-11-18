//! Setting the dock screen's work area directly, for when the dock hides the taskbar on its own
//! screen only.
//!
//! Windows' taskbar auto-hide is one switch for every screen, so the dock leaves it off there
//! to keep the other screens' taskbars pinned. A hidden taskbar without auto-hide still holds its
//! strip, though, so maximized windows would stop short of the screen edge. Instead, the dock
//! sets that screen's work area itself: the whole screen minus the dock's own strip. Explorer
//! recomputes work areas now and then (another app bar, a setting); each time, the work-area
//! notice re-places the dock, which calls `hold` and puts it back.

use std::sync::Mutex;

use windows::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONULL},
    UI::WindowsAndMessaging::{SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETWORKAREA},
};

use super::placement::Rect;

/// The screen whose work area the dock holds, and its work area from before.
struct Held {
    screen: Rect,
    original: RECT,
}

static HELD: Mutex<Option<Held>> = Mutex::new(None);

fn to_win(rect: Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width as i32,
        bottom: rect.y + rect.height as i32,
    }
}

/// The work area Windows has now for the screen at `screen`.
fn current(screen: Rect) -> Option<RECT> {
    let rect = to_win(screen);
    // SAFETY: plain queries with a correctly sized, local info struct.
    unsafe {
        let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
        if monitor.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(monitor, &mut info).as_bool().then_some(info.rcWork)
    }
}

fn set(work: RECT) {
    let mut work = work;
    // SAFETY: SPI_SETWORKAREA reads one RECT; Windows applies it to the screen containing it.
    let result = unsafe {
        SystemParametersInfoW(SPI_SETWORKAREA, 0, Some(&mut work as *mut RECT as *mut _), SPIF_SENDCHANGE)
    };
    if let Err(error) = result {
        tracing::warn!(%error, "could not set the screen's work area");
    }
}

/// Make `work` the work area of `screen`, remembering what it was the first time. Does nothing
/// when it already is, so the work-area notice this sends doesn't start another round.
pub fn hold(screen: Rect, work: Rect) {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    if held.as_ref().is_some_and(|h| h.screen != screen) {
        // The dock moved to another screen: the old one gets its work area back first.
        if let Some(old) = held.take() {
            set(old.original);
        }
    }
    let Some(now) = current(screen) else { return };
    let work = to_win(work);
    if held.is_none() {
        *held = Some(Held { screen, original: now });
    }
    if now != work {
        tracing::debug!(?screen, "setting the dock screen's work area");
        set(work);
    }
}

/// Give the screen back the work area it had before `hold`. Safe to call any number of times.
pub fn release() {
    // From the panic hook, a panic inside `hold` may still hold the lock: skip rather than hang.
    let mut guard = if std::thread::panicking() {
        match HELD.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        }
    } else {
        HELD.lock().unwrap_or_else(|e| e.into_inner())
    };
    let held = guard.take();
    drop(guard);
    if let Some(held) = held {
        tracing::debug!(screen = ?held.screen, "giving the work area back");
        set(held.original);
    }
}
