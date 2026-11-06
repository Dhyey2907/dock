//! The command bar: a centred glass window (Ctrl+Space) that finds apps, files, settings,
//! widgets, notes, to-dos and workspaces, and does sums. Ranking and the calculator live in the
//! page; this side lists what only Windows knows (installed apps, the search index) and reads
//! the notes and to-dos the notes widgets save.

#[cfg(windows)]
mod win;

use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;

use crate::{
    error::{AppError, AppResult},
    settings::{SettingsStore, Theme, WidgetKind},
};

pub const COMMAND_LABEL: &str = "command";
/// Event telling the command bar page it was just shown (clear and focus the box).
pub const COMMAND_SHOWN: &str = "command-shown";
/// Event asking the main dock to open a widget's popup.
pub const WIDGET_OPEN: &str = "widget-open";
/// Size of the command bar, logical pixels.
const SIZE: (f64, f64) = (660.0, 460.0);
/// Installed apps are listed again when the list is older than this.
const APPS_FRESH_FOR: Duration = Duration::from_secs(5 * 60);
/// Most files a search returns.
const MAX_FILES: usize = 12;

/// An app from the Start menu's list of everything installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledApp {
    /// Its app id (what `shell:AppsFolder` knows it by).
    pub id: String,
    pub name: String,
    /// Icon source for `app_icon`.
    pub icon: String,
}

/// A file or folder from the Windows search index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IndexHit {
    pub path: String,
    pub name: String,
    /// The folder it is in.
    pub folder: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteKind {
    Note,
    Todo,
}

/// A note or to-do from the notes widgets' files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteHit {
    pub kind: NoteKind,
    pub id: String,
    pub text: String,
    /// For to-dos.
    pub done: bool,
}

#[derive(Default)]
pub struct CommandState {
    apps: Mutex<Option<(Instant, Vec<InstalledApp>)>>,
    /// Paths the recent file searches returned: only these can be opened.
    files: Mutex<Vec<String>>,
}

/// A JSON id as text, whatever type the writer used.
fn text_of(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    done: bool,
}

/// Notes (`[{id,text,updated}]`) or to-dos (`[{id,text,done,created}]`) from `json`; anything
/// unreadable gives none.
pub fn parse_notes(json: &str, kind: NoteKind) -> Vec<NoteHit> {
    serde_json::from_str::<Vec<RawEntry>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !e.text.trim().is_empty())
        .map(|e| NoteHit {
            kind,
            id: text_of(e.id.as_ref()),
            text: e.text,
            done: kind == NoteKind::Todo && e.done,
        })
        .collect()
}

/// Every note and to-do saved in `dir`; missing files give none. Read only: the notes widgets
/// own these files.
pub fn read_notes(dir: &Path) -> Vec<NoteHit> {
    let read = |file: &str, kind| {
        std::fs::read_to_string(dir.join(file))
            .map(|json| parse_notes(&json, kind))
            .unwrap_or_default()
    };
    let mut hits = read("notes.json", NoteKind::Note);
    hits.extend(read("todos.json", NoteKind::Todo));
    hits
}

pub fn notes(app: &AppHandle) -> Vec<NoteHit> {
    match app.path().app_data_dir() {
        Ok(dir) => read_notes(&dir),
        Err(_) => Vec::new(),
    }
}

/// Every installed app, listed again when the list is a few minutes old.
pub fn apps(app: &AppHandle) -> Vec<InstalledApp> {
    let state = app.state::<CommandState>();
    if let Some((at, list)) = state.apps.lock().expect("command lock").as_ref() {
        if at.elapsed() < APPS_FRESH_FOR {
            return list.clone();
        }
    }
    #[cfg(windows)]
    let list = win::installed_apps();
    #[cfg(not(windows))]
    let list = Vec::new();
    *state.apps.lock().expect("command lock") = Some((Instant::now(), list.clone()));
    list
}

pub fn launch_app(app: &AppHandle, id: &str) -> AppResult<()> {
    if !apps(app).iter().any(|a| a.id == id) {
        return Err(AppError::InvalidInput("unknown app".into()));
    }
    hide(app)?;
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::AppId(id.to_string()));
    Ok(())
}

/// Files and folders in the search index matching `query`.
pub fn search_files(app: &AppHandle, query: &str) -> Vec<IndexHit> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Vec::new();
    }
    #[cfg(windows)]
    let hits = win::search_files(query, MAX_FILES);
    #[cfg(not(windows))]
    let hits: Vec<IndexHit> = Vec::new();
    let state = app.state::<CommandState>();
    let mut known = state.files.lock().expect("command lock");
    known.extend(hits.iter().map(|h| h.path.clone()));
    // Keep the list from growing without end.
    let excess = known.len().saturating_sub(200);
    known.drain(..excess);
    hits
}

