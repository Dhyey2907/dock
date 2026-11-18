//! Icon sources with fallbacks, and the pixel math for drawing a window's own icon.
//!
//! An icon source is one of "lnk:<path>", "exe:<path>", "path:<path>", "aumid:<app id>",
//! "hwnd:<window>", "image:<path>" or "favicon:<address>". Several can be joined with `|` into a
//! chain, best first: `app_icon` tries each in turn and answers with the first that draws. That
//! covers the apps that used to show only a letter: a packaged or Electron app whose app id
//! isn't in the Start menu, a pinned shortcut whose icon file is gone, a process the dock may
//! not open (an elevated or protected one), and a Store app hosted in ApplicationFrameHost.

/// The separator between the sources in a chain. Windows paths and app ids can't contain it.
const SEPARATOR: char = '|';

/// Joins sources into one chain, skipping empty and repeated ones.
pub fn chain<I, S>(sources: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<String> = Vec::new();
    for source in sources {
        let source = source.as_ref();
        if source.is_empty() || source.ends_with(':') || out.iter().any(|s| s.eq_ignore_ascii_case(source)) {
            continue;
        }
        out.push(source.to_string());
    }
    out.join(&SEPARATOR.to_string())
}

/// The sources in a chain, best first. A web address may hold the separator, so a favicon
/// source is never split.
pub fn sources(chain: &str) -> Vec<&str> {
    if chain.starts_with("favicon:") {
        return vec![chain];
    }
    chain
        .split(SEPARATOR)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// A chain without its window sources, for keeping after the windows close: Windows reuses
/// window handles, so a saved one could later draw another app's icon.
pub fn without_windows(chain: &str) -> String {
    self::chain(sources(chain).into_iter().filter(|s| !s.starts_with("hwnd:")))
}

/// Whether a program runs from a Store package, where its own file often has no icon and the
/// dock can't read it anyway.
pub fn is_packaged(exe: &str) -> bool {
    let exe = exe.to_lowercase();
    exe.contains(r"\windowsapps\") || exe.ends_with(r"\applicationframehost.exe")
}

/// Straight RGBA from the same icon drawn once over black and once over white (both BGRA).
///
/// Over black a pixel comes out as `a·c`, over white as `a·c + (1 − a)·255`, so the difference
/// gives the alpha and the black pass the color. This works for every kind of icon: ones with
/// an alpha channel, and old ones with only a mask, which a plain draw would leave opaque.
pub fn unblend(over_black: &[u8], over_white: &[u8]) -> Vec<u8> {
    over_black
        .chunks_exact(4)
        .zip(over_white.chunks_exact(4))
        .flat_map(|(b, w)| {
            let spread: u32 = (0..3).map(|i| u32::from(w[i].saturating_sub(b[i]))).sum();
            let alpha = 255 - (spread + 1) / 3;
            let color = |c: u8| {
                (u32::from(c) * 255 + alpha / 2)
                    .checked_div(alpha)
                    .map_or(0, |v| v.min(255) as u8)
            };
            [color(b[2]), color(b[1]), color(b[0]), alpha as u8]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_skip_empty_and_repeated_sources() {
        assert_eq!(
            chain([
                "aumid:App",
                "",
                "exe:",
                r"exe:C:\a.exe",
                r"EXE:c:\A.EXE",
                "hwnd:7"
            ]),
            r"aumid:App|exe:C:\a.exe|hwnd:7"
        );
        assert_eq!(chain(Vec::<String>::new()), "");
    }

    #[test]
    fn sources_split_chains_but_not_web_addresses() {
        assert_eq!(
            sources(r"lnk:C:\x.lnk|exe:C:\x.exe"),
            vec![r"lnk:C:\x.lnk", r"exe:C:\x.exe"]
        );
        assert_eq!(sources("aumid:App"), vec!["aumid:App"]);
        assert_eq!(
            sources("favicon:https://a.example/?q=a|b"),
            vec!["favicon:https://a.example/?q=a|b"]
        );
        assert!(sources("").is_empty());
    }

    #[test]
    fn window_sources_are_dropped_for_keeping() {
        assert_eq!(
            without_windows("aumid:App|exe:C:\\a.exe|hwnd:7"),
            "aumid:App|exe:C:\\a.exe"
        );
        assert_eq!(without_windows("hwnd:7"), "");
        assert_eq!(
            without_windows("favicon:https://a.example/"),
            "favicon:https://a.example/"
        );
    }

    #[test]
    fn packaged_programs_are_recognised() {
        assert!(is_packaged(
            r"C:\Program Files\WindowsApps\Microsoft.Todos_2.1\Todo.exe"
        ));
        assert!(is_packaged(r"c:\windows\system32\applicationframehost.exe"));
        assert!(!is_packaged(
            r"c:\program files\google\chrome\application\chrome.exe"
        ));
    }

    #[test]
    fn unblend_recovers_alpha_and_color() {
        // Opaque red, transparent, and half-transparent blue (BGRA).
        let black = [0, 0, 255, 0, 0, 0, 0, 0, 64, 0, 0, 0];
        let white = [0, 0, 255, 0, 255, 255, 255, 0, 191, 127, 127, 0];
        assert_eq!(
            unblend(&black, &white),
            vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 128, 128]
        );
    }
}
