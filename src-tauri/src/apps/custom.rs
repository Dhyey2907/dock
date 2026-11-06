//! The user's touches on the dock's app lists: websites and folders pinned after the taskbar's
//! pins, apps hidden, renamed or given another icon (their own pick, or one from an icon pack),
//! dividers, and running apps ordered by use. Pure: the service supplies the settings, the
//! usage totals and the icon pack's file names.

use std::collections::HashMap;

use super::{DockApp, ItemKind, Launch, Launchable};
use crate::settings::{AppsSettings, CustomPinKind};

/// Image files the dock decodes itself; any other file's icon comes from Windows.
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "bmp", "webp"];

/// The dock's own pins (websites and folders) as dock items, with how to open each. A folder
/// opens as a popup on a click; its launchable (a middle click) opens it in File Explorer.
pub fn custom_pins(settings: &AppsSettings) -> (Vec<DockApp>, Vec<Launchable>) {
    let mut apps = Vec::new();
    let mut launchables = Vec::new();
    for pin in &settings.custom_pins {
        let (key, icon, kind) = match pin.kind {
            CustomPinKind::Website => (
                format!("web:{}", pin.id),
                format!("favicon:{}", pin.target),
                ItemKind::Website,
            ),
            CustomPinKind::Stack => (
                format!("stack:{}", pin.id),
                format!("path:{}", pin.target),
                ItemKind::Stack,
            ),
        };
        launchables.push(Launchable {
            key: key.clone(),
            launch: Launch::Program(pin.target.clone()),
        });
        let mut app = DockApp::new(key, pin.name.clone(), true, icon);
        app.kind = kind;
        apps.push(app);
    }
    (apps, launchables)
}

/// A path's file name and its name without extension, lowercased.
fn file_names(path: &str) -> Option<(String, String)> {
    let path = std::path::Path::new(path);
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    let stem = path.file_stem()?.to_string_lossy().to_lowercase();
    Some((name, stem))
}

/// A packaged app id's family name without the publisher hash
/// ("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App" → "microsoft.windowsterminal").
fn family(app_id: &str) -> String {
    let before_bang = app_id.split('!').next().unwrap_or(app_id);
    before_bang
        .rsplit_once('_')
        .map_or(before_bang, |(name, _)| name)
        .to_lowercase()
}

/// Whether the user hid `app`: a pattern matches its program file name or path, app id, key
/// or name (all ignoring case).
pub fn is_hidden(app: &DockApp, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let mut names = vec![app.key.to_lowercase(), app.name.to_lowercase()];
    if let Some(id) = &app.app_id {
        names.push(id.to_lowercase());
    }
    if let Some(exe) = &app.exe {
        names.push(exe.to_lowercase());
        if let Some((file, _)) = file_names(exe) {
            names.push(file);
        }
    }
    patterns.iter().any(|p| names.iter().any(|n| n == p))
}

/// What the user types to hide `app`: its program's file name, else its app id, else its key.
pub fn hide_pattern(app: &DockApp) -> String {
    app.exe
        .as_deref()
        .and_then(file_names)
        .map(|(file, _)| file)
        .or_else(|| app.app_id.as_ref().map(|id| id.to_lowercase()))
        .unwrap_or_else(|| app.key.to_lowercase())
}

/// The names an icon pack may use for `app`, best first: "chrome.exe", "chrome", the app id,
/// its family name, then the app's name.
pub fn icon_pack_names(app: &DockApp) -> Vec<String> {
    let mut names = Vec::new();
    if let Some((file, stem)) = app.exe.as_deref().and_then(file_names) {
        names.push(file);
        names.push(stem);
    }
    if let Some(id) = &app.app_id {
        names.push(id.to_lowercase());
        names.push(family(id));
    }
    names.push(app.name.to_lowercase());
    names.dedup();
    names
}

/// The icon pack's picture for `app` ("image:<path>"), matching `<name>.png` ignoring case.
pub fn icon_pack_icon(app: &DockApp, dir: &str, files: &[String]) -> Option<String> {
    if dir.is_empty() || files.is_empty() {
        return None;
    }
    icon_pack_names(app).into_iter().find_map(|name| {
        let wanted = format!("{name}.png");
        files
            .iter()
            .find(|f| f.to_lowercase() == wanted)
            .map(|f| format!("image:{}", std::path::Path::new(dir).join(f).to_string_lossy()))
    })
}

/// The icon source for a file the user picked: pictures are drawn as they are, anything else
/// (an .ico, a program, a shortcut) shows the icon Windows gives it.
pub fn icon_for_file(path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        format!("image:{path}")
    } else {
        format!("path:{path}")
    }
}

/// Everything `apply` needs besides the list.
pub struct Touches<'a> {
    pub settings: &'a AppsSettings,
    /// Seconds each app (by key) has been the front app.
    pub usage: &'a HashMap<String, u64>,
    pub pack_dir: &'a str,
    pub pack_files: &'a [String],
}

