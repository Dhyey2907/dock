//! Workspaces: the set of open apps saved under a name (each window's program, its arguments
//! and where it was), and put back later. Saved in `workspaces.json` in the app data folder.
//!
//! Restoring reuses a window that is already open for an app before starting another copy,
//! then moves each window to where it was saved as it appears.

#[cfg(windows)]
mod win;

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

const FILE_NAME: &str = "workspaces.json";
/// Event carrying the saved workspaces whenever they change.
pub const WORKSPACES_CHANGED: &str = "workspaces-changed";
const MAX_NAME: usize = 40;
const MAX_WORKSPACES: usize = 30;

/// Where a window was, in Windows' "normal position" coordinates (what it returns to when not
/// maximized), plus whether it was maximized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowPlace {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

/// One window of a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceApp {
    /// A friendly name for the app.
    pub name: String,
    /// The program, when it is a plain program.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    /// What came after the program on its command line.
    #[serde(default)]
    pub args: String,
    /// Its app id, for Store and packaged apps (started through the Start menu's app list).
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<WindowPlace>,
}

impl WorkspaceApp {
    /// Whether a running window (its program and app id) is this app.
    pub fn matches(&self, exe: Option<&str>, app_id: Option<&str>) -> bool {
        match (&self.app_id, app_id) {
            (Some(saved), Some(running)) if self.exe.is_none() => saved.eq_ignore_ascii_case(running),
            _ => match (&self.exe, exe) {
                (Some(saved), Some(running)) => saved.eq_ignore_ascii_case(running),
                _ => false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Workspace {
    pub name: String,
    #[ts(type = "string")]
    pub saved: DateTime<Utc>,
    pub apps: Vec<WorkspaceApp>,
}

/// What came after the program on a command line (`"C:\a b\x.exe" --flag` gives `--flag`).
pub fn args_after_program(command_line: &str) -> String {
    let line = command_line.trim_start();
    let rest = if let Some(quoted) = line.strip_prefix('"') {
        match quoted.find('"') {
            Some(end) => &quoted[end + 1..],
            None => "",
        }
    } else {
        match line.find(char::is_whitespace) {
            Some(end) => &line[end..],
            None => "",
        }
    };
    rest.trim().to_string()
}

/// A workspace name as saved: trimmed and short.
pub fn clean_name(name: &str) -> AppResult<String> {
    let name: String = name.trim().chars().take(MAX_NAME).collect();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Give the workspace a name.".into()));
    }
    Ok(name)
}

/// `list` with `workspace` in it, replacing one of the same name (any case), newest first.
pub fn upsert(mut list: Vec<Workspace>, workspace: Workspace) -> Vec<Workspace> {
    list.retain(|w| !w.name.eq_ignore_ascii_case(&workspace.name));
    list.insert(0, workspace);
    list.truncate(MAX_WORKSPACES);
    list
}

/// The workspaces file plus an in-memory copy.
pub struct WorkspaceStore {
    path: PathBuf,
    list: Mutex<Vec<Workspace>>,
}

impl WorkspaceStore {
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let list = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            list: Mutex::new(list),
        }
    }

    pub fn list(&self) -> Vec<Workspace> {
        self.list.lock().expect("workspaces lock").clone()
    }

    pub fn get(&self, name: &str) -> Option<Workspace> {
        self.list()
            .into_iter()
            .find(|w| w.name.eq_ignore_ascii_case(name))
    }

    fn write(&self, list: Vec<Workspace>) -> AppResult<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&list)?)?;
        std::fs::rename(&tmp, &self.path)?;
        *self.list.lock().expect("workspaces lock") = list;
        Ok(())
    }

    pub fn save(&self, workspace: Workspace) -> AppResult<()> {
        self.write(upsert(self.list(), workspace))
    }

    pub fn remove(&self, name: &str) -> AppResult<()> {
        let mut list = self.list();
        list.retain(|w| !w.name.eq_ignore_ascii_case(name));
        self.write(list)
    }
}

fn changed(app: &AppHandle) {
    let list = app.state::<WorkspaceStore>().list();
    if let Err(error) = app.emit(WORKSPACES_CHANGED, &list) {
        tracing::warn!(%error, "failed to send workspaces");
    }
}

