//! The system widgets: tools you'd otherwise dig through Settings, Task Manager or the tray for.
//! Most read on demand, when their popup opens; the few with something to watch (Bluetooth
//! batteries, Downloads, the wallpaper timer) run in `service.rs`.

pub mod bluetooth;
pub mod brightness;
pub mod desktop;
pub mod devservers;
pub mod display;
pub mod downloads;
pub mod service;
pub mod trending;
pub mod uptime;
pub mod usb;
pub mod wallpaper;
pub mod wifi;

/// A NUL-terminated UTF-16 buffer as a string.
pub fn wide(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

/// A string as a NUL-terminated UTF-16 buffer.
pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_strings_round_trip() {
        assert_eq!(wide(&to_wide("Héllo")), "Héllo");
        assert_eq!(wide(&[72, 105, 0, 33]), "Hi");
        assert_eq!(wide(&[72, 105]), "Hi");
    }
}
