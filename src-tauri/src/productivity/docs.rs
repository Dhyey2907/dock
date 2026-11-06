//! The other widgets' saved documents: the kanban board, the time log, quick links and app
//! folders. Each page sends the whole document back when it changes; it is tidied here (blank
//! names dropped, lengths capped, unknown links refused) before it is saved.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const MAX_TEXT: usize = 500;
const MAX_ITEMS: usize = 1000;

fn clip(text: &str) -> String {
    text.trim().chars().take(MAX_TEXT).collect()
}

/// Ids are made by the page; keep them short and unique, dropping repeats.
fn unique_ids<T>(items: &mut Vec<T>, id: impl Fn(&T) -> &str) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        let id = id(item);
        !id.is_empty() && id.len() <= 64 && seen.insert(id.to_string())
    });
    items.truncate(MAX_ITEMS);
}

// ---------- Kanban ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KanbanColumn {
    Todo,
    Doing,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KanbanCard {
    pub id: String,
    pub text: String,
    pub column: KanbanColumn,
}

/// Cards in board order; a column shows its cards in this order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct KanbanBoard {
    pub cards: Vec<KanbanCard>,
}

impl KanbanBoard {
    pub fn normalized(mut self) -> Self {
        for card in &mut self.cards {
            card.text = clip(&card.text);
        }
        self.cards.retain(|c| !c.text.is_empty());
        unique_ids(&mut self.cards, |c| &c.id);
        self
    }
}

// ---------- Time tracker ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeEntry {
    pub id: String,
    pub project_id: String,
    #[ts(type = "string")]
    pub start: DateTime<Utc>,
    /// `None` while the timer runs.
    #[ts(optional, type = "string")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct TimeLog {
    pub projects: Vec<TimeProject>,
    pub entries: Vec<TimeEntry>,
}

/// Entries kept: about a year of a few a day.
const MAX_ENTRIES: usize = 5000;

impl TimeLog {
    pub fn normalized(mut self) -> Self {
        for p in &mut self.projects {
            p.name = clip(&p.name);
        }
        self.projects.retain(|p| !p.name.is_empty());
        unique_ids(&mut self.projects, |p| &p.id);
        let projects: std::collections::HashSet<String> =
            self.projects.iter().map(|p| p.id.clone()).collect();
        self.entries
            .retain(|e| projects.contains(&e.project_id) && e.end.is_none_or(|end| end >= e.start));
        // Only one timer runs: keep the latest start running, stop the others where the next began.
        let mut running: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].end.is_none())
            .collect();
        running.sort_by_key(|&i| self.entries[i].start);
        if let Some((&last, older)) = running.split_last() {
            let stop_at = self.entries[last].start;
            for &i in older {
                self.entries[i].end = Some(stop_at.max(self.entries[i].start));
            }
        }
        let mut seen = std::collections::HashSet::new();
        self.entries
            .retain(|e| !e.id.is_empty() && seen.insert(e.id.clone()));
        if self.entries.len() > MAX_ENTRIES {
            self.entries.sort_by_key(|e| e.start);
            let extra = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..extra);
        }
        self
    }
}

// ---------- Quick links ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuickLink {
    pub id: String,
    pub name: String,
    /// A web address (https://…) or a folder or file on this PC.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct QuickLinks {
    pub links: Vec<QuickLink>,
}

/// What a link or bookmark opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Web(String),
    Path(String),
}

/// Read a typed target: a web address (a bare "example.com" gets https://) or an absolute path
/// on this PC. Anything else (scripts, other schemes) is refused.
pub fn parse_target(text: &str) -> Option<Target> {
    let text = text.trim().trim_matches('"');
    if text.is_empty() {
        return None;
    }
    let is_path = text.starts_with("\\\\")
        || (text.len() >= 3 && text.as_bytes()[1] == b':' && matches!(text.as_bytes()[2], b'\\' | b'/'));
    if is_path {
        return Some(Target::Path(text.to_string()));
    }
    let with_scheme = if text.contains("://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url
            .host_str()
            .is_none_or(|h| !h.contains('.') && h != "localhost")
    {
        return None;
    }
    Some(Target::Web(url.to_string()))
}

impl QuickLinks {
    pub fn normalized(mut self) -> Self {
        for link in &mut self.links {
            link.name = clip(&link.name);
            link.target = match parse_target(&link.target) {
                Some(Target::Web(url)) => url,
                Some(Target::Path(path)) => path,
                None => String::new(),
            };
            if link.name.is_empty() {
                link.name = link.target.clone();
            }
        }
        self.links.retain(|l| !l.target.is_empty());
        unique_ids(&mut self.links, |l| &l.id);
        self
    }
}

// ---------- App folders ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderApp {
    pub name: String,
    /// The Start menu shortcut (or program) that starts it.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppFolder {
    pub id: String,
    pub name: String,
    pub apps: Vec<FolderApp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppFolders {
    pub folders: Vec<AppFolder>,
}

/// Programs a folder may start: shortcuts and program files.
pub fn launchable_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    matches!(parse_target(path), Some(Target::Path(_)))
        && (lower.ends_with(".lnk") || lower.ends_with(".exe") || lower.ends_with(".url"))
}

