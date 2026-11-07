//! What the PC itself knows, for the card widgets: Codex's plan limits (from its own session
//! files), network speed, temperature, free space, the Recycle Bin, and recent files.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Instant, SystemTime},
};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::error::{AppError, AppResult};

// ---------- Codex (ChatGPT) limits ----------

/// One of Codex's limit windows.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitWindow {
    pub used_percent: f64,
    pub window_minutes: u32,
    /// Unix seconds.
    #[ts(type = "number")]
    pub resets_at: i64,
}

/// Your ChatGPT plan's Codex limits, as Codex last saw them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodexUsage {
    /// "free", "plus", "pro"…
    pub plan: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<LimitWindow>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<LimitWindow>,
    /// When Codex wrote it (Unix seconds).
    #[ts(type = "number")]
    pub seen_at: i64,
}

fn limit_window(v: &Value) -> Option<LimitWindow> {
    Some(LimitWindow {
        used_percent: v.get("used_percent")?.as_f64()?,
        window_minutes: v.get("window_minutes")?.as_u64()? as u32,
        resets_at: v.get("resets_at")?.as_i64()?,
    })
}

/// The newest `rate_limits` in a Codex session file's lines.
pub fn parse_codex_limits(text: &str, seen_at: i64) -> Option<CodexUsage> {
    text.lines().rev().find_map(|line| {
        if !line.contains("\"rate_limits\"") {
            return None;
        }
        let json: Value = serde_json::from_str(line).ok()?;
        let limits = find_key(&json, "rate_limits")?;
        Some(CodexUsage {
            plan: limits
                .get("plan_type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            primary: limits.get("primary").and_then(limit_window),
            secondary: limits.get("secondary").and_then(limit_window),
            seen_at,
        })
    })
}

fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .filter(|v| v.is_object())
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

fn newest_files(dir: &Path, depth: usize, out: &mut Vec<(SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() && depth > 0 {
            newest_files(&path, depth - 1, out);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            out.push((meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), path));
        }
    }
}

/// Codex's latest limits from its newest session files; `None` when Codex isn't used here.
pub fn codex_usage() -> Option<CodexUsage> {
    let home = std::env::var_os("USERPROFILE")?;
    let sessions = PathBuf::from(home).join(".codex").join("sessions");
    let mut files = Vec::new();
    newest_files(&sessions, 3, &mut files);
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().take(6).find_map(|(modified, path)| {
        let text = std::fs::read_to_string(&path).ok()?;
        let seen = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        parse_codex_limits(&text, seen)
    })
}

// ---------- Network speed ----------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetSpeed {
    /// Bytes per second.
    pub down: f64,
    pub up: f64,
}

static LAST_BYTES: Mutex<Option<(Instant, u64, u64)>> = Mutex::new(None);

/// Speed since the last call (zero on the first).
pub fn net_speed() -> NetSpeed {
    #[cfg(windows)]
    let (down_total, up_total) = win::octets();
    #[cfg(not(windows))]
    let (down_total, up_total) = (0u64, 0u64);
    let now = Instant::now();
    let mut last = LAST_BYTES.lock().unwrap_or_else(|e| e.into_inner());
    let speed = match *last {
        Some((at, down, up)) => {
            let secs = now.duration_since(at).as_secs_f64().max(0.05);
            NetSpeed {
                down: down_total.saturating_sub(down) as f64 / secs,
                up: up_total.saturating_sub(up) as f64 / secs,
            }
        }
        None => NetSpeed { down: 0.0, up: 0.0 },
    };
    *last = Some((now, down_total, up_total));
    speed
}

// ---------- Temperature ----------

/// The hottest thermal zone Windows reports, in °C (laptops' ACPI zones; `None` if none).
pub fn temperature() -> Option<f64> {
    #[cfg(windows)]
    {
        win::temperature()
    }
    #[cfg(not(windows))]
    None
}

// ---------- Storage ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Drive {
    /// "C:".
    pub name: String,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub free: u64,
}

pub fn drives() -> Vec<Drive> {
    #[cfg(windows)]
    {
        win::drives()
    }
    #[cfg(not(windows))]
    Vec::new()
}

// ---------- Recycle Bin ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecycleBin {
    #[ts(type = "number")]
    pub items: u64,
    #[ts(type = "number")]
    pub bytes: u64,
}

