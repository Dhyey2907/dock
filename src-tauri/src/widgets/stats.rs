//! CPU, memory and battery. CPU use is the busy share of time since the last reading, so it has
//! to be sampled; the service samples every few seconds, and only while a widget shows it.
//!
//! Also the Top apps popup: the processes using the most processor and memory, grouped by
//! program (all of Chrome's processes are one row), and ending one.

use std::{collections::HashMap, sync::Mutex};

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemStats {
    /// Percent; `None` on the first reading (there is nothing to compare with yet).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<u8>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<u8>,
    /// Percent; `None` on a PC without a battery.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<u8>,
    pub charging: bool,
}

/// Busy share between two (idle, total) readings, as a percent.
pub fn cpu_percent(previous: (u64, u64), now: (u64, u64)) -> u8 {
    let idle = now.0.saturating_sub(previous.0);
    let total = now.1.saturating_sub(previous.1);
    if total == 0 {
        return 0;
    }
    ((total.saturating_sub(idle)) * 100 / total).min(100) as u8
}

/// Idle and total CPU time from the previous reading.
static LAST: Mutex<Option<(u64, u64)>> = Mutex::new(None);

#[cfg(windows)]
pub fn read() -> SystemStats {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::GetSystemTimes,
        },
    };
    fn ticks(t: FILETIME) -> u64 {
        (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
    }
    // SAFETY: plain queries filling local out-structs of the right size.
    unsafe {
        let (mut idle, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default());
        let cpu = if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
            // Kernel time includes idle time.
            let now = (ticks(idle), ticks(kernel) + ticks(user));
            let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
            let percent = last.map(|prev| cpu_percent(prev, now));
            *last = Some(now);
            percent
        } else {
            None
        };

        let mut memory = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        let ram = GlobalMemoryStatusEx(&mut memory)
            .ok()
            .map(|()| memory.dwMemoryLoad.min(100) as u8);

        let mut power = SYSTEM_POWER_STATUS::default();
        let (battery, charging) = if GetSystemPowerStatus(&mut power).is_ok() {
            // 255 = unknown; flag 128 = no system battery (a desktop PC).
            let has_battery = power.BatteryLifePercent <= 100 && power.BatteryFlag & 128 == 0;
            (
                has_battery.then_some(power.BatteryLifePercent),
                power.ACLineStatus == 1,
            )
        } else {
            (None, false)
        };

        SystemStats {
            cpu,
            ram,
            battery,
            charging,
        }
    }
}

#[cfg(not(windows))]
pub fn read() -> SystemStats {
    let _ = &LAST;
    SystemStats {
        cpu: None,
        ram: None,
        battery: None,
        charging: false,
    }
}

// ---------- Top apps ----------

/// A program and everything it's running, for the Top apps popup.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TopApp {
    /// The program's file name without ".exe" ("chrome").
    pub name: String,
    /// Its processes; ending the app ends all of them.
    pub pids: Vec<u32>,
    /// Share of the whole processor, percent (one decimal).
    pub cpu: f32,
    /// Working set in bytes.
    #[ts(type = "number")]
    pub memory: u64,
    /// Windows itself or the dock: no End task button.
    pub protected: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TopApps {
    pub by_cpu: Vec<TopApp>,
    pub by_memory: Vec<TopApp>,
}

/// One process at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcSample {
    pub pid: u32,
    /// "chrome.exe"
    pub name: String,
    /// Kernel plus user time, in 100 ns units.
    pub cpu_time: u64,
    pub memory: u64,
}

/// Processes that keep Windows (or the dock) running; ending them logs you out or worse.
const PROTECTED: [&str; 18] = [
    "system",
    "system idle process",
    "registry",
    "memory compression",
    "secure system",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "explorer.exe",
    "dock.exe",
];

/// Whether a process must not be ended from the dock.
pub fn is_protected(name: &str, pid: u32) -> bool {
    pid <= 4 || pid == std::process::id() || PROTECTED.contains(&name.to_ascii_lowercase().as_str())
}

fn display_name(exe: &str) -> String {
    let lower = exe.to_ascii_lowercase();
    if lower.ends_with(".exe") {
        exe[..exe.len() - 4].to_string()
    } else {
        exe.to_string()
    }
}

