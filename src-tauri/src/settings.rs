//! Typed user preferences, stored as one JSON file in the app config folder. Unknown or missing
//! fields fall back to defaults, so adding a field never breaks an older file. Secrets never
//! live here — they go to Windows Credential Manager.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppResult;

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Edge {
    #[default]
    Bottom,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DockStyle {
    /// Centered and floating, as wide as its content.
    #[default]
    Pill,
    /// Edge to edge, like the Windows taskbar.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    Glass,
    IosGlass,
    Midnight,
    White,
    Black,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DockSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl DockSize {
    /// Dock bar height in logical pixels.
    pub fn height(self) -> f64 {
        match self {
            DockSize::Small => 48.0,
            DockSize::Medium => 56.0,
            DockSize::Large => 64.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskbarMode {
    /// The Windows taskbar auto-hides while the dock runs.
    #[default]
    AutoHide,
    /// The Windows taskbar is hidden completely while the dock runs.
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub edge: Edge,
    pub style: DockStyle,
    pub theme: Theme,
    pub size: DockSize,
    /// The dock slides away and comes back when the cursor touches the edge.
    pub auto_hide: bool,
    pub taskbar_mode: TaskbarMode,
    pub launch_at_startup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: Edge::default(),
            style: DockStyle::default(),
            theme: Theme::default(),
            size: DockSize::default(),
            auto_hide: false,
            taskbar_mode: TaskbarMode::default(),
            launch_at_startup: true,
        }
    }
}

/// The settings file plus an in-memory copy.
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Read the file in `dir`; a missing or unreadable file gives defaults (an unreadable one
    /// is kept as `settings.json.bad` so nothing is silently lost).
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
                tracing::warn!(%error, "settings file unreadable; using defaults");
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().expect("settings lock").clone()
    }

    /// Save to disk (write a temp file, then rename, so a crash never leaves half a file).
    pub fn set(&self, settings: Settings) -> AppResult<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&settings)?)?;
        std::fs::rename(&tmp, &self.path)?;
        *self.current.lock().expect("settings lock") = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());
    }

    #[test]
    fn saved_settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path());
        let changed = Settings {
            edge: Edge::Top,
            theme: Theme::Midnight,
            ..Settings::default()
        };
        store.set(changed.clone()).unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), changed);
    }

    #[test]
    fn unknown_and_missing_fields_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), r#"{"edge":"top","fromTheFuture":1}"#).unwrap();
        let settings = SettingsStore::open(dir.path()).get();
        assert_eq!(settings.edge, Edge::Top);
        assert_eq!(settings.size, DockSize::Medium);
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "{not json").unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());
        assert!(dir.path().join("settings.json.bad").exists());
    }
}
