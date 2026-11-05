//! Reads Claude Code's transcripts (`~/.claude/projects/**.jsonl`) once at start — only files
//! touched in the last month — then watches the folder and reads just the lines added since.
//! The summary is re-sent when it changes, and recomputed when a window ends or a limit lifts.

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime},
};

use chrono::{Duration as Span, Utc};
use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use super::{next_change, parse_line, summarize, ClaudeUsage, LimitHit, Line, Usage};

pub const WIDGET_CLAUDE: &str = "widget-claude";
/// History kept (and read at start): long enough to learn your usual limit from past hits.
const KEEP_DAYS: i64 = 30;

#[derive(Default)]
struct Index {
    /// By message id: Claude Code writes a message's usage on several lines.
    usage: HashMap<String, Usage>,
    hits: Vec<LimitHit>,
    /// How far each file has been read (to the last complete line).
    offsets: HashMap<PathBuf, u64>,
}

impl Index {
    fn add(&mut self, line: Line) {
        match line {
            Line::Usage(u) => {
                let entry = self.usage.entry(u.id.clone()).or_insert_with(|| u.clone());
                if u.tokens > entry.tokens {
                    entry.tokens = u.tokens;
                }
            }
            Line::Limit(hit) => {
                if !self.hits.contains(&hit) {
                    self.hits.push(hit);
                }
            }
        }
    }

    fn prune(&mut self) {
        let oldest = Utc::now() - Span::days(KEEP_DAYS);
        self.usage.retain(|_, u| u.at >= oldest);
        self.hits.retain(|h| h.resets_at >= oldest);
    }
}

#[derive(Default)]
pub struct ClaudeService {
    index: Mutex<Index>,
    summary: Mutex<ClaudeUsage>,
    /// Bumped on every recompute, so a sleeper for an old boundary gives up.
    generation: AtomicU64,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

pub fn current(app: &AppHandle) -> ClaudeUsage {
    app.state::<ClaudeService>()
        .summary
        .lock()
        .expect("claude lock")
        .clone()
}

/// Claude Code's transcript folder (`CLAUDE_CONFIG_DIR` moves it).
fn root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("projects"));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".claude").join("projects"))
}

fn transcripts(dir: &Path, newer_than: SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            transcripts(&path, newer_than, out);
        } else if path.extension().is_some_and(|e| e == "jsonl")
            && meta.modified().is_ok_and(|m| m >= newer_than)
        {
            out.push(path);
        }
    }
}

/// Read a file from where we left off, up to its last complete line.
fn read_new(index: &mut Index, path: &Path) {
    let Ok(mut file) = File::open(path) else { return };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut offset = index.offsets.get(path).copied().unwrap_or(0);
    if len < offset {
        offset = 0; // rewritten or truncated: start over
    }
    if len == offset || file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }
    let mut reader = BufReader::new((&mut file).take(len - offset));
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                // A half-written last line is read again next time.
                if !line.ends_with('\n') {
                    break;
                }
                offset += n as u64;
                if let Some(parsed) = parse_line(line.trim_end()) {
                    index.add(parsed);
                }
            }
        }
    }
    index.offsets.insert(path.to_path_buf(), offset);
}

pub fn init(app: &AppHandle) {
    let app = app.clone();
    std::thread::Builder::new()
        .name("claude-usage".into())
        .spawn(move || {
            let Some(root) = root() else { return };
            let since = SystemTime::now() - Duration::from_secs(KEEP_DAYS as u64 * 24 * 3600);
            let mut files = Vec::new();
            transcripts(&root, since, &mut files);
            {
                let state = app.state::<ClaudeService>();
                let mut index = state.index.lock().expect("claude lock");
                for file in &files {
                    read_new(&mut index, file);
                }
                tracing::debug!(
                    files = files.len(),
                    messages = index.usage.len(),
                    "read Claude transcripts"
                );
            }
            recompute(&app);
            watch(&app, &root);
        })
        .ok();
}

