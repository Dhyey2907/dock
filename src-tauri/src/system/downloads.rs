//! "Download finished": watches your Downloads folder and shows a card, with Open, when a new
//! file has finished arriving. Browsers write to a temporary name (.crdownload, .part) and rename
//! it at the end, so those are ignored, and a file must stop growing before it counts.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration as StdDuration, Instant},
};

use chrono::{DateTime, Duration, Utc};

use crate::automations::{Bubble, BubbleKind};

/// Partial downloads and scratch files: never a finished download.
const IGNORED: [&str; 8] = [
    "crdownload",
    "part",
    "partial",
    "tmp",
    "download",
    "opdownload",
    "temp",
    "!ut",
];
/// How long a file must stay the same size before it counts as finished.
pub const SETTLE: StdDuration = StdDuration::from_millis(1500);
/// The same file isn't announced twice within this time (a browser may touch it again).
const REPEAT_AFTER: StdDuration = StdDuration::from_secs(10 * 60);

/// Whether `path` names a file worth a card (not a partial download or a hidden scratch file).
pub fn is_finished_download(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.starts_with('.') || name.starts_with("~$") || name.eq_ignore_ascii_case("desktop.ini") {
        return false;
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    !IGNORED.contains(&extension.as_str())
}

/// "4.2 MB"
pub fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn download_bubble(path: &Path, bytes: u64, now: DateTime<Utc>) -> Bubble {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "A file".into());
    Bubble {
        id: format!("download:{}", path.to_string_lossy().to_lowercase()),
        kind: BubbleKind::Download,
        title: format!("Downloaded {name}"),
        body: format!("{} · in Downloads", size_text(bytes)),
        expires_at: now + Duration::seconds(20),
    }
}

/// Files announced lately, so a file touched again soon after isn't announced twice.
#[derive(Debug, Default)]
pub struct Announced {
    seen: HashMap<PathBuf, Instant>,
}

impl Announced {
    /// Record `path`; `false` if it was announced within the last few minutes.
    pub fn first_time(&mut self, path: &Path, now: Instant) -> bool {
        self.seen.retain(|_, at| now.duration_since(*at) < REPEAT_AFTER);
        let key = PathBuf::from(path.to_string_lossy().to_lowercase());
        if self.seen.contains_key(&key) {
            return false;
        }
        self.seen.insert(key, now);
        true
    }
}

/// The user's Downloads folder.
#[cfg(windows)]
pub fn folder() -> Option<PathBuf> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    };
    // SAFETY: the returned string is freed after copying.
    unsafe {
        let text = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let path = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
        path.map(PathBuf::from)
    }
}

#[cfg(not(windows))]
pub fn folder() -> Option<PathBuf> {
    None
}

/// Wait until `path` stops growing; its size then, or `None` if it went away (a download that
/// was cancelled or renamed again).
pub fn settled_size(path: &Path) -> Option<u64> {
    let mut last = std::fs::metadata(path).ok()?.len();
    for _ in 0..40 {
        std::thread::sleep(SETTLE);
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        if metadata.len() == last && last > 0 {
            return Some(last);
        }
        last = metadata.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_downloads_are_ignored() {
        assert!(is_finished_download(Path::new(r"C:\Users\x\Downloads\setup.exe")));
        assert!(is_finished_download(Path::new("report.PDF")));
        assert!(is_finished_download(Path::new("Makefile")));
        assert!(!is_finished_download(Path::new("setup.exe.crdownload")));
        assert!(!is_finished_download(Path::new("movie.mkv.part")));
        assert!(!is_finished_download(Path::new("x.TMP")));
        assert!(!is_finished_download(Path::new("~$draft.docx")));
        assert!(!is_finished_download(Path::new(".hidden")));
        assert!(!is_finished_download(Path::new("desktop.ini")));
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_text(512), "512 bytes");
        assert_eq!(size_text(1536), "1.5 KB");
        assert_eq!(size_text(4_404_019), "4.2 MB");
        assert_eq!(size_text(250 * 1024 * 1024), "250 MB");
    }

    #[test]
    fn each_file_is_announced_once() {
        let mut announced = Announced::default();
        let start = Instant::now();
        assert!(announced.first_time(Path::new(r"C:\D\a.zip"), start));
        assert!(!announced.first_time(Path::new(r"C:\D\A.ZIP"), start + StdDuration::from_secs(5)));
        assert!(announced.first_time(Path::new(r"C:\D\b.zip"), start));
        assert!(announced.first_time(Path::new(r"C:\D\a.zip"), start + StdDuration::from_secs(11 * 60)));
    }

    #[test]
    fn cards_name_the_file_and_its_size() {
        let card = download_bubble(Path::new(r"C:\D\setup.exe"), 2048, Utc::now());
        assert_eq!(card.title, "Downloaded setup.exe");
        assert_eq!(card.body, "2.0 KB · in Downloads");
        assert_eq!(card.kind, BubbleKind::Download);
    }

    #[test]
    fn a_vanished_file_never_settles() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(settled_size(&dir.path().join("gone.zip")), None);
    }
}
