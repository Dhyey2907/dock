//! The dock as a Windows app bar: registered so Windows tells it about fullscreen apps and
//! layout changes, and (unless it auto-hides) claiming a strip of the screen so maximized
//! windows stop at the dock instead of sliding under it.

use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    UI::{
        Shell::{
            SHAppBarMessage, ABE_BOTTOM, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
        },
        WindowsAndMessaging::WM_APP,
    },
};

use super::placement::Rect;
use crate::settings::Edge;

/// The message Windows sends the dock for app-bar notices (`ABN_*` in wParam).
pub const CALLBACK: u32 = WM_APP + 0x42;

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: CALLBACK,
        ..Default::default()
    }
}

/// Register the window as an app bar. Returns false if Windows refused (or it already is one).
pub fn register(hwnd: HWND) -> bool {
    let mut data = data(hwnd);
    // SAFETY: registers this live window, owned by this process, for app-bar notices.
    unsafe { SHAppBarMessage(ABM_NEW, &mut data) != 0 }
}

pub fn unregister(hwnd: HWND) {
    let mut data = data(hwnd);
    // SAFETY: unregisters the app bar this window registered; harmless if it wasn't.
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut data) };
}

/// Claim `rect` along `edge`. An empty rect gives the space back but keeps the registration.
pub fn reserve(hwnd: HWND, rect: Rect, edge: Edge) {
    let mut data = data(hwnd);
    data.uEdge = match edge {
        Edge::Bottom => ABE_BOTTOM,
        Edge::Top => ABE_TOP,
    };
    data.rc = RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width as i32,
        bottom: rect.y + rect.height as i32,
    };
    let height = rect.height as i32;
    data.lParam = LPARAM(0);
    // SAFETY: the documented QUERYPOS/SETPOS handshake on our own registered app bar; `data`
    // is a correctly sized local that Windows adjusts in place.
    unsafe {
        SHAppBarMessage(ABM_QUERYPOS, &mut data);
        // Windows may have moved the proposed edge to make room for other bars; keep the
        // thickness we asked for, anchored to the edge it chose.
        match edge {
            Edge::Bottom => data.rc.top = data.rc.bottom - height,
            Edge::Top => data.rc.bottom = data.rc.top + height,
        }
        SHAppBarMessage(ABM_SETPOS, &mut data);
    }
}
