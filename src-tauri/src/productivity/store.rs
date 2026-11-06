//! A JSON file plus an in-memory copy, saved the way the settings store saves: write a temp file,
//! then rename it over the old one, so a crash never leaves half a file. An unreadable file is
//! kept aside as `<name>.bad` and the document starts empty.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppResult;

/// Write `bytes` to `path` atomically (temp file, then rename).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub struct JsonDoc<T> {
    path: PathBuf,
    value: Mutex<T>,
}

impl<T: Serialize + DeserializeOwned + Default + Clone> JsonDoc<T> {
    pub fn open(path: PathBuf) -> Self {
        let value = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<T>(&text).unwrap_or_else(|error| {
                tracing::warn!(%error, path = %path.display(), "file unreadable; starting empty");
                let mut bad = path.as_os_str().to_owned();
                bad.push(".bad");
                let _ = std::fs::rename(&path, PathBuf::from(bad));
                T::default()
            }),
            Err(_) => T::default(),
        };
        Self {
            path,
            value: Mutex::new(value),
        }
    }

    pub fn get(&self) -> T {
        self.value.lock().expect("doc lock").clone()
    }

    /// Change the document and save it. The lock is held through the write, so two changes
    /// never interleave on disk. Nothing changes in memory if the save fails.
    pub fn update<R>(&self, change: impl FnOnce(&mut T) -> R) -> AppResult<(T, R)> {
        let mut current = self.value.lock().expect("doc lock");
        let mut next = current.clone();
        let result = change(&mut next);
        write_atomic(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        *current = next.clone();
        Ok((next, result))
    }

    /// Replace the whole document.
    pub fn set(&self, value: T) -> AppResult<T> {
        self.update(|doc| *doc = value).map(|(doc, ())| doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_starts_empty_and_saves_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.json");
        let doc: JsonDoc<Vec<String>> = JsonDoc::open(path.clone());
        assert!(doc.get().is_empty());
        doc.update(|v| v.push("one".into())).unwrap();
        assert_eq!(
            JsonDoc::<Vec<String>>::open(path.clone()).get(),
            vec!["one".to_string()]
        );
        assert!(!dir.path().join("list.json.tmp").exists());
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.json");
        std::fs::write(&path, "{nope").unwrap();
        let doc: JsonDoc<Vec<String>> = JsonDoc::open(path);
        assert!(doc.get().is_empty());
        assert!(dir.path().join("list.json.bad").exists());
    }
}
