//! The Windows side of the command bar: the Start menu's app list and the search index, both
//! read through shell folders (`shell:AppsFolder` and a `search-ms:` query).

use std::ffi::c_void;

use windows::{
    core::{HSTRING, PWSTR},
    Win32::{
        System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::Shell::{
            BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, SHCreateItemFromParsingName,
            SHGetKnownFolderItem, KF_FLAG_DEFAULT, SIGDN, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
            SIGDN_PARENTRELATIVEPARSING,
        },
    },
};

use super::{IndexHit, InstalledApp};

/// Run `work` on a fresh thread with the single-threaded COM shell folders expect, and wait.
fn on_sta_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    std::thread::Builder::new()
        .name("shell-folder".into())
        .spawn(move || {
            // SAFETY: COM is set up and torn down on this thread only.
            let initialised = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
            let result = work();
            if initialised {
                // SAFETY: balances the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
            result
        })
        .ok()?
        .join()
        .ok()
}

fn name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: the returned string is ours to free, and is freed right after it is copied.
    unsafe {
        let text: PWSTR = item.GetDisplayName(kind).ok()?;
        let copy = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const c_void));
        copy.filter(|t| !t.is_empty())
    }
}

/// Up to `limit` items in a shell folder.
fn children(folder: &IShellItem, limit: usize) -> Vec<IShellItem> {
    let mut items = Vec::new();
    // SAFETY: COM calls on objects owned here; `got` has room for the one item asked for.
    unsafe {
        let Ok(list) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            return items;
        };
        while items.len() < limit {
            let mut got = [None];
            let mut fetched = 0u32;
            if list.Next(&mut got, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            if let Some(item) = got[0].take() {
                items.push(item);
            }
        }
    }
    items
}

/// Everything in the Start menu's "All apps", by name.
pub fn installed_apps() -> Vec<InstalledApp> {
    on_sta_thread(|| {
        // SAFETY: a known-folder lookup on this COM thread.
        let Ok(folder) =
            (unsafe { SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) })
        else {
            return Vec::new();
        };
        let mut apps: Vec<InstalledApp> = children(&folder, 2000)
            .iter()
            .filter_map(|item| {
                let id = name(item, SIGDN_PARENTRELATIVEPARSING)?;
                let name = name(item, SIGDN_NORMALDISPLAY)?;
                Some(InstalledApp {
                    icon: format!("aumid:{id}"),
                    id,
                    name,
                })
            })
            // Uninstallers and help links crowd the list without being apps anyone opens.
            .filter(|a| {
                let lower = a.name.to_lowercase();
                !lower.starts_with("uninstall") && !lower.contains("readme")
            })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.id == b.id);
        apps
    })
    .unwrap_or_default()
}

/// The search-ms: address that asks the index for `query` in the indexed places.
pub fn search_address(query: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    // `+` is a space in form encoding, but search-ms wants %20.
    format!("search-ms:query={}", encoded.replace('+', "%20"))
}

/// Files and folders the Windows search index finds for `query`.
pub fn search_files(query: &str, limit: usize) -> Vec<IndexHit> {
    let address = search_address(query);
    on_sta_thread(move || {
        // SAFETY: parsing a shell address on this COM thread.
        let Ok(folder) =
            (unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(address), None) })
        else {
            return Vec::new();
        };
        children(&folder, limit * 3)
            .iter()
            .filter_map(|item| {
                let path = name(item, SIGDN_FILESYSPATH)?;
                let shown = name(item, SIGDN_NORMALDISPLAY).unwrap_or_else(|| path.clone());
                let folder = std::path::Path::new(&path)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(IndexHit {
                    path,
                    name: shown,
                    folder,
                })
            })
            .take(limit)
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::search_address;

    #[test]
    fn queries_are_encoded_for_the_index() {
        assert_eq!(search_address("tax return"), "search-ms:query=tax%20return");
        assert_eq!(search_address("a&b"), "search-ms:query=a%26b");
    }
}
