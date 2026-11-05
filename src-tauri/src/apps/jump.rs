//! Jump lists: what an app's right-click menu shows above its windows, like the taskbar's.
//! File Explorer gets its pinned and frequent folders (Quick access); other apps get the
//! frequent and recent items Windows keeps for them. Apps' own custom lists ("New incognito
//! window") have no public way to be read, so they aren't shown.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many items each section shows at most.
const SECTION_LIMIT: usize = 9;

/// Explorer's app id (see `implicit_app_id`).
pub const EXPLORER_APP_ID: &str = "Microsoft.Windows.Explorer";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JumpItem {
    pub name: String,
    /// What opening it opens: a folder or file path.
    pub path: String,
    /// Icon source for `app_icon` ("path:<path>").
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JumpSection {
    /// "Pinned", "Frequent" or "Recent".
    pub title: String,
    pub items: Vec<JumpItem>,
}

/// An app's right-click menu, shown in the popup window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppMenu {
    /// The dock it was opened from, and the app's key on it.
    pub dock: String,
    pub key: String,
    pub name: String,
    /// Icon source for `app_icon`.
    pub icon: String,
    /// Whether the app can be started again ("open a new window").
    pub launchable: bool,
    pub windows: Vec<super::DockWindow>,
    pub sections: Vec<JumpSection>,
}

/// A choice in an app's menu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum MenuAction {
    /// Bring one of its windows forward.
    Window {
        #[ts(type = "number")]
        hwnd: isize,
    },
    /// Open a jump list item.
    Open { path: String },
    /// Start the app again.
    Launch,
    /// Close all its windows.
    Close,
}

impl AppMenu {
    /// The popup's size in logical pixels: rows of items, section titles and dividers.
    pub fn size(&self) -> (f64, f64) {
        const ROW: f64 = 36.0;
        const TITLE: f64 = 30.0;
        const DIVIDER: f64 = 9.0;
        const PADDING: f64 = 12.0;
        let items: usize = self.sections.iter().map(|s| s.items.len()).sum();
        let mut height = 2.0 * PADDING + TITLE * self.sections.len() as f64 + ROW * items as f64;
        if !self.windows.is_empty() {
            height += DIVIDER + ROW * self.windows.len().min(MAX_WINDOWS) as f64;
        }
        let actions = usize::from(self.launchable) + usize::from(!self.windows.is_empty());
        if actions > 0 {
            if items > 0 || !self.windows.is_empty() {
                height += DIVIDER;
            }
            height += ROW * actions as f64;
        }
        (300.0, height.max(ROW + 2.0 * PADDING))
    }

    /// Whether `path` is one of the items this menu offered.
    pub fn offers(&self, path: &str) -> bool {
        self.sections.iter().flat_map(|s| &s.items).any(|i| i.path == path)
    }
}

/// Most windows listed in a menu.
pub const MAX_WINDOWS: usize = 8;

/// The sections for the app with `app_id`; empty when Windows keeps nothing for it.
pub fn sections(app_id: Option<&str>) -> Vec<JumpSection> {
    #[cfg(windows)]
    {
        let found = match app_id {
            Some(id) if id.eq_ignore_ascii_case(EXPLORER_APP_ID) => win::explorer_folders(),
            Some(id) => win::documents(id),
            None => Vec::new(),
        };
        found
            .into_iter()
            .map(|(title, items)| JumpSection {
                title: title.into(),
                items: items.into_iter().take(SECTION_LIMIT).collect(),
            })
            .filter(|s| !s.items.is_empty())
            .collect()
    }
    #[cfg(not(windows))]
    {
        let _ = app_id;
        Vec::new()
    }
}