impl AppFolders {
    pub fn normalized(mut self) -> Self {
        for folder in &mut self.folders {
            folder.name = clip(&folder.name);
            if folder.name.is_empty() {
                folder.name = "Apps".into();
            }
            folder.apps.retain(|a| launchable_path(&a.path));
            let mut seen = std::collections::HashSet::new();
            folder.apps.retain(|a| seen.insert(a.path.to_lowercase()));
            folder.apps.truncate(48);
            for app in &mut folder.apps {
                app.name = clip(&app.name);
            }
        }
        unique_ids(&mut self.folders, |f| &f.id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn kanban_drops_blank_and_repeated_cards() {
        let card = |id: &str, text: &str| KanbanCard {
            id: id.into(),
            text: text.into(),
            column: KanbanColumn::Todo,
        };
        let board = KanbanBoard {
            cards: vec![card("a", " x "), card("b", "  "), card("a", "y"), card("c", "z")],
        }
        .normalized();
        let ids: Vec<_> = board.cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["a", "c"]);
        assert_eq!(board.cards[0].text, "x");
    }

    #[test]
    fn only_one_timer_runs() {
        let entry = |id: &str, start: i64| TimeEntry {
            id: id.into(),
            project_id: "p".into(),
            start: at(start),
            end: None,
        };
        let log = TimeLog {
            projects: vec![TimeProject {
                id: "p".into(),
                name: "Work".into(),
            }],
            entries: vec![entry("a", 0), entry("b", 100), entry("c", 50)],
        }
        .normalized();
        let end = |id: &str| log.entries.iter().find(|e| e.id == id).unwrap().end;
        assert_eq!(end("a"), Some(at(100)));
        assert_eq!(end("c"), Some(at(100)));
        assert_eq!(end("b"), None);
    }

    #[test]
    fn entries_of_removed_projects_go() {
        let log = TimeLog {
            projects: vec![],
            entries: vec![TimeEntry {
                id: "a".into(),
                project_id: "gone".into(),
                start: at(0),
                end: Some(at(5)),
            }],
        }
        .normalized();
        assert!(log.entries.is_empty());
    }

    #[test]
    fn targets() {
        assert_eq!(
            parse_target("example.com"),
            Some(Target::Web("https://example.com/".into()))
        );
        assert_eq!(
            parse_target("http://localhost:3000/x"),
            Some(Target::Web("http://localhost:3000/x".into()))
        );
        assert_eq!(
            parse_target(r"C:\Users\me\Documents"),
            Some(Target::Path(r"C:\Users\me\Documents".into()))
        );
        assert_eq!(
            parse_target(r#""D:/Music""#),
            Some(Target::Path("D:/Music".into()))
        );
        assert_eq!(
            parse_target(r"\\nas\share"),
            Some(Target::Path(r"\\nas\share".into()))
        );
        assert_eq!(parse_target("javascript:alert(1)"), None);
        assert_eq!(parse_target("file:///C:/x"), None);
        assert_eq!(parse_target("notes"), None);
        assert_eq!(parse_target(""), None);
    }

    #[test]
    fn links_are_tidied() {
        let links = QuickLinks {
            links: vec![
                QuickLink {
                    id: "a".into(),
                    name: "".into(),
                    target: "github.com".into(),
                },
                QuickLink {
                    id: "b".into(),
                    name: "bad".into(),
                    target: "cmd /c".into(),
                },
            ],
        }
        .normalized();
        assert_eq!(links.links.len(), 1);
        assert_eq!(links.links[0].target, "https://github.com/");
        assert_eq!(links.links[0].name, "https://github.com/");
    }

    #[test]
    fn folders_keep_only_programs() {
        let app = |path: &str| FolderApp {
            name: "x".into(),
            path: path.into(),
        };
        let folders = AppFolders {
            folders: vec![AppFolder {
                id: "f".into(),
                name: " ".into(),
                apps: vec![
                    app(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Paint.lnk"),
                    app(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\paint.LNK"),
                    app(r"C:\x\run.bat"),
                    app("calc.exe"),
                ],
            }],
        }
        .normalized();
        assert_eq!(folders.folders[0].name, "Apps");
        assert_eq!(folders.folders[0].apps.len(), 1);
    }
}