/// Group processes by program and rank them. CPU use is each process's time since `previous`
/// (pid to CPU time) over `elapsed` (100 ns units) across `cpus` processors.
pub fn rank_apps(
    previous: &HashMap<u32, u64>,
    samples: &[ProcSample],
    elapsed: u64,
    cpus: u32,
    limit: usize,
) -> TopApps {
    let mut groups: Vec<(String, TopApp, u64)> = Vec::new();
    for sample in samples.iter().filter(|s| s.pid != 0) {
        let key = sample.name.to_ascii_lowercase();
        let busy = previous
            .get(&sample.pid)
            .map(|before| sample.cpu_time.saturating_sub(*before))
            .unwrap_or(0);
        let index = match groups.iter().position(|(k, _, _)| *k == key) {
            Some(index) => index,
            None => {
                let app = TopApp {
                    name: display_name(&sample.name),
                    pids: Vec::new(),
                    cpu: 0.0,
                    memory: 0,
                    protected: false,
                };
                groups.push((key, app, 0));
                groups.len() - 1
            }
        };
        let (_, app, total_busy) = &mut groups[index];
        app.pids.push(sample.pid);
        app.memory += sample.memory;
        app.protected |= is_protected(&sample.name, sample.pid);
        *total_busy += busy;
    }
    let capacity = (elapsed as f64) * f64::from(cpus.max(1));
    let mut apps: Vec<TopApp> = groups
        .into_iter()
        .map(|(_, mut app, busy)| {
            if capacity > 0.0 {
                let percent = (busy as f64 * 100.0 / capacity).min(100.0);
                app.cpu = ((percent * 10.0).round() / 10.0) as f32;
            }
            app
        })
        .collect();
    apps.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)));
    let by_cpu = apps.iter().take(limit).cloned().collect();
    apps.sort_by_key(|a| std::cmp::Reverse(a.memory));
    let by_memory = apps.into_iter().take(limit).collect();
    TopApps { by_cpu, by_memory }
}

type ProcTimes = HashMap<u32, u64>;

/// The last process reading: when, and each pid's CPU time.
static LAST_PROCS: Mutex<Option<(std::time::Instant, ProcTimes)>> = Mutex::new(None);

/// The busiest programs now. The first call (or one after a long gap) takes two readings half a
/// second apart, since CPU use needs a span of time.
pub fn top_apps(limit: usize) -> TopApps {
    use std::time::{Duration, Instant};
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
    let mut last = LAST_PROCS.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = last
        .as_ref()
        .is_some_and(|(at, _)| at.elapsed() < Duration::from_secs(30));
    if !fresh {
        *last = Some((Instant::now(), times(&sample_processes())));
        std::thread::sleep(Duration::from_millis(500));
    }
    let samples = sample_processes();
    let now = Instant::now();
    let (before, previous) = last.take().unwrap_or((now, HashMap::new()));
    let elapsed = ((now - before).as_nanos() / 100) as u64;
    let ranked = rank_apps(&previous, &samples, elapsed, cpus, limit);
    *last = Some((now, times(&samples)));
    ranked
}

fn times(samples: &[ProcSample]) -> ProcTimes {
    samples.iter().map(|s| (s.pid, s.cpu_time)).collect()
}

#[cfg(windows)]
fn sample_processes() -> Vec<ProcSample> {
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };
    fn ticks(t: FILETIME) -> u64 {
        (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
    }
    let mut samples = Vec::new();
    // SAFETY: a process snapshot walked with correctly sized entries; every handle opened here is
    // closed here.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return samples;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let pid = entry.th32ProcessID;
            let name = crate::system::wide(&entry.szExeFile);
            let (mut cpu_time, mut memory) = (0, 0);
            if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let (mut created, mut exited, mut kernel, mut user) = (
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                );
                if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user).is_ok() {
                    cpu_time = ticks(kernel) + ticks(user);
                }
                let mut counters = PROCESS_MEMORY_COUNTERS {
                    cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                    ..Default::default()
                };
                if GetProcessMemoryInfo(process, &mut counters, counters.cb).is_ok() {
                    memory = counters.WorkingSetSize as u64;
                }
                let _ = CloseHandle(process);
            }
            samples.push(ProcSample {
                pid,
                name,
                cpu_time,
                memory,
            });
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    samples
}

#[cfg(not(windows))]
fn sample_processes() -> Vec<ProcSample> {
    Vec::new()
}