/// One dock's list with the user's touches. The main dock (`main`) also gets the dock's own
/// pins, right after the taskbar's.
pub fn apply(mut apps: Vec<DockApp>, main: bool, custom: &[DockApp], t: &Touches) -> Vec<DockApp> {
    if main && !custom.is_empty() {
        let at = apps.iter().rposition(|a| a.pinned).map_or(0, |i| i + 1);
        apps.splice(at..at, custom.iter().cloned());
    }
    apps.retain(|a| !is_hidden(a, &t.settings.hidden));
    for app in &mut apps {
        let own = t.settings.override_for(&app.key);
        if let Some(name) = own.map(|o| o.name.as_str()).filter(|n| !n.is_empty()) {
            app.name = name.to_string();
        }
        if let Some(icon) = own.map(|o| o.icon.as_str()).filter(|i| !i.is_empty()) {
            app.icon = icon_for_file(icon);
        } else if app.kind == ItemKind::App {
            if let Some(icon) = icon_pack_icon(app, t.pack_dir, t.pack_files) {
                app.icon = icon;
            }
        }
        app.divider_after = app.pinned && t.settings.separators.contains(&app.key);
    }
    if t.settings.sort_by_use {
        sort_by_use(&mut apps, t.usage);
    }
    apps
}

/// Running apps that aren't pinned, most-used first; pins keep their places. Ties keep the
/// order they had.
pub fn sort_by_use(apps: &mut [DockApp], usage: &HashMap<String, u64>) {
    let slots: Vec<usize> = (0..apps.len()).filter(|&i| !apps[i].pinned).collect();
    let mut running: Vec<DockApp> = slots.iter().map(|&i| apps[i].clone()).collect();
    running.sort_by_key(|a| std::cmp::Reverse(usage.get(&a.key).copied().unwrap_or(0)));
    for (slot, app) in slots.into_iter().zip(running) {
        apps[slot] = app;
    }
}

/// The pinned item Alt+`n` opens (1-based), in dock order.
pub fn nth_pinned(apps: &[DockApp], n: usize) -> Option<&DockApp> {
    apps.iter().filter(|a| a.pinned).nth(n.checked_sub(1)?)
}

