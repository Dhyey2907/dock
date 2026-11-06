//! What the productivity widgets' pages may call.

use serde::Deserialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{
    bookmarks::Bookmark,
    docs::{parse_target, AppFolders, FolderApp, KanbanBoard, QuickLinks, Target, TimeLog},
    notes::{Note, Todo, TodoChange},
    search::FileHit,
    service::{self, ProductivityState},
    sessions::ClaudeActivity,
    snip::{self, ScreenText, SnipBackdrop, SnipMode},
    songs::SongsToday,
};
use crate::error::{AppError, AppResult};

fn docs(app: &AppHandle) -> tauri::State<'_, ProductivityState> {
    app.state::<ProductivityState>()
}

// ---------- Notes and to-dos ----------

#[tauri::command]
pub fn notes_get(app: AppHandle) -> Vec<Note> {
    docs(&app).notes.get()
}

/// Save a note's text; a new note when `id` is null. Returns its id.
#[tauri::command]
pub fn note_save(app: AppHandle, id: Option<String>, text: String) -> AppResult<String> {
    service::save_note(&app, id.as_deref(), &text)
}

#[tauri::command]
pub fn note_delete(app: AppHandle, id: String) -> AppResult<()> {
    service::remove_note(&app, &id)
}

#[tauri::command]
pub fn todos_get(app: AppHandle) -> Vec<Todo> {
    docs(&app).todos.get()
}

#[tauri::command]
pub fn todo_change(app: AppHandle, change: TodoChange) -> AppResult<Vec<Todo>> {
    service::change_todo(&app, change)
}

/// What quick capture saves as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CaptureKind {
    Note,
    Todo,
}

/// Quick capture: save `text` as a new note or to-do, then close the popup.
#[tauri::command]
pub fn capture_save(app: AppHandle, kind: CaptureKind, text: String) -> AppResult<()> {
    if !text.trim().is_empty() {
        match kind {
            CaptureKind::Note => {
                service::save_note(&app, None, text.trim())?;
            }
            CaptureKind::Todo => {
                service::change_todo(&app, TodoChange::Add { text })?;
            }
        }
    }
    crate::shell::panel::close(&app)
}

// ---------- Whole documents ----------

#[tauri::command]
pub fn kanban_get(app: AppHandle) -> KanbanBoard {
    docs(&app).kanban.get()
}

#[tauri::command]
pub fn kanban_set(app: AppHandle, board: KanbanBoard) -> AppResult<KanbanBoard> {
    service::set_kanban(&app, board)
}

#[tauri::command]
pub fn timelog_get(app: AppHandle) -> TimeLog {
    docs(&app).timelog.get()
}

#[tauri::command]
pub fn timelog_set(app: AppHandle, log: TimeLog) -> AppResult<TimeLog> {
    service::set_timelog(&app, log)
}

#[tauri::command]
pub fn links_get(app: AppHandle) -> QuickLinks {
    docs(&app).links.get()
}

#[tauri::command]
pub fn links_set(app: AppHandle, links: QuickLinks) -> AppResult<QuickLinks> {
    service::set_links(&app, links)
}

#[tauri::command]
pub fn folders_get(app: AppHandle) -> AppFolders {
    docs(&app).folders.get()
}

#[tauri::command]
pub fn folders_set(app: AppHandle, folders: AppFolders) -> AppResult<AppFolders> {
    service::set_folders(&app, folders)
}

// ---------- Opening things ----------

fn open_target(target: &str) -> AppResult<()> {
    let target = match parse_target(target) {
        Some(Target::Web(url)) => url,
        Some(Target::Path(path)) if std::path::Path::new(&path).exists() => path,
        Some(Target::Path(_)) => {
            return Err(AppError::InvalidInput("That folder or file isn't there.".into()))
        }
        None => {
            return Err(AppError::InvalidInput(
                "That isn't a web address or a folder.".into(),
            ))
        }
    };
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(target));
    #[cfg(not(windows))]
    let _ = target;
    Ok(())
}

/// Open a quick link or bookmark: a web address or a folder or file on this PC.
#[tauri::command(async)]
pub fn link_open(app: AppHandle, target: String) -> AppResult<()> {
    open_target(&target)?;
    crate::shell::panel::close(&app)
}

