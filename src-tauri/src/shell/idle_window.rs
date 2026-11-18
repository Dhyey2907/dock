//! Popup windows that exist only while they're in use. A hidden window still keeps a browser
//! process of its own (about 20 MB), so each one is made when first needed and closed after it
//! has sat unused for a while.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::error::AppResult;

/// How long a hidden popup stays ready, so one used again soon still opens at once.
const KEEP_FOR: Duration = Duration::from_secs(120);
/// The longest a new popup stays off screen waiting for its page.
const PAGE_WAIT: Duration = Duration::from_millis(1500);

pub struct IdleWindow {
    label: &'static str,
    /// When it was last used. Held while the window is fetched, made or closed, so a popup being
    /// opened can't be closed underneath it.
    last_used: Mutex<Option<Instant>>,
    page_ready: AtomicBool,
    closer_running: AtomicBool,
}

/// How much longer a window last used at `last_used` should be kept.
fn time_left(last_used: Option<Instant>, now: Instant, keep_for: Duration) -> Duration {
    last_used.map_or(Duration::ZERO, |used| {
        keep_for.saturating_sub(now.saturating_duration_since(used))
    })
}

impl IdleWindow {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            last_used: Mutex::new(None),
            page_ready: AtomicBool::new(false),
            closer_running: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Instant>> {
        self.last_used.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The window, made with `create` if it doesn't exist. Never call this on the main thread:
    /// making a window there can deadlock WebView2.
    pub fn get_or_create(
        &self,
        app: &AppHandle,
        create: impl FnOnce(&AppHandle) -> AppResult<WebviewWindow>,
    ) -> AppResult<WebviewWindow> {
        let mut last_used = self.lock();
        *last_used = Some(Instant::now());
        if let Some(window) = app.get_webview_window(self.label) {
            return Ok(window);
        }
        self.page_ready.store(false, Ordering::SeqCst);
        create(app)
    }

    /// The window's page has loaded and asked what to show.
    pub fn page_loaded(&self) {
        self.page_ready.store(true, Ordering::SeqCst);
    }

    /// Wait for the page, so a new popup never appears empty.
    pub fn wait_for_page(&self) {
        let start = Instant::now();
        while !self.page_ready.load(Ordering::SeqCst) && start.elapsed() < PAGE_WAIT {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The popup was hidden: close its window once it has been unused for a while, unless
    /// `in_use` says it's showing again by then.
    pub fn close_when_idle(&'static self, app: &AppHandle, in_use: fn(&AppHandle) -> bool) {
        if app.get_webview_window(self.label).is_none() {
            return;
        }
        {
            let mut last_used = self.lock();
            *last_used = Some(Instant::now());
            if self.closer_running.swap(true, Ordering::SeqCst) {
                return;
            }
        }
        let app = app.clone();
        std::thread::spawn(move || loop {
            let wait = time_left(*self.lock(), Instant::now(), KEEP_FOR);
            if !wait.is_zero() {
                std::thread::sleep(wait);
                continue;
            }
            let last_used = self.lock();
            if !time_left(*last_used, Instant::now(), KEEP_FOR).is_zero() {
                continue;
            }
            if !in_use(&app) {
                self.close(&app);
            }
            self.closer_running.store(false, Ordering::SeqCst);
            drop(last_used);
            return;
        });
    }

    /// Close the window and wait until its label is free for the next one.
    fn close(&self, app: &AppHandle) {
        let Some(window) = app.get_webview_window(self.label) else {
            return;
        };
        if let Err(error) = window.destroy() {
            tracing::warn!(%error, label = self.label, "couldn't close an idle popup");
            return;
        }
        let start = Instant::now();
        while app.get_webview_window(self.label).is_some() && start.elapsed() < PAGE_WAIT {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_recently_used_window() {
        let used = Instant::now();
        let keep = Duration::from_secs(120);
        assert_eq!(time_left(Some(used), used, keep), keep);
        assert_eq!(
            time_left(Some(used), used + Duration::from_secs(20), keep),
            Duration::from_secs(100)
        );
    }

    #[test]
    fn nothing_left_once_idle_or_never_used() {
        let used = Instant::now();
        let keep = Duration::from_secs(120);
        assert!(time_left(Some(used), used + keep, keep).is_zero());
        assert!(time_left(Some(used), used + keep * 2, keep).is_zero());
        assert!(time_left(None, used, keep).is_zero());
    }

    #[test]
    fn a_loaded_page_needs_no_waiting() {
        let window = IdleWindow::new("test");
        window.page_loaded();
        let start = Instant::now();
        window.wait_for_page();
        assert!(start.elapsed() < Duration::from_millis(100));
    }
}