/// A process's program file name ("chrome.exe"), if it's still running and can be asked.
#[cfg(windows)]
pub fn process_name(pid: u32) -> Option<String> {
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    // SAFETY: the handle is closed below; the buffer outlives the call and its size is passed.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let ok =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut size)
                .is_ok();
        let _ = CloseHandle(process);
        if !ok {
            return None;
        }
        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_string)
    }
}

#[cfg(not(windows))]
pub fn process_name(_pid: u32) -> Option<String> {
    None
}

/// End the processes `pids`, which must still be running `name` (a pid may have been reused
/// since the list was read). Windows' own processes and the dock are refused.
pub fn end_task(pids: &[u32], name: &str) -> crate::error::AppResult<()> {
    use crate::error::AppError;
    let wanted = display_name(name).to_ascii_lowercase();
    let (mut ended, mut failed) = (0, 0);
    for &pid in pids {
        let Some(actual) = process_name(pid) else { continue };
        if display_name(&actual).to_ascii_lowercase() != wanted {
            continue;
        }
        if is_protected(&actual, pid) {
            return Err(AppError::InvalidInput("a protected process".into()));
        }
        if terminate(pid) {
            ended += 1;
        } else {
            failed += 1;
        }
    }
    if failed > 0 && ended == 0 {
        return Err(AppError::Platform("could not end the app".into()));
    }
    Ok(())
}

#[cfg(windows)]
fn terminate(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE},
    };
    // SAFETY: the handle is closed after use.
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, pid) else {
            return false;
        };
        let ok = TerminateProcess(process, 1).is_ok();
        let _ = CloseHandle(process);
        ok
    }
}

#[cfg(not(windows))]
fn terminate(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pid: u32, name: &str, cpu_time: u64, memory: u64) -> ProcSample {
        ProcSample {
            pid,
            name: name.into(),
            cpu_time,
            memory,
        }
    }

    #[test]
    fn ranks_programs_by_processor_and_memory() {
        let previous: HashMap<u32, u64> = [(10, 1_000), (11, 500), (20, 0)].into_iter().collect();
        let samples = [
            sample(10, "chrome.exe", 3_000, 300),
            sample(11, "chrome.exe", 1_500, 200),
            sample(20, "Code.exe", 1_000, 900),
            // New since the last reading: no CPU use counted yet.
            sample(30, "game.exe", 9_000_000, 100),
            sample(0, "System Idle Process", 0, 0),
        ];
        // 10 000 units on 2 processors: 20 000 of capacity.
        let ranked = rank_apps(&previous, &samples, 10_000, 2, 5);
        let chrome = &ranked.by_cpu[0];
        assert_eq!(chrome.name, "chrome");
        assert_eq!(chrome.pids, vec![10, 11]);
        assert_eq!(chrome.memory, 500);
        assert!((chrome.cpu - 15.0).abs() < 0.01);
        assert_eq!(ranked.by_cpu[1].name, "Code");
        assert!((ranked.by_cpu[1].cpu - 5.0).abs() < 0.01);
        assert_eq!(ranked.by_memory[0].name, "Code");
        assert!(ranked.by_cpu.iter().all(|a| a.name != "System Idle Process"));
        assert_eq!(rank_apps(&previous, &samples, 10_000, 2, 1).by_cpu.len(), 1);
    }

    #[test]
    fn windows_own_processes_are_protected() {
        assert!(is_protected("csrss.exe", 700));
        assert!(is_protected("SVCHOST.EXE", 1200));
        assert!(is_protected("anything.exe", 4));
        assert!(is_protected("anything.exe", std::process::id()));
        assert!(!is_protected("chrome.exe", 1200));
        let ranked = rank_apps(&HashMap::new(), &[sample(900, "dwm.exe", 0, 1)], 1, 1, 5);
        assert!(ranked.by_memory[0].protected);
    }

    #[test]
    fn no_elapsed_time_means_no_cpu() {
        let ranked = rank_apps(&HashMap::new(), &[sample(9, "a.exe", 100, 1)], 0, 4, 5);
        assert_eq!(ranked.by_cpu[0].cpu, 0.0);
    }

    #[test]
    fn computes_cpu_use_between_readings() {
        assert_eq!(cpu_percent((100, 1000), (150, 1100)), 50);
        assert_eq!(cpu_percent((100, 1000), (200, 1100)), 0);
        assert_eq!(cpu_percent((100, 1000), (100, 1100)), 100);
        assert_eq!(cpu_percent((100, 1000), (100, 1000)), 0);
    }
}