pub fn recycle_bin() -> RecycleBin {
    #[cfg(windows)]
    {
        win::recycle_bin()
    }
    #[cfg(not(windows))]
    RecycleBin { items: 0, bytes: 0 }
}

/// Empty the Recycle Bin for good (the card asks first).
pub fn empty_recycle_bin() -> AppResult<()> {
    #[cfg(windows)]
    {
        win::empty_recycle_bin()
    }
    #[cfg(not(windows))]
    Err(AppError::Platform("not on Windows".into()))
}

// ---------- Recent files, downloads, screenshots ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentFile {
    pub name: String,
    /// What opens it (a file, or Windows' shortcut to it).
    pub path: String,
    /// Unix seconds.
    #[ts(type = "number")]
    pub modified: i64,
    /// A small picture, for screenshots.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

fn newest_in(dir: &Path, count: usize, keep: impl Fn(&Path) -> bool) -> Vec<(SystemTime, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let path = e.path();
            (meta.is_file() && keep(&path)).then(|| (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), path))
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.truncate(count);
    files
}

fn unix(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn file_entry((modified, path): (SystemTime, PathBuf), strip_lnk: bool) -> RecentFile {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = if strip_lnk {
        name.strip_suffix(".lnk").unwrap_or(&name).to_string()
    } else {
        name
    };
    RecentFile {
        name,
        path: path.to_string_lossy().to_string(),
        modified: unix(modified),
        thumbnail: None,
    }
}

/// The files you opened last (Windows' own Recent list), newest first.
pub fn recent_files() -> Vec<RecentFile> {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return Vec::new();
    };
    let dir = PathBuf::from(appdata).join(r"Microsoft\Windows\Recent");
    newest_in(&dir, 16, |p| {
        p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
    })
    .into_iter()
    .map(|f| file_entry(f, true))
    // Windows also lists app links it opened (ms-gamingoverlay and the like): not files.
    .filter(|f| !f.name.starts_with("ms-") && !f.name.contains("---"))
    .take(8)
    .collect()
}

/// The newest finished files in Downloads.
pub fn downloads() -> Vec<RecentFile> {
    let Some(dir) = crate::system::downloads::folder() else {
        return Vec::new();
    };
    newest_in(&dir, 6, crate::system::downloads::is_finished_download)
        .into_iter()
        .map(|f| file_entry(f, false))
        .collect()
}

/// The newest screenshots (Windows' Screenshots folder, wherever it is), with small pictures.
pub fn screenshots() -> Vec<RecentFile> {
    let Some(dir) = crate::screenshot::folder() else {
        return Vec::new();
    };
    newest_in(&dir, 3, crate::screenshot::is_picture)
        .into_iter()
        .map(|f| {
            let path = f.1.clone();
            let mut entry = file_entry(f, false);
            entry.thumbnail = crate::screenshot::thumbnail(&path).ok();
            entry
        })
        .collect()
}