/// Save the apps open now as `name`.
pub fn save_current(app: &AppHandle, name: &str) -> AppResult<Workspace> {
    let name = clean_name(name)?;
    #[cfg(windows)]
    let apps = win::capture();
    #[cfg(not(windows))]
    let apps = Vec::new();
    if apps.is_empty() {
        return Err(AppError::InvalidInput(
            "There are no app windows open to save.".into(),
        ));
    }
    let workspace = Workspace {
        name,
        saved: Utc::now(),
        apps,
    };
    app.state::<WorkspaceStore>().save(workspace.clone())?;
    changed(app);
    Ok(workspace)
}

pub fn remove(app: &AppHandle, name: &str) -> AppResult<()> {
    app.state::<WorkspaceStore>().remove(name)?;
    changed(app);
    Ok(())
}

/// Open the apps of the workspace `name` and put their windows back. Returns at once; windows
/// are placed as they appear.
pub fn restore(app: &AppHandle, name: &str) -> AppResult<()> {
    let workspace = app
        .state::<WorkspaceStore>()
        .get(name)
        .ok_or_else(|| AppError::InvalidInput("That workspace isn't saved any more.".into()))?;
    #[cfg(windows)]
    std::thread::spawn(move || win::restore(&workspace));
    #[cfg(not(windows))]
    let _ = workspace;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> Workspace {
        Workspace {
            name: name.into(),
            saved: DateTime::parse_from_rfc3339("2026-11-06T09:30:00Z")
                .unwrap()
                .into(),
            apps: vec![
                WorkspaceApp {
                    name: "Code".into(),
                    exe: Some(r"c:\program files\vs code\code.exe".into()),
                    args: r#"--new-window "C:\dev\dock""#.into(),
                    app_id: None,
                    place: Some(WindowPlace {
                        x: 10,
                        y: 20,
                        width: 1200,
                        height: 800,
                        maximized: false,
                    }),
                },
                WorkspaceApp {
                    name: "Calculator".into(),
                    exe: None,
                    args: String::new(),
                    app_id: Some("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into()),
                    place: None,
                },
            ],
        }
    }

    #[test]
    fn workspaces_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::open(dir.path());
        assert!(store.list().is_empty());
        store.save(sample("Coding")).unwrap();
        let reopened = WorkspaceStore::open(dir.path());
        assert_eq!(reopened.list(), vec![sample("Coding")]);
        let json = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(json.contains("\"appId\""), "{json}");
        // A missing place or app id is left out, not written as null.
        assert!(!json.contains("null"), "{json}");
    }

    #[test]
    fn older_or_broken_files_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"[{"name":"Old","saved":"2026-01-01T00:00:00Z","apps":[{"name":"Notepad","exe":"notepad.exe"}]}]"#,
        )
        .unwrap();
        let list = WorkspaceStore::open(dir.path()).list();
        assert_eq!(list[0].apps[0].args, "");
        std::fs::write(dir.path().join(FILE_NAME), "{nope").unwrap();
        assert!(WorkspaceStore::open(dir.path()).list().is_empty());
    }

    #[test]
    fn saving_a_name_again_replaces_it_and_moves_it_first() {
        let list = upsert(vec![sample("Coding"), sample("Writing")], sample("writing"));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "writing");
        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::open(dir.path());
        store.save(sample("A")).unwrap();
        store.remove("a").unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn arguments_are_split_from_the_program() {
        assert_eq!(
            args_after_program(r#""C:\Program Files\x.exe" --a "b c""#),
            r#"--a "b c""#
        );
        assert_eq!(
            args_after_program(r"C:\Windows\notepad.exe  C:\notes.txt"),
            r"C:\notes.txt"
        );
        assert_eq!(args_after_program(r#""C:\x.exe""#), "");
        assert_eq!(args_after_program("notepad.exe"), "");
    }

    #[test]
    fn running_windows_match_by_program_or_app_id() {
        let ws = sample("x");
        assert!(ws.apps[0].matches(Some(r"C:\Program Files\VS Code\Code.exe"), None));
        assert!(!ws.apps[0].matches(Some(r"c:\other.exe"), None));
        assert!(ws.apps[1].matches(None, Some("microsoft.windowscalculator_8wekyb3d8bbwe!app")));
        assert!(clean_name("   ").is_err());
        assert_eq!(clean_name("  Morning  ").unwrap(), "Morning");
    }
}
