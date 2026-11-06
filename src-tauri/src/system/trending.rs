//! GitHub trending: today's trending repositories, read from github.com/trending at most once an
//! hour and kept on disk, so the popup opens instantly and works offline with the last list.

use std::{path::Path, time::Duration};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How long a fetched list is good for.
pub const FRESH_FOR: chrono::Duration = chrono::Duration::hours(1);
const URL: &str = "https://github.com/trending";
const CACHE_FILE: &str = "github-trending.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrendingRepo {
    /// "owner/name"
    pub repo: String,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[ts(type = "number")]
    pub stars: u64,
    #[ts(type = "number")]
    pub stars_today: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Trending {
    pub repos: Vec<TrendingRepo>,
    /// When the list was read; `None` if it never has been.
    #[ts(optional, type = "string")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<DateTime<Utc>>,
    /// The last refresh failed (offline); the list is the last good one.
    #[serde(default)]
    pub stale: bool,
}

/// Whether a list read at `fetched_at` should be read again at `now`.
pub fn needs_refresh(trending: &Trending, now: DateTime<Utc>) -> bool {
    trending.repos.is_empty() || trending.fetched_at.is_none_or(|at| now - at >= FRESH_FOR)
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let to = text[from..].find(end)? + from;
    Some(&text[from..to])
}

/// Text with tags removed, entities decoded and whitespace collapsed.
fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let decoded = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn number(text: &str) -> u64 {
    text.chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// The repositories on a trending page, in order.
pub fn parse(html: &str) -> Vec<TrendingRepo> {
    html.split("<article class=\"Box-row\"")
        .skip(1)
        .filter_map(|article| {
            let heading = between(article, "<h2", "</h2>")?;
            let href = between(heading, "href=\"/", "\"")?;
            let repo = href.trim_matches('/').to_string();
            if repo.split('/').count() != 2 {
                return None;
            }
            let description = between(article, "<p class=\"col-9", "</p>")
                .and_then(|p| p.split_once('>').map(|(_, rest)| plain(rest)))
                .filter(|d| !d.is_empty());
            let language = between(article, "itemprop=\"programmingLanguage\">", "<")
                .map(plain)
                .filter(|l| !l.is_empty());
            let stars = between(article, &format!("href=\"/{repo}/stargazers\""), "</a>")
                .and_then(|a| a.rsplit_once('>'))
                .map(|(_, count)| number(count))
                .unwrap_or(0);
            let stars_today = article
                .find("stars today")
                .or_else(|| article.find("stars this"))
                .map(|at| {
                    let before = plain(&article[..at]);
                    number(before.rsplit(' ').next().unwrap_or(""))
                })
                .unwrap_or(0);
            Some(TrendingRepo {
                repo,
                description,
                language,
                stars,
                stars_today,
            })
        })
        .collect()
}

pub fn load_cache(dir: &Path) -> Trending {
    std::fs::read_to_string(dir.join(CACHE_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_cache(dir: &Path, trending: &Trending) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(bytes) = serde_json::to_vec(trending) {
        let _ = std::fs::write(dir.join(CACHE_FILE), bytes);
    }
}

async fn fetch() -> Option<Vec<TrendingRepo>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Dock (Windows desktop)")
        .build()
        .ok()?;
    let html = http
        .get(URL)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    let repos = parse(&html);
    (!repos.is_empty()).then_some(repos)
}

/// Today's list: the cached one while it's fresh, else a new read. Offline, the last good list
/// comes back marked stale (or an empty one, the first time); never an error.
pub async fn get(cache_dir: &Path) -> Trending {
    let cached = load_cache(cache_dir);
    let now = Utc::now();
    if !needs_refresh(&cached, now) {
        return cached;
    }
    match fetch().await {
        Some(repos) => {
            let fresh = Trending {
                repos,
                fetched_at: Some(now),
                stale: false,
            };
            save_cache(cache_dir, &fresh);
            fresh
        }
        None => Trending {
            stale: true,
            ..cached
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><body>
<article class="Box-row">
  <div class="float-right d-flex"><a href="/login?return_to=%2FNVIDIA%2Fauto-ontology">Star</a></div>
  <h2 class="h3 lh-condensed">
    <a data-hydro-click="{}" href="/NVIDIA/auto-ontology" class="Link"><svg class="octicon"><path d="M2"></path></svg>
      <span class="text-normal">NVIDIA /</span>
      auto-ontology</a>  </h2>
    <p class="col-9 color-fg-muted my-1 tmp-pr-4">
      Semantic &amp; graph-based reasoning layers over data.
    </p>
  <div class="f6 color-fg-muted mt-2">
      <span class="d-inline-block ml-0"><span class="repo-language-color"></span>
  <span itemprop="programmingLanguage">Python</span>
</span>
      <a href="/NVIDIA/auto-ontology/stargazers" class="Link"><svg aria-label="star"><path d="M8"></path>
</svg>
        7,785</a>
      <a href="/NVIDIA/auto-ontology/forks" class="Link"><svg aria-label="fork"></svg>
        628</a>
      <span class="d-inline-block float-sm-right">
        <svg aria-hidden="true" class="octicon octicon-star"><path d="M8"></path>
</svg>
        2,679 stars today
</span>  </div>
</article>
<article class="Box-row">
  <h2 class="h3 lh-condensed"><a href="/someone/tool">someone / tool</a></h2>
  <div class="f6"><a href="/someone/tool/stargazers">12</a>
  <span>3 stars today</span></div>
</article>
</body></html>"#;

    #[test]
    fn reads_repositories_from_the_trending_page() {
        let repos = parse(PAGE);
        assert_eq!(repos.len(), 2);
        assert_eq!(
            repos[0],
            TrendingRepo {
                repo: "NVIDIA/auto-ontology".into(),
                description: Some("Semantic & graph-based reasoning layers over data.".into()),
                language: Some("Python".into()),
                stars: 7785,
                stars_today: 2679,
            }
        );
        assert_eq!(repos[1].repo, "someone/tool");
        assert_eq!(repos[1].description, None);
        assert_eq!(repos[1].language, None);
        assert_eq!((repos[1].stars, repos[1].stars_today), (12, 3));
        assert!(parse("<html>nothing here</html>").is_empty());
    }

    #[test]
    fn refreshes_at_most_hourly() {
        let at = DateTime::parse_from_rfc3339("2026-11-06T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let list = Trending {
            repos: parse(PAGE),
            fetched_at: Some(at),
            stale: false,
        };
        assert!(!needs_refresh(&list, at + chrono::Duration::minutes(59)));
        assert!(needs_refresh(&list, at + chrono::Duration::minutes(60)));
        assert!(needs_refresh(&Trending::default(), at));
    }

    #[test]
    fn the_cache_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_cache(dir.path()), Trending::default());
        let list = Trending {
            repos: parse(PAGE),
            fetched_at: Some(Utc::now()),
            stale: false,
        };
        save_cache(dir.path(), &list);
        assert_eq!(load_cache(dir.path()), list);
    }
}