/// Open one of the files these cards listed.
pub fn open_listed(path: &str) -> AppResult<()> {
    let listed = recent_files()
        .iter()
        .chain(downloads().iter())
        .chain(screenshots().iter())
        .any(|f| f.path == path);
    if !listed {
        return Err(AppError::InvalidInput("not a listed file".into()));
    }
    #[cfg(windows)]
    crate::apps::win::open(path);
    Ok(())
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{BSTR, PCWSTR},
        Win32::{
            NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2},
            NetworkManagement::Ndis::IfOperStatusUp,
            Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives},
            System::{
                Com::{
                    CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE,
                    RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
                },
                Variant::VARIANT,
                Wmi::{
                    IWbemLocator, WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
                    WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
                },
            },
            UI::Shell::{
                SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI,
                SHERB_NOSOUND, SHQUERYRBINFO,
            },
        },
    };

    use super::{Drive, RecycleBin};
    use crate::error::{AppError, AppResult};

    /// Bytes received and sent so far on the connected adapters (loopback left out).
    pub fn octets() -> (u64, u64) {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        // SAFETY: GetIfTable2 allocates the table; it's read within its reported length and
        // freed with FreeMibTable.
        unsafe {
            if GetIfTable2(&mut table).is_err() || table.is_null() {
                return (0, 0);
            }
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
            let mut down = 0u64;
            let mut up = 0u64;
            for row in rows {
                // 24 is IF_TYPE_SOFTWARE_LOOPBACK; only hardware adapters that are up.
                if row.Type == 24
                    || row.OperStatus != IfOperStatusUp
                    || row.InterfaceAndOperStatusFlags._bitfield & 1 == 0
                {
                    continue;
                }
                down += row.InOctets;
                up += row.OutOctets;
            }
            FreeMibTable(table as *const _);
            (down, up)
        }
    }

    pub fn temperature() -> Option<f64> {
        crate::apps::win::com_init();
        // SAFETY: COM calls on objects owned here; COM is set up on this thread above.
        unsafe {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
            let services = locator
                .ConnectServer(
                    &BSTR::from("ROOT\\CIMV2"),
                    &BSTR::new(),
                    &BSTR::new(),
                    &BSTR::new(),
                    0,
                    &BSTR::new(),
                    None,
                )
                .ok()?;
            CoSetProxyBlanket(
                &services,
                10,
                0,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
            .ok()?;
            let flags = WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0);
            let list = services
                .ExecQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from("SELECT HighPrecisionTemperature FROM Win32_PerfFormattedData_Counters_ThermalZoneInformation"),
                    flags,
                    None,
                )
                .ok()?;
            let mut hottest: Option<f64> = None;
            loop {
                let mut found = [None];
                let mut count = 0;
                if list.Next(WBEM_INFINITE, &mut found, &mut count).is_err() || count == 0 {
                    break;
                }
                let Some(object) = found[0].take() else { break };
                let mut value = VARIANT::default();
                if object
                    .Get(&BSTR::from("HighPrecisionTemperature"), 0, &mut value, None, None)
                    .is_ok()
                {
                    // Tenths of a kelvin.
                    if let Ok(tenths) = u32::try_from(&value) {
                        let celsius = f64::from(tenths) / 10.0 - 273.15;
                        if (0.0..130.0).contains(&celsius) {
                            hottest = Some(hottest.map_or(celsius, |h: f64| h.max(celsius)));
                        }
                    }
                }
            }
            hottest
        }
    }

    pub fn drives() -> Vec<Drive> {
        // SAFETY: plain queries; each path is a NUL-terminated local.
        unsafe {
            let mask = GetLogicalDrives();
            (0..26u32)
                .filter(|i| mask & (1 << i) != 0)
                .filter_map(|i| {
                    let letter = char::from(b'A' + i as u8);
                    let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
                    // 3 is DRIVE_FIXED.
                    if GetDriveTypeW(PCWSTR(root.as_ptr())) != 3 {
                        return None;
                    }
                    let (mut free, mut total) = (0u64, 0u64);
                    GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), Some(&mut free), Some(&mut total), None)
                        .ok()?;
                    Some(Drive {
                        name: format!("{letter}:"),
                        total,
                        free,
                    })
                })
                .collect()
        }
    }

    pub fn recycle_bin() -> RecycleBin {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: a query into a correctly sized local; a null path means every drive.
        let _ = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) };
        RecycleBin {
            items: info.i64NumItems.max(0) as u64,
            bytes: info.i64Size.max(0) as u64,
        }
    }

    pub fn empty_recycle_bin() -> AppResult<()> {
        // SAFETY: empties every drive's bin; the card has asked the user first.
        unsafe {
            SHEmptyRecycleBinW(
                None,
                PCWSTR::null(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
            .map_err(|e| AppError::Platform(format!("could not empty the Recycle Bin: {e}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_codex_limits_from_its_newest_line() {
        let text = concat!(
            r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":5.0,"window_minutes":300,"resets_at":100},"secondary":null,"plan_type":"plus"}}}"#,
            "\n",
            r#"{"type":"other"}"#,
            "\n",
            r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":15.0,"window_minutes":43200,"resets_at":1795271341},"secondary":{"used_percent":2.5,"window_minutes":10080,"resets_at":1795000000},"plan_type":"free"}}}"#,
        );
        let usage = parse_codex_limits(text, 7).unwrap();
        assert_eq!(usage.plan, "free");
        assert_eq!(usage.primary.as_ref().unwrap().used_percent, 15.0);
        assert_eq!(usage.secondary.as_ref().unwrap().window_minutes, 10080);
        assert_eq!(usage.seen_at, 7);
    }

    #[test]
    fn no_limits_no_usage() {
        assert!(parse_codex_limits("{\"type\":\"x\"}\n", 0).is_none());
    }
}