/// Start an app from an app folder.
#[tauri::command(async)]
pub fn folder_app_launch(app: AppHandle, path: String) -> AppResult<()> {
    if !super::docs::launchable_path(&path) || !std::path::Path::new(&path).exists() {
        return Err(AppError::InvalidInput("That app isn't there any more.".into()));
    }
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Shortcut(path));
    crate::shell::panel::close(&app)
}

/// Apps to choose from for a folder. Async: walks the Start menu folders.
#[tauri::command(async)]
pub fn start_menu_apps() -> Vec<FolderApp> {
    service::start_menu_apps()
}

/// Chrome and Edge bookmarks. Async: reads the browsers' files.
#[tauri::command(async)]
pub fn bookmarks_get() -> Vec<Bookmark> {
    super::bookmarks::read_all()
}

// ---------- File search ----------

/// Files and folders matching `query`, from the Windows Search index.
#[tauri::command(async)]
pub fn file_search(query: String) -> AppResult<Vec<FileHit>> {
    #[cfg(windows)]
    {
        super::search::search(&query, 30)
    }
    #[cfg(not(windows))]
    {
        let _ = query;
        Ok(Vec::new())
    }
}

/// Open a found file, or show it in its folder.
#[tauri::command(async)]
pub fn file_open(app: AppHandle, path: String, reveal: bool) -> AppResult<()> {
    if !std::path::Path::new(&path).exists() {
        return Err(AppError::InvalidInput("That file isn't there any more.".into()));
    }
    if reveal {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new("explorer.exe")
                .raw_arg(format!("/select,\"{}\"", path.replace('"', "")))
                .spawn()?;
        }
    } else {
        #[cfg(windows)]
        crate::apps::win::launch(&crate::apps::Launch::Program(path));
    }
    crate::shell::panel::close(&app)
}

// ---------- Songs and Claude sessions ----------

#[tauri::command]
pub fn songs_get(app: AppHandle) -> SongsToday {
    service::songs(&app)
}

/// Claude Code sessions, looked at now. Async: reads the transcript folder.
#[tauri::command(async)]
pub fn claude_sessions_get(app: AppHandle) -> ClaudeActivity {
    service::refresh_activity(&app)
}

// ---------- Screen areas, pins and text ----------

/// Choose an area of the screen to pin or read. Async: captures the screen and makes a window.
#[tauri::command(async)]
pub fn snip_start(app: AppHandle, mode: SnipMode) -> AppResult<()> {
    snip::start(&app, mode)
}

/// The captured screen, for the area picker. Async: encoding takes a moment.
#[tauri::command(async)]
pub fn snip_backdrop(app: AppHandle) -> Option<SnipBackdrop> {
    snip::backdrop(&app)
}

/// The chosen area, as fractions of the picker. Async: reading text takes a moment.
#[tauri::command(async)]
pub fn snip_finish(app: AppHandle, x: f64, y: f64, width: f64, height: f64) -> AppResult<()> {
    snip::finish(&app, x, y, width, height)
}

#[tauri::command(async)]
pub fn snip_cancel(app: AppHandle) -> AppResult<()> {
    snip::cancel(&app)
}

/// Pin the clipboard's picture; false when it holds none.
#[tauri::command(async)]
pub fn pin_clipboard(app: AppHandle) -> AppResult<bool> {
    snip::pin_clipboard(&app)
}

#[tauri::command]
pub fn pin_image(app: AppHandle, id: u32) -> Option<String> {
    snip::pin_image(&app, id)
}

#[tauri::command(async)]
pub fn pin_close(app: AppHandle, id: u32) -> AppResult<()> {
    snip::close_pin(&app, id)
}

#[tauri::command(async)]
pub fn pins_close_all(app: AppHandle) -> AppResult<()> {
    snip::close_all_pins(&app)
}

#[tauri::command]
pub fn pins_count(app: AppHandle) -> u32 {
    snip::pin_count(&app)
}

#[tauri::command]
pub fn screen_text_last(app: AppHandle) -> ScreenText {
    snip::last_text(&app)
}

/// Copy text (the last text read from the screen) again.
#[tauri::command(async)]
pub fn text_copy(text: String) -> AppResult<()> {
    #[cfg(windows)]
    super::capture::copy_text(&text)?;
    #[cfg(not(windows))]
    let _ = text;
    Ok(())
}
