//! The productivity widgets: quick notes, to-dos, a kanban board, meeting notes, a time tracker,
//! quick links, app folders, browser bookmarks, file search, pinned pictures, text from the
//! screen, songs played today and Claude Code sessions. Plus the quick-capture shortcut.

pub mod bookmarks;
pub mod capture;
pub mod commands;
pub mod docs;
pub mod notes;
pub mod search;
pub mod service;
pub mod sessions;
pub mod snip;
pub mod songs;
pub mod store;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Shortcut that opens quick capture (a note or a to-do) from anywhere.
pub const CAPTURE_SHORTCUT: &str = "Ctrl+Alt+N";

/// Register the quick-capture shortcut. Called whenever the shortcuts are set again (setting
/// the show/hide shortcut clears them all).
pub fn register_capture_shortcut(app: &AppHandle) {
    let Ok(Some(shortcut)) = crate::shell::shortcut::parse(CAPTURE_SHORTCUT) else {
        return;
    };
    let result = app
        .global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let app = app.clone();
                // Off the main thread: the popup window may need creating.
                std::thread::spawn(move || {
                    if let Err(error) = crate::shell::panel::show_capture(&app) {
                        tracing::warn!(%error, "could not open quick capture");
                    }
                });
            }
        });
    if let Err(error) = result {
        tracing::warn!(%error, "quick capture shortcut not set");
    }
}
