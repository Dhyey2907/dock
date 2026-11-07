//! System changes the dock reacts to. Tauri doesn't surface these, so the dock's window
//! procedure is subclassed and the front window is watched with a WinEvent hook:
//! - display layout or work area changes, and app-bar position notices: re-place the dock,
//!   collapsing bursts until the layout settles;
//! - a drag of the dock ends: snap it to the nearer edge;
//! - fullscreen apps opening or closing: hide the dock while one covers its screen;
//! - Explorer restarting (it forgets app bars and re-shows the taskbar): set everything again;
//! - signing out or shutting down: put the taskbar back before the dock is killed.

use std::sync::{
    atomic::{AtomicBool, AtomicIsize, Ordering},
    Once, OnceLock,
};
use std::time::Duration;

use tauri::{AppHandle, WebviewWindow};
use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            Shell::{DefSubclassProc, SetWindowSubclass, ABN_FULLSCREENAPP, ABN_POSCHANGED},
            WindowsAndMessaging::{
                FindWindowW, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect,
                GetWindowThreadProcessId, IsZoomed, RegisterWindowMessageW, EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, GWL_STYLE, OBJID_WINDOW, SPI_SETDESKWALLPAPER,
                SPI_SETWORKAREA, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_DISPLAYCHANGE,
                WM_ENDSESSION, WM_EXITSIZEMOVE, WM_NCDESTROY, WM_SETTINGCHANGE, WS_CAPTION,
            },
        },
    },
};

use super::{appbar, taskbar};

static APP: OnceLock<AppHandle> = OnceLock::new();
/// `RegisterWindowMessage("TaskbarCreated")`: broadcast when Explorer (re)starts.
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

/// Run `work` once, `delay` after the first of a burst of calls guarded by `pending`.
fn coalesce(pending: &'static AtomicBool, delay: Duration, work: fn(&AppHandle)) {
    if pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(app) = APP.get().cloned() else {
        pending.store(false, Ordering::SeqCst);
        return;
    };
    // Off the window procedure: moving windows and emitting reach back into this thread.
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        pending.store(false, Ordering::SeqCst);
        work(&app);
    });
}

fn relayout_soon() {
    static PENDING: AtomicBool = AtomicBool::new(false);
    coalesce(&PENDING, Duration::from_millis(400), super::relayout);
}

fn fullscreen_soon() {
    static PENDING: AtomicBool = AtomicBool::new(false);
    coalesce(&PENDING, Duration::from_millis(150), refresh_fullscreen);
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        appbar::CALLBACK if wparam.0 == ABN_FULLSCREENAPP as usize => fullscreen_soon(),
        appbar::CALLBACK if wparam.0 == ABN_POSCHANGED as usize => relayout_soon(),
        WM_DISPLAYCHANGE => {
            relayout_soon();
            // A new resolution or primary screen changes the desktop picture too.
            if let Some(app) = APP.get() {
                crate::glass::service::refresh_soon(app);
            }
        }
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => relayout_soon(),
        WM_SETTINGCHANGE if wparam.0 == SPI_SETDESKWALLPAPER.0 as usize => {
            if let Some(app) = APP.get() {
                crate::glass::service::refresh_soon(app);
            }
        }
        WM_EXITSIZEMOVE => {
            if let Some(app) = APP.get().cloned() {
                let raw = hwnd.0 as isize;
                std::thread::spawn(move || {
                    let dragged = super::dock_windows(&app)
                        .into_iter()
                        .find(|w| w.hwnd().is_ok_and(|h| h.0 as isize == raw));
                    if let Some(window) = dragged {
                        super::snap_after_drag(&app, window.label());
                    }
                });
            }
        }
        // Sign-out or shutdown is going ahead: the dock is about to be killed.
        WM_ENDSESSION if wparam.0 != 0 => taskbar::restore_system(),
        WM_NCDESTROY => appbar::unregister(hwnd),
        _ if Some(&msg) == TASKBAR_CREATED.get() => {
            // A new Explorer process: watch its taskbar instead.
            watch_taskbar();
            if let Some(app) = APP.get().cloned() {
                std::thread::spawn(move || super::explorer_restarted(&app));
            }
        }
        _ => {}
    }
    // SAFETY: forwarding the unmodified message to the next window procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Start watching the system. Call once, on the main thread, before `attach`.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    // SAFETY: registering (or looking up) a named broadcast message.
    let _ = TASKBAR_CREATED.set(unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) });
    watch_front_window();
    watch_taskbar();
}

fn rehide_taskbar(app: &AppHandle) {
    use tauri::Manager;
    taskbar::rehide_system(app.state::<crate::settings::SettingsStore>().get().taskbar_mode);
}