fn watch(app: &AppHandle, root: &Path) {
    let handle = app.clone();
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        let changed: Vec<PathBuf> = event
            .paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        if !changed.is_empty() {
            changed_soon(&handle, changed);
        }
    });
    match watcher {
        Ok(mut watcher) => match watcher.watch(root, RecursiveMode::Recursive) {
            Ok(()) => *app.state::<ClaudeService>().watcher.lock().expect("claude lock") = Some(watcher),
            Err(error) => tracing::warn!(%error, "could not watch Claude transcripts"),
        },
        Err(error) => tracing::warn!(%error, "could not watch Claude transcripts"),
    }
}

/// Files that changed in a burst (Claude writes many lines a second) are read once, together.
fn changed_soon(app: &AppHandle, paths: Vec<PathBuf>) {
    static PENDING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    static SCHEDULED: AtomicBool = AtomicBool::new(false);
    {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        for path in paths {
            if !pending.contains(&path) {
                pending.push(path);
            }
        }
    }
    if SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        SCHEDULED.store(false, Ordering::SeqCst);
        let paths = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
        {
            let state = app.state::<ClaudeService>();
            let mut index = state.index.lock().expect("claude lock");
            for path in &paths {
                read_new(&mut index, path);
            }
        }
        recompute(&app);
    });
}

/// Summarize, send if changed, and sleep until the summary next changes on its own.
fn recompute(app: &AppHandle) {
    let state = app.state::<ClaudeService>();
    let now = Utc::now();
    let summary = {
        let mut index = state.index.lock().expect("claude lock");
        index.prune();
        let mut usage: Vec<Usage> = index.usage.values().cloned().collect();
        usage.sort_by_key(|u| u.at);
        let mut hits = index.hits.clone();
        hits.sort_by_key(|h| h.at);
        summarize(&usage, &hits, now)
    };
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let wake_at = next_change(&summary, now);
    {
        let mut stored = state.summary.lock().expect("claude lock");
        if *stored != summary {
            *stored = summary.clone();
            if let Err(error) = app.emit(WIDGET_CLAUDE, &summary) {
                tracing::warn!(%error, "failed to send Claude usage");
            }
        }
    }
    let Some(wake_at) = wake_at else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep((wake_at - Utc::now()).to_std().unwrap_or_default() + Duration::from_secs(1));
        if app.state::<ClaudeService>().generation.load(Ordering::SeqCst) == generation {
            recompute(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const LINE: &str = r#"{"type":"assistant","timestamp":"TS","message":{"id":"ID","model":"claude-opus-5-5","usage":{"input_tokens":10,"output_tokens":20}}}"#;

    fn line(id: &str) -> String {
        LINE.replace("ID", id).replace("TS", &Utc::now().to_rfc3339())
    }

    #[test]
    fn reads_only_new_complete_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", line("m1")).unwrap();
        // The same message again (Claude Code repeats it per content block), then half a line.
        write!(file, "{}\n{}", line("m1"), &line("m2")[..30]).unwrap();
        file.flush().unwrap();

        let mut index = Index::default();
        read_new(&mut index, &path);
        assert_eq!(index.usage.len(), 1);
        assert_eq!(index.usage["m1"].tokens, 30);

        // The half line is finished later; only it is read.
        writeln!(file, "{}", &line("m2")[30..]).unwrap();
        file.flush().unwrap();
        read_new(&mut index, &path);
        assert_eq!(index.usage.len(), 2);
    }

    #[test]
    fn a_rewritten_file_is_read_from_the_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, format!("{}\n{}\n", line("a"), line("b"))).unwrap();
        let mut index = Index::default();
        read_new(&mut index, &path);
        std::fs::write(&path, format!("{}\n", line("c"))).unwrap();
        read_new(&mut index, &path);
        assert!(index.usage.contains_key("c"));
    }
}