pub fn open_file(app: &AppHandle, path: &str) -> AppResult<()> {
    let known = app
        .state::<CommandState>()
        .files
        .lock()
        .expect("command lock")
        .iter()
        .any(|p| p == path);
    if !known {
        return Err(AppError::InvalidInput("not a search result".into()));
    }
    hide(app)?;
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(path.to_string()));
    Ok(())
}

/// Open a widget's popup on the main dock, turning the widget on first if it is off.
pub fn open_widget(app: &AppHandle, kind: WidgetKind) -> AppResult<()> {
    if kind == WidgetKind::Unknown {
        return Err(AppError::InvalidInput("unknown widget".into()));
    }
    hide(app)?;
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    let mut changed = false;
    for entry in settings
        .widgets
        .iter_mut()
        .filter(|w| w.kind == kind && !w.enabled)
    {
        entry.enabled = true;
        changed = true;
    }
    if changed {
        store.set(settings.clone())?;
        app.emit(crate::commands::SETTINGS_CHANGED, &settings)?;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // A widget just turned on needs a moment to appear on the dock.
        std::thread::sleep(Duration::from_millis(if changed { 450 } else { 60 }));
        if let Err(error) = app.emit_to(crate::shell::DOCK_LABEL, WIDGET_OPEN, kind) {
            tracing::warn!(%error, "could not open the widget");
        }
    });
    Ok(())
}

pub fn hide(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(COMMAND_LABEL) {
        window.hide()?;
    }
    Ok(())
}

/// Show the command bar, or hide it if it is showing. Call off the main thread: the first
/// open creates the window.
pub fn toggle_window(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(COMMAND_LABEL) {
        if window.is_visible()? {
            return hide(app);
        }
    }
    show(app)
}

/// Show the command bar. Call off the main thread: the first open creates the window.
pub fn show(app: &AppHandle) -> AppResult<()> {
    let window = match app.get_webview_window(COMMAND_LABEL) {
        Some(window) => window,
        None => create(app)?,
    };
    let theme = app.state::<SettingsStore>().get().theme;
    window.set_theme(Some(if theme == Theme::White {
        tauri::Theme::Light
    } else {
        tauri::Theme::Dark
    }))?;
    window.set_size(tauri::LogicalSize::new(SIZE.0, SIZE.1))?;
    // Centred across the main dock's screen, a little above the middle.
    if let Some(geometry) = crate::shell::geometry_of(app, crate::shell::DOCK_LABEL) {
        let outer = window.outer_size()?;
        let x = geometry.screen_x + (geometry.screen_width as i32 - outer.width as i32) / 2;
        let y = geometry.screen_y + (geometry.screen_height as i32 - outer.height as i32) / 3;
        window.set_position(PhysicalPosition::new(x, y.max(geometry.screen_y)))?;
    } else {
        window.center()?;
    }
    app.emit_to(COMMAND_LABEL, COMMAND_SHOWN, ())?;
    window.show()?;
    #[cfg(windows)]
    crate::shell::make_tool_window(&window, false)?;
    window.set_focus()?;
    Ok(())
}

fn create(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, COMMAND_LABEL, WebviewUrl::App("index.html#command".into()))
        .title("Dock command bar")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .effects(tauri::utils::config::WindowEffectsConfig {
            effects: vec![tauri::window::Effect::Acrylic],
            ..Default::default()
        })
        .build()?;
    #[cfg(windows)]
    crate::shell::panel::round_corners(&window);
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_todos_are_read_whatever_their_id_types() {
        let notes = parse_notes(
            r#"[{"id":"a1","text":"Buy milk","updated":"2026-11-01T10:00:00Z"},{"id":2,"text":"  "}]"#,
            NoteKind::Note,
        );
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "a1");
        assert!(!notes[0].done);
        let todos = parse_notes(
            r#"[{"id":7,"text":"Ship it","done":true,"created":1730000000}]"#,
            NoteKind::Todo,
        );
        assert_eq!(todos[0].id, "7");
        assert!(todos[0].done);
        assert!(parse_notes("{broken", NoteKind::Todo).is_empty());
    }

    #[test]
    fn missing_files_give_no_notes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_notes(dir.path()).is_empty());
        std::fs::write(
            dir.path().join("todos.json"),
            r#"[{"id":"1","text":"Call","done":false}]"#,
        )
        .unwrap();
        let hits = read_notes(dir.path());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, NoteKind::Todo);
    }
}
