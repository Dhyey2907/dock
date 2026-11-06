//! Claude Code sessions on this PC: running (written to in the last two minutes) and recent,
//! from the transcripts in `~/.claude/projects/<project>/<session>.jsonl`. Read only: each file's
//! modification time, plus its first and last few hundred kilobytes for the title and folder.

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// A session written to this recently counts as running.
pub const RUNNING_SECS: i64 = 120;
/// Sessions listed.
const RECENT: usize = 12;
/// Sessions older than this aren't listed.
const RECENT_DAYS: i64 = 7;
const HEAD_BYTES: u64 = 128 * 1024;
const TAIL_BYTES: u64 = 256 * 1024;
const TITLE_CHARS: usize = 90;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeSession {
    pub id: String,
    pub title: String,
    /// The folder it runs in (its last part, "dock").
    pub project: String,
    #[ts(type = "string")]
    pub modified: DateTime<Utc>,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeActivity {
    /// Newest first.
    pub sessions: Vec<ClaudeSession>,
    pub running: u32,
}

fn clip(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > TITLE_CHARS {
        let cut: String = one_line.chars().take(TITLE_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        one_line
    }
}

fn lines(text: &str) -> impl DoubleEndedIterator<Item = Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
}

/// The text of a user's prompt, if this line is one typed by a person (not a tool result or a
/// wrapped command).
fn prompt_text(line: &Value) -> Option<&str> {
    if line.get("type")?.as_str()? != "user" || line.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let content = line.get("message")?.get("content")?;
    let text = match content {
        Value::String(text) => text.as_str(),
        Value::Array(parts) => parts
            .iter()
            .find(|p| p.get("type").and_then(Value::as_str) == Some("text"))?
            .get("text")?
            .as_str()?,
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty() && !text.starts_with('<')).then_some(text)
}

/// A session's title: the name it was given (newest wins), else its summary, else its first
/// prompt. `head` and `tail` are the starts and ends of the file (they may overlap).
pub fn title(head: &str, tail: &str) -> Option<String> {
    for line in lines(tail).rev() {
        let found = match line.get("type").and_then(Value::as_str) {
            Some("custom-title") => line.get("customTitle"),
            Some("summary") => line.get("summary"),
            _ => None,
        };
        if let Some(text) = found.and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            return Some(clip(text));
        }
    }
    lines(head).find_map(|line| prompt_text(&line).map(clip))
}

/// The session's folder name, from the first line that records it.
pub fn project(head: &str, dir_name: &str) -> String {
    lines(head)
        .find_map(|line| line.get("cwd").and_then(Value::as_str).map(str::to_string))
        .and_then(|cwd| {
            cwd.trim_end_matches(['\\', '/'])
                .rsplit(['\\', '/'])
                .next()
                .map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            dir_name
                .rsplit('-')
                .find(|p| !p.is_empty())
                .unwrap_or(dir_name)
                .to_string()
        })
}

/// Text from `start`, cut to whole lines (a partial first line is dropped unless at 0).
fn read_span(file: &mut File, start: u64, len: u64) -> String {
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = file.take(len).read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if start == 0 {
        text
    } else {
        text.split_once('\n')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_default()
    }
}

/// What's read from a file, kept while its length doesn't change.
#[derive(Clone)]
struct FileRead {
    len: u64,
    title: Option<String>,
    project: String,
}

/// Keeps titles between scans, so only changed files are read again.
#[derive(Default)]
pub struct Scanner {
    cache: HashMap<PathBuf, FileRead>,
}

impl Scanner {
    fn read(&mut self, path: &Path, len: u64, dir_name: &str) -> FileRead {
        if let Some(hit) = self.cache.get(path).filter(|r| r.len == len) {
            return hit.clone();
        }
        let read = match File::open(path) {
            Ok(mut file) => {
                let head = read_span(&mut file, 0, HEAD_BYTES);
                let tail_start = len.saturating_sub(TAIL_BYTES);
                let tail = read_span(&mut file, tail_start, TAIL_BYTES);
                FileRead {
                    len,
                    title: title(&head, &tail),
                    project: project(&head, dir_name),
                }
            }
            Err(_) => FileRead {
                len,
                title: None,
                project: dir_name.to_string(),
            },
        };
        self.cache.insert(path.to_path_buf(), read.clone());
        read
    }