/// In "Dock only" mode, Windows still shows its taskbar now and then (opening Start, some
/// notifications). Watch Explorer for its taskbar windows being shown and hide them again.
/// Only Explorer's own events are delivered, so this costs nothing the rest of the time.
fn watch_taskbar() {
    static HOOK: AtomicIsize = AtomicIsize::new(0);

    unsafe extern "system" fn on_show(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        id_object: i32,
        id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        if id_object != OBJID_WINDOW.0 || id_child != 0 {
            return;
        }
        let mut class = [0u16; 32];
        // SAFETY: `class` is a local buffer of the given length.
        let len = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd" {
            static PENDING: AtomicBool = AtomicBool::new(false);
            coalesce(&PENDING, Duration::from_millis(60), rehide_taskbar);
        }
    }

    // SAFETY: a lookup by class name, then a read-only query on the window found.
    let explorer = unsafe {
        let Ok(tray) = FindWindowW(w!("Shell_TrayWnd"), None) else {
            return;
        };
        let mut pid = 0;
        GetWindowThreadProcessId(tray, Some(&mut pid));
        pid
    };
    if explorer == 0 {
        return;
    }
    // SAFETY: an out-of-context hook with a `'static` callback, on the UI thread (the window
    // procedure or `install`), limited to Explorer's process.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_SHOW,
            None,
            Some(on_show),
            explorer,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.is_invalid() {
        tracing::warn!("could not watch the Windows taskbar; it may peek out in Dock only mode");
    }
    let old = HOOK.swap(hook.0 as isize, Ordering::SeqCst);
    if old != 0 {
        // SAFETY: a hook this function installed earlier, for an Explorer that has gone.
        let _ = unsafe { UnhookWinEvent(HWINEVENTHOOK(old as *mut _)) };
    }
}

/// Subclass a dock window and register it as an app bar. Call on the main thread (the thread
/// that owns the window), once per dock.
pub fn attach(window: &WebviewWindow) {
    let Ok(raw) = window.hwnd() else { return };
    let hwnd = HWND(raw.0);
    // SAFETY: called on the main thread that owns this live window; the callback only reads
    // statics and forwards every message unchanged.
    if !unsafe { SetWindowSubclass(hwnd, Some(subclass), 1, 0) }.as_bool() {
        tracing::warn!("could not watch for system changes; the dock may drift until restart");
    }
    if !appbar::register(hwnd) {
        tracing::warn!(
            label = window.label(),
            "could not register the dock as an app bar"
        );
    }
}

/// App-bar notices miss some fullscreen changes (F11 in a browser), so also re-check whenever
/// the front window changes or moves. Bursts (dragging a window) collapse into one check.
fn watch_front_window() {
    static INSTALL: Once = Once::new();

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        id_object: i32,
        id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        // Location changes fire for every window and caret; only the front window matters.
        if event == EVENT_OBJECT_LOCATIONCHANGE
            // SAFETY: a plain query with no arguments.
            && (id_object != OBJID_WINDOW.0 || id_child != 0 || hwnd != unsafe { GetForegroundWindow() })
        {
            return;
        }
        fullscreen_soon();
    }

    INSTALL.call_once(|| {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        for event in [EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_LOCATIONCHANGE] {
            // SAFETY: an out-of-context hook with a `'static` callback, installed on the UI
            // thread for the life of the app; the callback only reads statics.
            let hook = unsafe { SetWinEventHook(event, event, None, Some(on_event), 0, 0, flags) };
            if hook.is_invalid() {
                tracing::warn!(event, "could not watch the front window for fullscreen apps");
            }
        }
    });
}

/// Whether the front window fills a dock's screen (a video, a game, a browser in F11); each
/// dock hides only for a fullscreen app on its own screen.
pub fn refresh_fullscreen(app: &AppHandle) {
    // SAFETY: read-only queries on the foreground window; every out-parameter is a local of
    // the right type and size.
    let covered_monitor = unsafe {
        let front = GetForegroundWindow();
        let mut class = [0u16; 32];
        let len = GetClassNameW(front, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        let mut pid = 0;
        GetWindowThreadProcessId(front, Some(&mut pid));
        // The desktop fills the screen too, and a dock itself is never "fullscreen".
        let ignored =
            front.is_invalid() || pid == std::process::id() || class == "Progman" || class == "WorkerW";
        let monitor = MonitorFromWindow(front, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut rect = RECT::default();
        // With the taskbar and dock auto-hiding, an ordinary maximized window covers the
        // monitor too. Fullscreen apps (F11, video, games) have no title bar.
        let maximized_window = IsZoomed(front).as_bool()
            && (GetWindowLongW(front, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0;
        let covers = !ignored
            && !maximized_window
            && GetMonitorInfoW(monitor, &mut info).as_bool()
            && GetWindowRect(front, &mut rect).is_ok()
            && rect.left <= info.rcMonitor.left
            && rect.top <= info.rcMonitor.top
            && rect.right >= info.rcMonitor.right
            && rect.bottom >= info.rcMonitor.bottom;
        covers.then_some(monitor)
    };
    for window in super::dock_windows(app) {
        let Ok(raw) = window.hwnd() else { continue };
        // SAFETY: a read-only query on our own live dock window.
        let on_covered_screen = covered_monitor
            .is_some_and(|m| m == unsafe { MonitorFromWindow(HWND(raw.0), MONITOR_DEFAULTTONEAREST) });
        super::set_fullscreen(app, window.label(), on_covered_screen);
    }
}
