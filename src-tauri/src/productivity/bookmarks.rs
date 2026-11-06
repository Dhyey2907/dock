//! Chrome and Edge bookmarks, read from each browser's own `Bookmarks` file in its default
//! profile. Read only, and only when the popup asks.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// Most bookmarks listed per browser.
const MAX_PER_BROWSER: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Browser {
    Chrome,
    Edge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bookmark {
    pub name: String,
    pub url: String,
    /// The folders it sits in, joined with " / " ("Bookmarks bar / Work").
    pub folder: String,
    pub browser: Browser,
}

/// The bookmarks in a `Bookmarks` file, in the browser's order (bar first, then other, then
/// mobile). Only web links are kept.
pub fn parse(text: &str, browser: Browser) -> Vec<Bookmark> {
    let Ok(root) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(roots) = root.get("roots").and_then(Value::as_object) {
        for key in ["bookmark_bar", "other", "synced"] {
            if let Some(node) = roots.get(key) {
                walk(node, "", browser, &mut out);
            }
        }
    }
    out.truncate(MAX_PER_BROWSER);
    out
}

fn walk(node: &Value, path: &str, browser: Browser, out: &mut Vec<Bookmark>) {
    let name = node.get("name").and_then(Value::as_str).unwrap_or("").trim();
    match node.get("type").and_then(Value::as_str) {
        Some("url") => {
            let url = node.get("url").and_then(Value::as_str).unwrap_or("");
            if url.starts_with("http://") || url.starts_with("https://") {
                out.push(Bookmark {
                    name: if name.is_empty() {
                        url.to_string()
                    } else {
                        name.to_string()
                    },
                    url: url.to_string(),
                    folder: path.to_string(),
                    browser,
                });
            }
        }
        Some("folder") => {
            let here = match (path.is_empty(), name.is_empty()) {
                (_, true) => path.to_string(),
                (true, false) => name.to_string(),
                (false, false) => format!("{path} / {name}"),
            };
            if let Some(children) = node.get("children").and_then(Value::as_array) {
                for child in children {
                    walk(child, &here, browser, out);
                }
            }
        }
        _ => {}
    }
}

fn file(browser: Browser) -> Option<PathBuf> {
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    Some(match browser {
        Browser::Chrome => local.join(r"Google\Chrome\User Data\Default\Bookmarks"),
        Browser::Edge => local.join(r"Microsoft\Edge\User Data\Default\Bookmarks"),
    })
}

/// Every browser's bookmarks; a browser that isn't installed adds none.
pub fn read_all() -> Vec<Bookmark> {
    [Browser::Chrome, Browser::Edge]
        .into_iter()
        .flat_map(|browser| {
            file(browser)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map(|text| parse(&text, browser))
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{
      "checksum": "x",
      "roots": {
        "bookmark_bar": { "type": "folder", "name": "Bookmarks bar", "children": [
          { "type": "url", "name": "GitHub", "url": "https://github.com/" },
          { "type": "folder", "name": "Work", "children": [
            { "type": "url", "name": "", "url": "https://example.com/a" },
            { "type": "url", "name": "Script", "url": "javascript:alert(1)" }
          ]}
        ]},
        "other": { "type": "folder", "name": "Other bookmarks", "children": [
          { "type": "url", "name": "News", "url": "http://news.example" }
        ]},
        "synced": { "type": "folder", "name": "Mobile bookmarks", "children": [] }
      },
      "version": 1
    }"#;

    #[test]
    fn reads_folders_in_order_and_skips_non_web_links() {
        let marks = parse(FILE, Browser::Edge);
        let names: Vec<_> = marks.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["GitHub", "https://example.com/a", "News"]);
        assert_eq!(marks[1].folder, "Bookmarks bar / Work");
        assert_eq!(marks[2].folder, "Other bookmarks");
        assert_eq!(marks[0].browser, Browser::Edge);
    }

    #[test]
    fn a_broken_file_gives_nothing() {
        assert!(parse("{", Browser::Chrome).is_empty());
        assert!(parse("{}", Browser::Chrome).is_empty());
    }
}