/// Whether media playing from `source` (the session's app id: a packaged app's id, or a
/// program's file name for desktop apps, "chrome.exe") belongs to `app`.
pub fn media_matches(source: &str, app: &DockApp) -> bool {
    let source = source.trim().to_lowercase();
    if source.is_empty() || app.kind != ItemKind::App {
        return false;
    }
    if let Some(id) = &app.app_id {
        if id.to_lowercase() == source || (source.contains('!') && family(id) == family(&source)) {
            return true;
        }
    }
    app.exe
        .as_deref()
        .and_then(file_names)
        .is_some_and(|(file, stem)| source == file || source == stem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{AppOverride, CustomPin};

    fn app(key: &str, pinned: bool, exe: Option<&str>, app_id: Option<&str>) -> DockApp {
        let mut app = DockApp::new(key.into(), format!("Name {key}"), pinned, format!("icon:{key}"));
        app.exe = exe.map(Into::into);
        app.app_id = app_id.map(Into::into);
        app
    }

    fn touches<'a>(settings: &'a AppsSettings, usage: &'a HashMap<String, u64>) -> Touches<'a> {
        Touches {
            settings,
            usage,
            pack_dir: "",
            pack_files: &[],
        }
    }

    #[test]
    fn custom_pins_go_after_the_taskbar_pins_on_the_main_dock_only() {
        let settings = AppsSettings {
            custom_pins: vec![
                CustomPin {
                    id: "w1".into(),
                    name: "News".into(),
                    kind: CustomPinKind::Website,
                    target: "https://news.example.com/".into(),
                },
                CustomPin {
                    id: "s1".into(),
                    name: "Downloads".into(),
                    kind: CustomPinKind::Stack,
                    target: r"C:\Users\me\Downloads".into(),
                },
            ],
            ..AppsSettings::default()
        };
        let (custom, launchables) = custom_pins(&settings);
        assert_eq!(custom[0].key, "web:w1");
        assert_eq!(custom[0].icon, "favicon:https://news.example.com/");
        assert_eq!(custom[1].kind, ItemKind::Stack);
        assert_eq!(
            launchables[1].launch,
            Launch::Program(r"C:\Users\me\Downloads".into())
        );
        let usage = HashMap::new();
        let list = vec![app("pin:a", true, None, None), app("exe:b", false, None, None)];
        let main = apply(list.clone(), true, &custom, &touches(&settings, &usage));
        assert_eq!(
            main.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(),
            ["pin:a", "web:w1", "stack:s1", "exe:b"]
        );
        let other = apply(list, false, &custom, &touches(&settings, &usage));
        assert_eq!(other.len(), 2);
    }

    #[test]
    fn hidden_apps_match_by_file_name_path_app_id_or_key() {
        let notepad = app("exe:x", false, Some(r"c:\windows\notepad.exe"), None);
        let terminal = app(
            "app:t",
            false,
            None,
            Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"),
        );
        assert!(is_hidden(&notepad, &["notepad.exe".into()]));
        assert!(is_hidden(&notepad, &[r"c:\windows\notepad.exe".into()]));
        assert!(!is_hidden(&notepad, &["notepad".into()]));
        assert!(is_hidden(
            &terminal,
            &["microsoft.windowsterminal_8wekyb3d8bbwe!app".into()]
        ));
        assert!(is_hidden(&terminal, &["app:t".into()]));
        assert!(!is_hidden(&terminal, &[]));
        assert_eq!(hide_pattern(&notepad), "notepad.exe");
        assert_eq!(
            hide_pattern(&terminal),
            "microsoft.windowsterminal_8wekyb3d8bbwe!app"
        );
    }

    #[test]
    fn overrides_rename_and_reicon_and_win_over_icon_packs() {
        let settings = AppsSettings {
            overrides: vec![AppOverride {
                key: "pin:a".into(),
                name: "Work".into(),
                icon: r"C:\icons\work.ico".into(),
            }],
            separators: vec!["pin:a".into(), "exe:c".into()],
            ..AppsSettings::default()
        };
        let usage = HashMap::new();
        let files = vec!["CHROME.png".to_string(), "code.png".to_string()];
        let t = Touches {
            pack_dir: r"C:\pack",
            pack_files: &files,
            ..touches(&settings, &usage)
        };
        let list = vec![
            app("pin:a", true, Some(r"c:\chrome.exe"), None),
            app("pin:b", true, Some(r"c:\apps\Chrome.exe"), None),
            app("exe:c", false, Some(r"c:\code.exe"), None),
        ];
        let out = apply(list, true, &[], &t);
        assert_eq!(out[0].name, "Work");
        assert_eq!(out[0].icon, r"path:C:\icons\work.ico");
        assert!(out[0].divider_after);
        assert_eq!(out[1].icon, r"image:C:\pack\CHROME.png");
        assert_eq!(out[2].icon, r"image:C:\pack\code.png");
        // Dividers go between pinned apps only.
        assert!(!out[2].divider_after);
        assert_eq!(icon_for_file(r"C:\a\b.PNG"), r"image:C:\a\b.PNG");
    }

    #[test]
    fn icon_packs_match_app_ids_and_their_family_names() {
        let terminal = app(
            "app:t",
            false,
            None,
            Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"),
        );
        let files = vec!["microsoft.windowsterminal.png".to_string()];
        assert_eq!(
            icon_pack_icon(&terminal, "C:\\p", &files).as_deref(),
            Some("image:C:\\p\\microsoft.windowsterminal.png")
        );
        assert_eq!(icon_pack_icon(&terminal, "", &files), None);
    }

    #[test]
    fn sorting_by_use_reorders_running_apps_around_the_pins() {
        let mut apps = vec![
            app("pin:a", true, None, None),
            app("exe:b", false, None, None),
            app("exe:c", false, None, None),
            app("exe:d", false, None, None),
        ];
        let usage = HashMap::from([("exe:c".to_string(), 50), ("exe:d".to_string(), 900)]);
        sort_by_use(&mut apps, &usage);
        assert_eq!(
            apps.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(),
            ["pin:a", "exe:d", "exe:c", "exe:b"]
        );
    }

    #[test]
    fn number_shortcuts_count_pinned_items_only() {
        let apps = vec![
            app("pin:a", true, None, None),
            app("exe:b", false, None, None),
            app("web:c", true, None, None),
        ];
        assert_eq!(nth_pinned(&apps, 1).map(|a| a.key.as_str()), Some("pin:a"));
        assert_eq!(nth_pinned(&apps, 2).map(|a| a.key.as_str()), Some("web:c"));
        assert!(nth_pinned(&apps, 3).is_none());
        assert!(nth_pinned(&apps, 0).is_none());
    }

    #[test]
    fn media_matches_programs_and_packaged_apps() {
        let chrome = app(
            "pin:c",
            true,
            Some(r"c:\program files\google\chrome.exe"),
            Some("Chrome"),
        );
        assert!(media_matches("chrome.exe", &chrome));
        assert!(media_matches("Chrome", &chrome));
        assert!(!media_matches("msedge.exe", &chrome));
        let spotify = app(
            "pin:s",
            true,
            None,
            Some("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
        );
        assert!(media_matches(
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
            &spotify
        ));
        assert!(!media_matches("", &spotify));
        let mut site = app("web:x", true, None, None);
        site.kind = ItemKind::Website;
        assert!(!media_matches("chrome.exe", &site));
    }
}
