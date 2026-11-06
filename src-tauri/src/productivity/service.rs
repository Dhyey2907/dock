//! Keeps the productivity widgets' files and live data, and sends each change to every page.
//!
//! - Notes, to-dos, the board, the time log, links and folders: saved files, changed by commands.
//! - Songs played today: noted from the music widget's changes.
//! - Claude Code sessions: the transcript folder is looked at every 15 seconds, only while the
//!   widget is on.

use std::{path::Path, sync::Mutex, time::Duration};

use chrono::{Local, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    docs::{AppFolders, FolderApp, KanbanBoard, QuickLinks, TimeLog},
    notes::{apply_todo, delete_note, upsert_note, Note, Todo, TodoChange},
    sessions::{ClaudeActivity, Scanner},
    songs::SongsToday,
    store::JsonDoc,
};
use crate::{
    error::AppResult,
    media::MediaState,
    settings::{SettingsStore, WidgetKind},
};

pub const NOTES_CHANGED: &str = "notes-changed";
pub const TODOS_CHANGED: &str = "todos-changed";
pub const KANBAN_CHANGED: &str = "kanban-changed";
pub const TIMELOG_CHANGED: &str = "timelog-changed";
pub const LINKS_CHANGED: &str = "links-changed";
pub const FOLDERS_CHANGED: &str = "folders-changed";
pub const SONGS_CHANGED: &str = "songs-changed";
pub const CLAUDE_ACTIVITY: &str = "claude-activity";

const ACTIVITY_EVERY: Duration = Duration::from_secs(15);

pub struct ProductivityState {
    pub notes: JsonDoc<Vec<Note>>,
    pub todos: JsonDoc<Vec<Todo>>,
    pub kanban: JsonDoc<KanbanBoard>,
    pub timelog: JsonDoc<TimeLog>,
    pub links: JsonDoc<QuickLinks>,
    pub folders: JsonDoc<AppFolders>,
    pub songs: JsonDoc<SongsToday>,
    activity: Mutex<ClaudeActivity>,
    scanner: Mutex<Scanner>,
}

impl ProductivityState {
    pub fn open(dir: &Path) -> Self {
        Self {
            notes: JsonDoc::open(dir.join("notes.json")),
            todos: JsonDoc::open(dir.join("todos.json")),
            kanban: JsonDoc::open(dir.join("kanban.json")),
            timelog: JsonDoc::open(dir.join("time-tracker.json")),
            links: JsonDoc::open(dir.join("quick-links.json")),
            folders: JsonDoc::open(dir.join("app-folders.json")),
            songs: JsonDoc::open(dir.join("songs-today.json")),
            activity: Mutex::new(ClaudeActivity::default()),
            scanner: Mutex::new(Scanner::default()),
        }
    }
}

fn state(app: &AppHandle) -> tauri::State<'_, ProductivityState> {
    app.state::<ProductivityState>()
}

fn send<T: Serialize + Clone>(app: &AppHandle, event: &str, value: &T) {
    if let Err(error) = app.emit(event, value) {
        tracing::warn!(%error, event, "failed to send widget data");
    }
}

pub fn init(app: &AppHandle) -> AppResult<()> {
    let dir = app.path().app_data_dir()?;
    app.manage(ProductivityState::open(&dir));
    // Yesterday's songs go as soon as the dock starts.
    let songs = &state(app).songs;
    if songs.get().day != Some(Local::now().date_naive()) {
        songs.update(|s| s.roll_over(Local::now().date_naive()))?;
    }
    let handle = app.clone();
    std::thread::Builder::new()
        .name("claude-activity".into())
        .spawn(move || activity_loop(&handle))
        .ok();
    Ok(())
}

// ---------- Notes and to-dos ----------

/// Save a note's text (a new note when `id` is `None`); returns its id.
pub fn save_note(app: &AppHandle, id: Option<&str>, text: &str) -> AppResult<String> {
    let (notes, id) = state(app)
        .notes
        .update(|n| upsert_note(n, id, text, Utc::now()))?;
    send(app, NOTES_CHANGED, &notes);
    Ok(id)
}

pub fn remove_note(app: &AppHandle, id: &str) -> AppResult<()> {
    let (notes, ()) = state(app).notes.update(|n| delete_note(n, id))?;
    send(app, NOTES_CHANGED, &notes);
    Ok(())
}

pub fn change_todo(app: &AppHandle, change: TodoChange) -> AppResult<Vec<Todo>> {
    let (todos, ()) = state(app).todos.update(|t| apply_todo(t, change, Utc::now()))?;
    send(app, TODOS_CHANGED, &todos);
    Ok(todos)
}