fn item(name: String, path: String) -> JumpItem {
    JumpItem {
        icon: format!("path:{path}"),
        name,
        path,
    }
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{Interface, HSTRING, PCWSTR},
        Win32::{
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                ApplicationDocumentLists, Common::IObjectArray, IApplicationDocumentLists, IEnumShellItems,
                IShellItem, IShellItem2, PropertiesSystem::PSGetPropertyKeyFromName, SHCreateItemFromParsingName,
                ADLT_FREQUENT, ADLT_RECENT, BHID_EnumItems, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
            },
        },
    };

    use super::{item, JumpItem};

    type Found = Vec<(&'static str, Vec<JumpItem>)>;

    /// A shell item's display name and parsing path (`None` for things with no path).
    fn describe(shell_item: &IShellItem) -> Option<JumpItem> {
        // SAFETY: COM calls on a live item; each returned string is copied, then freed.
        unsafe {
            let read = |sigdn| -> Option<String> {
                let raw = shell_item.GetDisplayName(sigdn).ok()?;
                let text = raw.to_string().ok();
                CoTaskMemFree(Some(raw.0 as *const _));
                text
            };
            let name = read(SIGDN_NORMALDISPLAY)?;
            let path = read(SIGDN_DESKTOPABSOLUTEPARSING)?;
            // Only real places: "::{GUID}" items (Recycle Bin, Home) aren't jump list targets.
            (!path.starts_with("::")).then(|| item(name, path))
        }
    }

    /// File Explorer's pinned and frequent folders, the same list its taskbar jump list shows.
    pub fn explorer_folders() -> Found {
        crate::apps::win::com_init();
        let mut pinned = Vec::new();
        let mut frequent = Vec::new();
        // SAFETY: COM calls on objects owned here; out-parameters are locals of the right type.
        unsafe {
            let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                &HSTRING::from("shell:::{3936E9E4-D92C-4EEE-A85A-BC16D5EA0819}"),
                None,
            ) else {
                return Vec::new();
            };
            let Ok(list) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
                return Vec::new();
            };
            let mut is_pinned_key = Default::default();
            let has_key =
                PSGetPropertyKeyFromName(PCWSTR(HSTRING::from("System.Home.IsPinned").as_ptr()), &mut is_pinned_key)
                    .is_ok();
            loop {
                let mut next = [None];
                let mut count = 0;
                if list.Next(&mut next, Some(&mut count)).is_err() || count == 0 {
                    break;
                }
                let Some(shell_item) = next[0].take() else { break };
                let Some(found) = describe(&shell_item) else { continue };
                let is_pinned = has_key
                    && shell_item
                        .cast::<IShellItem2>()
                        .and_then(|i| i.GetBool(&is_pinned_key))
                        .is_ok_and(|b| b.as_bool());
                if is_pinned {
                    pinned.push(found);
                } else {
                    frequent.push(found);
                }
            }
        }
        vec![("Pinned", pinned), ("Frequent", frequent)]
    }

    /// The frequent and recent items Windows keeps for `app_id`.
    pub fn documents(app_id: &str) -> Found {
        crate::apps::win::com_init();
        // SAFETY: COM calls on objects owned here.
        unsafe {
            let Ok(lists) =
                CoCreateInstance::<_, IApplicationDocumentLists>(&ApplicationDocumentLists, None, CLSCTX_INPROC_SERVER)
            else {
                return Vec::new();
            };
            if lists.SetAppID(&HSTRING::from(app_id)).is_err() {
                return Vec::new();
            }
            let read = |kind| -> Vec<JumpItem> {
                let Ok(array) = lists.GetList::<IObjectArray>(kind, super::SECTION_LIMIT as u32) else {
                    return Vec::new();
                };
                let count = array.GetCount().unwrap_or(0);
                (0..count)
                    .filter_map(|i| array.GetAt::<IShellItem>(i).ok())
                    .filter_map(|i| describe(&i))
                    .collect()
            };
            let frequent = read(ADLT_FREQUENT);
            // Recent, minus what Frequent already shows.
            let recent = read(ADLT_RECENT)
                .into_iter()
                .filter(|r| !frequent.iter().any(|f| f.path == r.path))
                .collect();
            vec![("Frequent", frequent), ("Recent", recent)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(items: usize, windows: usize, launchable: bool) -> AppMenu {
        AppMenu {
            dock: "dock".into(),
            key: "pin:x".into(),
            name: "X".into(),
            icon: String::new(),
            launchable,
            windows: (0..windows)
                .map(|i| super::super::DockWindow {
                    hwnd: i as isize,
                    title: format!("w{i}"),
                    active: false,
                    minimized: false,
                })
                .collect(),
            sections: vec![JumpSection {
                title: "Pinned".into(),
                items: (0..items).map(|i| item(format!("i{i}"), format!("C:\\f{i}"))).collect(),
            }],
        }
    }

    #[test]
    fn size_grows_with_rows() {
        let small = menu(0, 0, true).size().1;
        let big = menu(5, 2, true).size().1;
        assert!(big > small + 5.0 * 36.0);
    }

    #[test]
    fn only_offered_paths_open() {
        let m = menu(2, 0, true);
        assert!(m.offers("C:\\f1"));
        assert!(!m.offers("C:\\Windows\\System32\\cmd.exe"));
    }
}