    pub fn scan(&mut self, root: &Path, now: DateTime<Utc>) -> ClaudeActivity {
        let oldest = now - Duration::days(RECENT_DAYS);
        let mut found: Vec<(PathBuf, DateTime<Utc>, u64, String)> = Vec::new();
        for dir in std::fs::read_dir(root).into_iter().flatten().flatten() {
            let dir_path = dir.path();
            if !dir_path.is_dir() {
                continue;
            }
            let dir_name = dir.file_name().to_string_lossy().into_owned();
            for entry in std::fs::read_dir(&dir_path).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "jsonl") {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                let modified: DateTime<Utc> = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();
                if modified >= oldest {
                    found.push((path, modified, meta.len(), dir_name.clone()));
                }
            }
        }
        found.sort_by_key(|f| std::cmp::Reverse(f.1));
        found.truncate(RECENT);
        let sessions: Vec<ClaudeSession> = found
            .into_iter()
            .map(|(path, modified, len, dir_name)| {
                let read = self.read(&path, len, &dir_name);
                let id = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ClaudeSession {
                    title: read.title.unwrap_or_else(|| "Untitled session".into()),
                    project: read.project,
                    running: (now - modified).num_seconds() < RUNNING_SECS,
                    modified,
                    id,
                }
            })
            .collect();
        let keep: std::collections::HashSet<_> = sessions.iter().map(|s| s.id.clone()).collect();
        self.cache.retain(|path, _| {
            path.file_stem()
                .is_some_and(|s| keep.contains(s.to_string_lossy().as_ref()))
        });
        ClaudeActivity {
            running: sessions.iter().filter(|s| s.running).count() as u32,
            sessions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r#"{"type":"queue-operation","content":"hi"}
{"type":"user","isMeta":true,"message":{"content":"<local-command>"},"cwd":"C:\\dev\\dock"}
{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}
{"type":"user","message":{"content":[{"type":"text","text":"  Fix the   failing build\nplease "}]}}
"#;

    #[test]
    fn named_sessions_use_the_newest_name() {
        let tail = r#"{"type":"custom-title","customTitle":"Old name"}
{"type":"assistant"}
{"type":"custom-title","customTitle":"Dock widgets"}
{"type":"summary","summary":"ignored when named later"}"#;
        // The newest line wins, whichever kind it is.
        assert_eq!(title(HEAD, tail).as_deref(), Some("ignored when named later"));
        let tail = r#"{"type":"summary","summary":"x"}
{"type":"custom-title","customTitle":"Dock widgets"}"#;
        assert_eq!(title(HEAD, tail).as_deref(), Some("Dock widgets"));
    }

    #[test]
    fn unnamed_sessions_use_the_first_prompt() {
        assert_eq!(title(HEAD, "").as_deref(), Some("Fix the failing build please"));
        assert_eq!(title("", ""), None);
    }

    #[test]
    fn long_titles_are_cut() {
        let long = format!(r#"{{"type":"custom-title","customTitle":"{}"}}"#, "a".repeat(200));
        let t = title("", &long).unwrap();
        assert_eq!(t.chars().count(), TITLE_CHARS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn project_comes_from_the_working_folder() {
        assert_eq!(project(HEAD, "C--dev-dock"), "dock");
        assert_eq!(project("", "C--dev-dock"), "dock");
    }

    #[test]
    fn scan_lists_newest_first_and_marks_running() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("C--dev-dock");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("aaa.jsonl"), HEAD).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let now = Utc::now();
        let activity = Scanner::default().scan(root.path(), now);
        assert_eq!(activity.sessions.len(), 1);
        assert_eq!(activity.sessions[0].id, "aaa");
        assert_eq!(activity.sessions[0].project, "dock");
        assert!(activity.sessions[0].running);
        assert_eq!(activity.running, 1);
        // An hour on, it's no longer running.
        let later = Scanner::default().scan(root.path(), now + Duration::hours(1));
        assert_eq!(later.running, 0);
    }
}