// ---------- Whole documents ----------

pub fn set_kanban(app: &AppHandle, board: KanbanBoard) -> AppResult<KanbanBoard> {
    let board = state(app).kanban.set(board.normalized())?;
    send(app, KANBAN_CHANGED, &board);
    Ok(board)
}

pub fn set_timelog(app: &AppHandle, log: TimeLog) -> AppResult<TimeLog> {
    let log = state(app).timelog.set(log.normalized())?;
    send(app, TIMELOG_CHANGED, &log);
    Ok(log)
}

pub fn set_links(app: &AppHandle, links: QuickLinks) -> AppResult<QuickLinks> {
    let links = state(app).links.set(links.normalized())?;
    send(app, LINKS_CHANGED, &links);
    Ok(links)
}

pub fn set_folders(app: &AppHandle, folders: AppFolders) -> AppResult<AppFolders> {
    let folders = state(app).folders.set(folders.normalized())?;
    send(app, FOLDERS_CHANGED, &folders);
    Ok(folders)
}

// ---------- Songs played today ----------

/// The music widget's state changed.
pub fn music_changed(app: &AppHandle, music: Option<&MediaState>) {
    let Some(state) = app.try_state::<ProductivityState>() else {
        return;
    };
    let today = Local::now().date_naive();
    let mut changed = false;
    let result = state
        .songs
        .update(|s| changed = s.observe(music, today, Utc::now()));
    match result {
        Ok((songs, ())) if changed => send(app, SONGS_CHANGED, &songs),
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "could not save today's songs"),
    }
}

pub fn songs(app: &AppHandle) -> SongsToday {
    let mut songs = state(app).songs.get();
    songs.roll_over(Local::now().date_naive());
    songs
}

// ---------- Claude Code sessions ----------

fn claude_root() -> Option<std::path::PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(std::path::PathBuf::from(dir).join("projects"));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(std::path::PathBuf::from(home).join(".claude").join("projects"))
}

/// Look at the transcripts now, send the result if it changed, and return it.
pub fn refresh_activity(app: &AppHandle) -> ClaudeActivity {
    let state = state(app);
    let activity = match claude_root() {
        Some(root) => state
            .scanner
            .lock()
            .expect("scanner lock")
            .scan(&root, Utc::now()),
        None => ClaudeActivity::default(),
    };
    let mut current = state.activity.lock().expect("activity lock");
    if *current != activity {
        *current = activity.clone();
        drop(current);
        send(app, CLAUDE_ACTIVITY, &activity);
    }
    activity
}

fn activity_loop(app: &AppHandle) {
    loop {
        let settings = app.state::<SettingsStore>().get();
        if settings
            .widgets
            .iter()
            .any(|w| w.kind == WidgetKind::ClaudeSessions && w.enabled)
        {
            refresh_activity(app);
        }
        std::thread::sleep(ACTIVITY_EVERY);
    }
}

// ---------- Start menu apps (for app folders) ----------

/// Start menu shortcuts by name, sorted, one per name, leaving out uninstallers and help.
pub fn start_menu_entries(paths: Vec<std::path::PathBuf>) -> Vec<FolderApp> {
    let mut apps: Vec<FolderApp> = paths
        .into_iter()
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().trim().to_string();
            let lower = name.to_lowercase();
            let skip = ["uninstall", "readme", "help", "documentation", "website"]
                .iter()
                .any(|w| lower.contains(w));
            (!skip && !name.is_empty()).then(|| FolderApp {
                name,
                path: path.to_string_lossy().into_owned(),
            })
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    apps
}

fn shortcuts_in(dir: &Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > 4 {
        return;
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            shortcuts_in(&path, depth + 1, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            out.push(path);
        }
    }
}

/// Every app in the Start menu (yours and everyone's).
pub fn start_menu_apps() -> Vec<FolderApp> {
    let mut paths = Vec::new();
    for var in ["APPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            shortcuts_in(
                &std::path::PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"),
                0,
                &mut paths,
            );
        }
    }
    start_menu_entries(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_menu_lists_apps_once_by_name() {
        let apps = start_menu_entries(vec![
            r"C:\a\Zoom.lnk".into(),
            r"C:\b\paint.lnk".into(),
            r"C:\a\Paint.lnk".into(),
            r"C:\a\Uninstall Zoom.lnk".into(),
        ]);
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["paint", "Zoom"]);
    }
}
