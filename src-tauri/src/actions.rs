//! Commands for the command bar, workspaces, the latest screenshot, the hotkey editor and the
//! account widgets. Anything that may create a window or block is async.

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{
    accounts::{self, anthropic, github, google, notion, spotify, KeyAccount, OAuthProvider},
    command::{self, IndexHit, InstalledApp, NoteHit},
    commands::SETTINGS_CHANGED,
    error::AppResult,
    screenshot,
    settings::{Settings, SettingsStore, WidgetKind},
    shell::shortcut::{self, HotkeyAction},
    workspaces::{self, Workspace, WorkspaceStore},
};

// ---------- Hotkeys ----------

/// Set one global shortcut. Everything is registered again first, so a taken, repeated or
/// invalid one is refused and the working set stays as it was.
#[tauri::command]
pub fn hotkey_set(app: AppHandle, action: HotkeyAction, shortcut: String) -> AppResult<Settings> {
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    action.set(&mut settings, shortcut.trim().to_string());
    if let Err(error) = shortcut::apply_all(&app, &settings) {
        // Put the working ones back.
        let _ = shortcut::apply_all(&app, &store.get());
        return Err(error);
    }
    store.set(settings)?;
    let settings = store.get();
    app.emit(SETTINGS_CHANGED, &settings)?;
    Ok(settings)
}

// ---------- Command bar ----------

#[tauri::command(async)]
pub fn command_toggle(app: AppHandle) -> AppResult<()> {
    command::toggle_window(&app)
}

#[tauri::command]
pub fn command_hide(app: AppHandle) -> AppResult<()> {
    command::hide(&app)
}

#[tauri::command(async)]
pub fn command_apps(app: AppHandle) -> Vec<InstalledApp> {
    command::apps(&app)
}

#[tauri::command(async)]
pub fn command_files(app: AppHandle, query: String) -> Vec<IndexHit> {
    command::search_files(&app, &query)
}

#[tauri::command(async)]
pub fn command_notes(app: AppHandle) -> Vec<NoteHit> {
    command::notes(&app)
}

#[tauri::command(async)]
pub fn command_launch_app(app: AppHandle, id: String) -> AppResult<()> {
    command::launch_app(&app, &id)
}

#[tauri::command(async)]
pub fn command_open_file(app: AppHandle, path: String) -> AppResult<()> {
    command::open_file(&app, &path)
}

#[tauri::command(async)]
pub fn command_open_widget(app: AppHandle, kind: WidgetKind) -> AppResult<()> {
    command::open_widget(&app, kind)
}

/// Open settings from the command bar (hiding it first), at `section` and maybe a control in it.
#[tauri::command(async)]
pub fn command_open_settings(app: AppHandle, section: String) -> AppResult<()> {
    command::hide(&app)?;
    crate::shell::open_settings_window(&app, Some(&section))
}

// ---------- Workspaces ----------

#[tauri::command]
pub fn workspaces_list(app: AppHandle) -> Vec<Workspace> {
    app.state::<WorkspaceStore>().list()
}

/// Save the open apps as `name`. Async: asking WMI for command lines takes a moment.
#[tauri::command(async)]
pub fn workspace_save(app: AppHandle, name: String) -> AppResult<Workspace> {
    workspaces::save_current(&app, &name)
}

#[tauri::command(async)]
pub fn workspace_restore(app: AppHandle, name: String) -> AppResult<()> {
    command::hide(&app)?;
    workspaces::restore(&app, &name)
}

#[tauri::command]
pub fn workspace_remove(app: AppHandle, name: String) -> AppResult<()> {
    workspaces::remove(&app, &name)
}

// ---------- Latest screenshot ----------

#[tauri::command]
pub fn screenshot_latest(app: AppHandle) -> Option<screenshot::LatestScreenshot> {
    screenshot::current(&app)
}

#[tauri::command(async)]
pub fn screenshot_open(app: AppHandle, path: String) -> AppResult<()> {
    screenshot::open(&app, &path)
}

/// Drag the screenshot out as a file (called while the mouse button is down).
#[tauri::command(async)]
pub fn screenshot_drag(app: AppHandle, window: WebviewWindow, path: String) -> AppResult<()> {
    screenshot::drag(&app, &window, &path)
}

#[tauri::command]
pub fn screenshot_dismiss(app: AppHandle) {
    screenshot::dismiss(&app);
}

// ---------- Accounts ----------

#[tauri::command(async)]
pub fn account_status() -> accounts::AccountStatus {
    accounts::status()
}

/// Save (or, when empty, remove) a pasted key. It goes straight to Credential Manager.
#[tauri::command(async)]
pub fn account_key_set(
    app: AppHandle,
    account: KeyAccount,
    value: String,
) -> AppResult<accounts::AccountStatus> {
    accounts::set_key(account, &value)?;
    if account == KeyAccount::Github {
        github::refresh_soon(&app);
    }
    Ok(accounts::status())
}

/// Sign in through the browser; finishes when the browser comes back (or after five minutes).
#[tauri::command]
pub async fn account_connect(app: AppHandle, provider: OAuthProvider) -> AppResult<accounts::AccountStatus> {
    match provider {
        OAuthProvider::Spotify => spotify::sign_in(&app).await?,
        OAuthProvider::Google => google::sign_in(&app).await?,
    }
    Ok(accounts::status())
}

#[tauri::command(async)]
pub fn account_disconnect(provider: OAuthProvider) -> accounts::AccountStatus {
    accounts::disconnect(provider);
    accounts::status()
}

#[tauri::command(async)]
pub fn account_open_link(url: String) -> AppResult<()> {
    accounts::open_link(&url)
}

/// Ask Claude; the answer streams back as `ask-claude` events carrying the returned id.
#[tauri::command(async)]
pub fn ask_claude(app: AppHandle, question: String) -> AppResult<u32> {
    anthropic::ask(&app, &question)
}

#[tauri::command]
pub fn ask_claude_stop() {
    anthropic::stop();
}

#[tauri::command]
pub async fn spotify_playlists(app: AppHandle) -> AppResult<Vec<spotify::Playlist>> {
    spotify::playlists(&app).await
}

#[tauri::command]
pub async fn spotify_play(app: AppHandle, uri: String) -> AppResult<()> {
    spotify::play(&app, &uri).await
}

#[tauri::command]
pub async fn notion_tasks(app: AppHandle) -> AppResult<Vec<notion::NotionTask>> {
    let db = app.state::<SettingsStore>().get().accounts.notion_database_id;
    notion::tasks(&db).await
}

#[tauri::command]
pub async fn notion_complete(app: AppHandle, id: String) -> AppResult<()> {
    let db = app.state::<SettingsStore>().get().accounts.notion_database_id;
    notion::complete(&db, &id).await
}

#[tauri::command]
pub async fn google_tasks(app: AppHandle) -> AppResult<Vec<google::GoogleTask>> {
    google::tasks(&app).await
}

#[tauri::command]
pub async fn google_complete(app: AppHandle, list_id: String, id: String) -> AppResult<()> {
    google::complete(&app, &list_id, &id).await
}

#[tauri::command]
pub fn github_current(app: AppHandle) -> github::GithubState {
    github::current(&app)
}

#[tauri::command]
pub async fn github_refresh(app: AppHandle) -> AppResult<github::GithubState> {
    github::runs(&app).await
}
