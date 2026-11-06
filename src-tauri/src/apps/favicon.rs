//! Icons for pinned websites: the site's own icon, fetched from the site once and kept in the
//! dock's cache folder. The page's `<link rel="icon">` tags are tried first (the biggest
//! first), then `/apple-touch-icon.png` and `/favicon.ico`. Only the pinned site is contacted.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::Duration,
};

use url::Url;

/// Pixels the icon is kept at (like app icons).
const SIZE: u32 = 64;
/// Most bytes read from any one response.
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// One `<link>` tag's attributes we care about.
struct LinkTag {
    rel: String,
    href: String,
    sizes: String,
    kind: String,
}

/// The value of attribute `name` in a tag's text (quoted or not), ignoring case.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(name) {
        let start = from + at;
        from = start + name.len();
        // A whole attribute name: preceded by space, followed by optional spaces and '='.
        let before_ok = start == 0 || lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = tag[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value = rest[1..].trim_start();
        let (quote, body) = match value.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
            _ => (None, value),
        };
        let end = match quote {
            Some(q) => body.find(q),
            None => body.find(|c: char| c.is_whitespace() || c == '>'),
        }
        .unwrap_or(body.len());
        return Some(body[..end].trim().to_string());
    }
    None
}

fn link_tags(html: &str) -> Vec<LinkTag> {
    let lower = html.to_ascii_lowercase();
    let mut tags = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<link") {
        let start = from + at;
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let tag = &html[start..end];
        tags.push(LinkTag {
            rel: attribute(tag, "rel").unwrap_or_default().to_ascii_lowercase(),
            href: attribute(tag, "href").unwrap_or_default(),
            sizes: attribute(tag, "sizes").unwrap_or_default().to_ascii_lowercase(),
            kind: attribute(tag, "type").unwrap_or_default().to_ascii_lowercase(),
        });
        from = end;
    }
    tags
}

/// The biggest size a `sizes` attribute lists ("32x32 192x192" → 192); 0 when unknown.
fn largest(sizes: &str) -> u32 {
    sizes
        .split_whitespace()
        .filter_map(|s| s.split_once('x').and_then(|(w, _)| w.parse().ok()))
        .max()
        .unwrap_or(0)
}

/// The icons a page offers, best first: touch icons, then icons by size. SVG icons are
/// skipped (they can't be drawn as a picture here).
pub fn icon_links(html: &str, base: &Url) -> Vec<Url> {
    let mut found: Vec<(u32, Url)> = link_tags(html)
        .into_iter()
        .filter(|t| !t.href.is_empty() && !t.kind.contains("svg") && !t.href.to_lowercase().ends_with(".svg"))
        .filter_map(|t| {
            let rels: Vec<&str> = t.rel.split_whitespace().collect();
            let rank = if rels.iter().any(|r| r.starts_with("apple-touch-icon")) {
                // A touch icon is usually 180 pixels.
                1000 + largest(&t.sizes).max(180)
            } else if rels.contains(&"icon") {
                largest(&t.sizes).max(16)
            } else {
                return None;
            };
            Some((rank, base.join(&t.href).ok()?))
        })
        .filter(|(_, url)| matches!(url.scheme(), "http" | "https"))
        .collect();
    found.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
    found.into_iter().map(|(_, url)| url).collect()
}

/// The cache file for a site, by its host.
pub fn cache_file(dir: &Path, address: &str) -> Option<PathBuf> {
    let url = Url::parse(address).ok()?;
    let host: String = url
        .host_str()?
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        .collect();
    (!host.is_empty()).then(|| dir.join(format!("{host}.png")))
}

/// A picture's bytes as a `SIZE`-pixel PNG.
pub fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    if image.width() < 8 || image.height() < 8 {
        return None;
    }
    let image = image.resize(SIZE, SIZE, image::imageops::FilterType::Lanczos3);
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

async fn get(client: &reqwest::Client, url: Url) -> Option<(Url, Vec<u8>)> {
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let final_url = response.url().clone();
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= MAX_BYTES).then(|| (final_url, bytes.to_vec()))
}

async fn fetch_async(address: &str) -> Option<Vec<u8>> {
    let site = Url::parse(address).ok()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Dock (Windows desktop)")
        .build()
        .ok()?;
    let mut candidates = Vec::new();
    let mut base = site.clone();
    if let Some((page_url, page)) = get(&client, site.clone()).await {
        base = page_url;
        candidates.extend(icon_links(&String::from_utf8_lossy(&page), &base));
    }
    for fallback in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Ok(url) = base.join(fallback) {
            if !candidates.contains(&url) {
                candidates.push(url);
            }
        }
    }
    for url in candidates {
        if let Some(png) = get(&client, url).await.and_then(|(_, bytes)| to_png(&bytes)) {
            return Some(png);
        }
    }
    None
}

/// The site's icon as a PNG, from the cache in `dir` or fetched (and cached). Blocks; call it
/// off the UI thread.
pub fn icon(dir: &Path, address: &str) -> Option<Vec<u8>> {
    let file = cache_file(dir, address)?;
    if let Ok(bytes) = std::fs::read(&file) {
        return Some(bytes);
    }
    let address = address.to_string();
    // Its own thread and runtime: callers may already be inside the async runtime.
    let png = std::thread::spawn(move || tauri::async_runtime::block_on(fetch_async(&address)))
        .join()
        .ok()
        .flatten()?;
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(&file, &png);
    }
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_links_prefer_touch_icons_then_size() {
        let base = Url::parse("https://example.com/news/").unwrap();
        let html = r#"<html><head>
            <LINK rel="icon" href="/small.png" sizes="16x16">
            <link rel='icon' type='image/png' href='big.png' sizes='32x32 192x192'>
            <link rel="mask-icon" href="/mask.svg">
            <link rel=icon href=/vector.svg type="image/svg+xml">
            <link rel="apple-touch-icon" href="https://cdn.example.com/touch.png">
            <link rel="stylesheet" href="/site.css">
        </head></html>"#;
        let links: Vec<String> = icon_links(html, &base).into_iter().map(String::from).collect();
        assert_eq!(
            links,
            [
                "https://cdn.example.com/touch.png",
                "https://example.com/news/big.png",
                "https://example.com/small.png",
            ]
        );
    }

    #[test]
    fn attributes_must_be_whole_names() {
        assert_eq!(
            attribute(r#"<link data-href="x" href="y">"#, "href").as_deref(),
            Some("y")
        );
        assert_eq!(attribute("<link rel=icon>", "rel").as_deref(), Some("icon"));
        assert_eq!(attribute("<link rel>", "rel"), None);
    }

    #[test]
    fn cache_files_are_named_by_host() {
        let dir = Path::new("C:\\cache");
        assert_eq!(
            cache_file(dir, "https://www.example.com/a?b").unwrap(),
            dir.join("www.example.com.png")
        );
        assert!(cache_file(dir, "not a url").is_none());
    }

    #[test]
    fn pictures_become_small_pngs() {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let png = to_png(bytes.get_ref()).unwrap();
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!((back.width(), back.height()), (SIZE, SIZE));
        assert!(to_png(b"not a picture").is_none());
    }
}
