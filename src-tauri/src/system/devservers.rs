//! Local dev servers: programs listening for connections on this PC (Vite on 5173, a Rails app on
//! 3000...), read from Windows' TCP table, with the program that owns each port.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DevServer {
    pub port: u16,
    pub pid: u32,
    /// The program, without ".exe" ("node").
    pub process: String,
    /// Reachable from other devices too (listening on every address, not just this PC).
    pub public: bool,
    /// Windows' own or the dock: no Stop button.
    pub protected: bool,
}

/// One listening socket from the TCP table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    /// Bound to a loopback address only (127.0.0.1 or ::1).
    pub loopback: bool,
    /// Bound to every address (0.0.0.0 or ::).
    pub any: bool,
}

/// Ports below this are Windows services (135, 139, 445), never a dev server.
const LOWEST_PORT: u16 = 1024;

/// The port in a TCP table row: the low 16 bits, in network byte order.
pub fn port_from_row(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

/// The listeners that look like local servers, one per port (lowest port first), named by
/// `name_of`. Windows' own services and ports bound to one specific network address are left out.
pub fn dev_servers(listeners: &[Listener], name_of: impl Fn(u32) -> Option<String>) -> Vec<DevServer> {
    let mut servers: Vec<DevServer> = Vec::new();
    let mut sorted = listeners.to_vec();
    sorted.sort_by_key(|l| (l.port, l.pid));
    for listener in sorted {
        if listener.port < LOWEST_PORT || listener.pid <= 4 || !(listener.loopback || listener.any) {
            continue;
        }
        if let Some(existing) = servers.iter_mut().find(|s| s.port == listener.port) {
            existing.public |= listener.any;
            continue;
        }
        let Some(exe) = name_of(listener.pid) else {
            continue;
        };
        let protected = crate::widgets::stats::is_protected(&exe, listener.pid);
        // Windows' service hosts listen on many high ports too; they aren't anyone's dev server.
        if protected && exe.eq_ignore_ascii_case("svchost.exe") {
            continue;
        }
        let process = exe
            .get(..exe.len().saturating_sub(4))
            .filter(|_| exe.to_ascii_lowercase().ends_with(".exe"))
            .unwrap_or(&exe)
            .to_string();
        servers.push(DevServer {
            port: listener.port,
            pid: listener.pid,
            process,
            public: listener.any,
            protected,
        });
    }
    servers
}

#[cfg(windows)]
fn listeners() -> Vec<Listener> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 23;

    /// The raw table for one address family: a u32 count, then that many rows.
    fn table(family: u32) -> Vec<u8> {
        let mut size = 0u32;
        // SAFETY: the first call only reports the size; the second fills a buffer of that size,
        // retried if the table grew in between.
        unsafe {
            GetExtendedTcpTable(None, &mut size, false, family, TCP_TABLE_OWNER_PID_LISTENER, 0);
            for _ in 0..3 {
                let mut buffer = vec![0u8; size as usize + 256];
                size = buffer.len() as u32;
                let result = GetExtendedTcpTable(
                    Some(buffer.as_mut_ptr().cast()),
                    &mut size,
                    false,
                    family,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                );
                if result == 0 {
                    return buffer;
                }
            }
        }
        Vec::new()
    }

    /// Rows of `T` after the u32 count. Rows start at the struct's alignment (4 bytes for both).
    fn rows<T: Copy>(buffer: &[u8]) -> Vec<T> {
        if buffer.len() < 4 {
            return Vec::new();
        }
        let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        let size = std::mem::size_of::<T>();
        (0..count)
            .filter_map(|i| {
                let start = 4 + i * size;
                let bytes = buffer.get(start..start + size)?;
                // SAFETY: `bytes` is exactly one plain-data row; read unaligned to be safe.
                Some(unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<T>()) })
            })
            .collect()
    }

    let mut found = Vec::new();
    for row in rows::<MIB_TCPROW_OWNER_PID>(&table(AF_INET)) {
        let address = u32::from_be(row.dwLocalAddr);
        found.push(Listener {
            port: port_from_row(row.dwLocalPort),
            pid: row.dwOwningPid,
            loopback: address >> 24 == 127,
            any: address == 0,
        });
    }
    for row in rows::<MIB_TCP6ROW_OWNER_PID>(&table(AF_INET6)) {
        let address = std::net::Ipv6Addr::from(row.ucLocalAddr);
        found.push(Listener {
            port: port_from_row(row.dwLocalPort),
            pid: row.dwOwningPid,
            loopback: address.is_loopback(),
            any: address.is_unspecified(),
        });
    }
    found
}

#[cfg(not(windows))]
fn listeners() -> Vec<Listener> {
    Vec::new()
}

/// The local servers running now.
pub fn list() -> Vec<DevServer> {
    dev_servers(&listeners(), crate::widgets::stats::process_name)
}

/// Stop the server on `port`, if it's still `pid`'s.
pub fn stop(port: u16, pid: u32) -> crate::error::AppResult<()> {
    let Some(server) = list().into_iter().find(|s| s.port == port && s.pid == pid) else {
        return Err(crate::error::AppError::InvalidInput(
            "that server has stopped".into(),
        ));
    };
    crate::widgets::stats::end_task(&[server.pid], &server.process)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listener(port: u16, pid: u32, loopback: bool, any: bool) -> Listener {
        Listener {
            port,
            pid,
            loopback,
            any,
        }
    }

    #[test]
    fn ports_come_in_network_byte_order() {
        // Port 5173 (0x1435) as Windows stores it: bytes swapped in the low word.
        assert_eq!(port_from_row(0x3514), 5173);
        assert_eq!(port_from_row(0xABCD_3514), 5173);
    }

    #[test]
    fn keeps_local_servers_once_per_port() {
        let names = |pid: u32| match pid {
            100 => Some("node.exe".to_string()),
            200 => Some("svchost.exe".to_string()),
            300 => Some("python.exe".to_string()),
            _ => None,
        };
        let found = dev_servers(
            &[
                listener(5173, 100, true, false),
                // The same port on IPv6, on every address.
                listener(5173, 100, false, true),
                listener(445, 300, false, true),
                listener(49664, 200, false, true),
                listener(8000, 300, false, true),
                // Bound to one network address only.
                listener(9000, 300, false, false),
                // A process that has gone.
                listener(7000, 999, true, false),
            ],
            names,
        );
        assert_eq!(
            found,
            vec![
                DevServer {
                    port: 5173,
                    pid: 100,
                    process: "node".into(),
                    public: true,
                    protected: false,
                },
                DevServer {
                    port: 8000,
                    pid: 300,
                    process: "python".into(),
                    public: true,
                    protected: false,
                },
            ]
        );
    }
}
