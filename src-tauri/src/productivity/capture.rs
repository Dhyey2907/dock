//! Screen pictures, the clipboard and on-device text recognition (Windows.Media.Ocr). The pure
//! parts (cropping, reading clipboard bitmaps, encoding) are tested; the Windows calls are thin.

use base64::Engine;

/// A picture, 4 bytes a pixel, top row first. `bgra` order, as Windows gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// A rectangle in a picture's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The part of a `width` × `height` picture that fractions (0–1, of the picture's size) cover,
/// kept inside it. `None` when it's under 2 pixels either way.
pub fn fraction_rect(width: u32, height: u32, fx: f64, fy: f64, fw: f64, fh: f64) -> Option<PixelRect> {
    if ![fx, fy, fw, fh].iter().all(|v| v.is_finite()) {
        return None;
    }
    let (fx, fw) = if fw < 0.0 { (fx + fw, -fw) } else { (fx, fw) };
    let (fy, fh) = if fh < 0.0 { (fy + fh, -fh) } else { (fy, fh) };
    let px = |f: f64, size: u32| (f.clamp(0.0, 1.0) * size as f64).round() as u32;
    let (x0, y0) = (px(fx, width), px(fy, height));
    let (x1, y1) = (px(fx + fw, width), px(fy + fh, height));
    let rect = PixelRect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    };
    (rect.width >= 2 && rect.height >= 2).then_some(rect)
}

impl Shot {
    pub fn crop(&self, rect: PixelRect) -> Shot {
        let x = rect.x.min(self.width);
        let y = rect.y.min(self.height);
        let width = rect.width.min(self.width - x);
        let height = rect.height.min(self.height - y);
        let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
        for row in y..y + height {
            let start = (row as usize * self.width as usize + x as usize) * 4;
            bgra.extend_from_slice(&self.bgra[start..start + width as usize * 4]);
        }
        Shot { width, height, bgra }
    }

    fn rgba(&self) -> Vec<u8> {
        let mut rgba = self.bgra.clone();
        for p in rgba.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        rgba
    }

    /// As a PNG data URL (for pinned pictures: exact pixels).
    pub fn png_data_url(&self) -> Option<String> {
        use image::ImageEncoder;
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut bytes,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(
            &self.rgba(),
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
        Some(data_url("image/png", &bytes))
    }

    /// As a JPEG data URL (for the full-screen backdrop while choosing an area: smaller, quicker).
    pub fn jpeg_data_url(&self) -> Option<String> {
        let rgb: Vec<u8> = self
            .bgra
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0]])
            .collect();
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
            .encode(&rgb, self.width, self.height, image::ExtendedColorType::Rgb8)
            .ok()?;
        Some(data_url("image/jpeg", &bytes))
    }

    /// Small text reads better scaled up: make the shorter side at least 80 pixels (at most 4×).
    pub fn for_ocr(&self) -> Shot {
        let short = self.width.min(self.height).max(1);
        let factor = 80u32.div_ceil(short).clamp(1, 4);
        let mut shot = self.clone();
        for p in shot.bgra.chunks_exact_mut(4) {
            p[3] = 255;
        }
        if factor == 1 {
            return shot;
        }
        let Some(image) = image::RgbaImage::from_raw(shot.width, shot.height, shot.bgra) else {
            return self.clone();
        };
        let (width, height) = (self.width * factor, self.height * factor);
        let scaled = image::imageops::resize(&image, width, height, image::imageops::FilterType::CatmullRom);
        Shot {
            width,
            height,
            bgra: scaled.into_raw(),
        }
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Read a clipboard bitmap (`CF_DIB`: a BITMAPINFOHEADER, maybe colour masks, then pixels).
/// 24- and 32-bit pictures only, which is what screenshots and browsers put there.
pub fn parse_dib(dib: &[u8]) -> Option<Shot> {
    const BI_RGB: u32 = 0;
    const BI_BITFIELDS: u32 = 3;
    let header = u32_at(dib, 0)? as usize;
    let width = i32_at(dib, 4)?;
    let raw_height = i32_at(dib, 8)?;
    let bits = u16_at(dib, 14)?;
    let compression = u32_at(dib, 16)?;
    let colors_used = u32_at(dib, 32)? as usize;
    if width <= 0 || raw_height == 0 || header < 40 || !matches!(bits, 24 | 32) {
        return None;
    }
    if !(compression == BI_RGB || (compression == BI_BITFIELDS && bits == 32)) {
        return None;
    }
    let (width, height) = (width as usize, raw_height.unsigned_abs() as usize);
    if width * height > 100_000_000 {
        return None;
    }
    let masks = if compression == BI_BITFIELDS && header == 40 {
        12
    } else {
        0
    };
    let offset = header + masks + colors_used * 4;
    let stride = (width * bits as usize).div_ceil(32) * 4;
    let pixels = dib.get(offset..offset + stride * height)?;
    let step = bits as usize / 8;
    let mut bgra = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        // Positive height: bottom row first.
        let source = if raw_height > 0 { height - 1 - row } else { row };
        let line = &pixels[source * stride..source * stride + width * step];
        for p in line.chunks_exact(step) {
            bgra.extend_from_slice(&[p[0], p[1], p[2], if step == 4 { p[3] } else { 255 }]);
        }
    }
    // Many apps leave alpha at zero in 32-bit clipboard pictures: treat that as opaque.
    if step == 4 && bgra.chunks_exact(4).all(|p| p[3] == 0) {
        for p in bgra.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    Some(Shot {
        width: width as u32,
        height: height as u32,
        bgra,
    })
}

#[cfg(windows)]
pub use self::win::*;

#[cfg(windows)]
mod win {
    use windows::{
        Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
        Win32::{
            Foundation::{HANDLE, HGLOBAL, POINT, RECT},
            Graphics::Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
                GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
                BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, SRCCOPY,
            },
            System::{
                DataExchange::{
                    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                    OpenClipboard, SetClipboardData,
                },
                Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
                Ole::{CF_DIB, CF_UNICODETEXT},
            },
            UI::WindowsAndMessaging::GetCursorPos,
        },
    };

    use super::{parse_dib, Shot};
    use crate::error::{AppError, AppResult};

    /// The screen the cursor is on, in physical pixels.
    pub fn cursor_screen() -> Option<RECT> {
        // SAFETY: plain queries into local structs.
        unsafe {
            let mut point = POINT::default();
            GetCursorPos(&mut point).ok()?;
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(monitor, &mut info)
                .as_bool()
                .then_some(info.rcMonitor)
        }
    }

    /// What's on screen in `rect` (physical pixels, virtual-screen coordinates).
    pub fn grab(rect: RECT) -> Option<Shot> {
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        if width <= 0 || height <= 0 {
            return None;
        }
        // SAFETY: GDI objects made here are deselected and deleted before returning; the pixel
        // buffer holds exactly `height` rows of `width` 32-bit pixels.
        unsafe {
            let screen = GetDC(None);
            let memory = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            let old = SelectObject(memory, HGDIOBJ(bitmap.0));
            let copied = BitBlt(
                memory,
                0,
                0,
                width,
                height,
                Some(screen),
                rect.left,
                rect.top,
                SRCCOPY | CAPTUREBLT,
            );
            SelectObject(memory, old);
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bgra = vec![0u8; width as usize * height as usize * 4];
            let rows = GetDIBits(
                memory,
                bitmap,
                0,
                height as u32,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            (copied.is_ok() && rows > 0).then_some(Shot {
                width: width as u32,
                height: height as u32,
                bgra,
            })
        }
    }

    /// Holds the clipboard open; closes it when dropped.
    struct Clipboard;

    impl Clipboard {
        fn open() -> AppResult<Self> {
            // Another app may hold it for a moment.
            for _ in 0..10 {
                // SAFETY: opening the clipboard for this thread; closed in Drop.
                if unsafe { OpenClipboard(None) }.is_ok() {
                    return Ok(Clipboard);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(AppError::Platform("the clipboard is busy".into()))
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: this thread opened the clipboard.
            let _ = unsafe { CloseClipboard() };
        }
    }

    /// The picture on the clipboard, if there is one.
    pub fn clipboard_image() -> AppResult<Option<Shot>> {
        // SAFETY: the clipboard's memory is locked while copied and unlocked straight after.
        unsafe {
            if IsClipboardFormatAvailable(u32::from(CF_DIB.0)).is_err() {
                return Ok(None);
            }
            let _open = Clipboard::open()?;
            let handle = GetClipboardData(u32::from(CF_DIB.0))?;
            let global = HGLOBAL(handle.0);
            let size = GlobalSize(global);
            let data = GlobalLock(global) as *const u8;
            if data.is_null() || size == 0 {
                return Ok(None);
            }
            let bytes = std::slice::from_raw_parts(data, size).to_vec();
            let _ = GlobalUnlock(global);
            Ok(parse_dib(&bytes))
        }
    }

    /// Put `text` on the clipboard.
    pub fn copy_text(text: &str) -> AppResult<()> {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        // SAFETY: the memory is sized for the text, filled while locked, and handed to the
        // clipboard (which owns it after SetClipboardData succeeds).
        unsafe {
            let _open = Clipboard::open()?;
            EmptyClipboard()?;
            let global = GlobalAlloc(GMEM_MOVEABLE, bytes)?;
            let target = GlobalLock(global) as *mut u16;
            if target.is_null() {
                return Err(AppError::Platform("out of memory".into()));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            let _ = GlobalUnlock(global);
            SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(global.0)))?;
        }
        Ok(())
    }

    /// The text in a picture, line by line, read on this PC in the user's languages.
    pub fn read_text(shot: &Shot) -> AppResult<String> {
        let shot = shot.for_ocr();
        let max = OcrEngine::MaxImageDimension()?;
        if shot.width > max || shot.height > max {
            return Err(AppError::InvalidInput("That area is too big to read.".into()));
        }
        let writer = DataWriter::new()?;
        writer.WriteBytes(&shot.bgra)?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            shot.width as i32,
            shot.height as i32,
        )?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
            AppError::Platform("No text recognition language is installed in Windows.".into())
        })?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;
        let mut lines = Vec::new();
        for line in result.Lines()? {
            lines.push(line.Text()?.to_string());
        }
        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_become_pixels_inside_the_picture() {
        assert_eq!(
            fraction_rect(1000, 500, 0.1, 0.2, 0.5, 0.5),
            Some(PixelRect {
                x: 100,
                y: 100,
                width: 500,
                height: 250
            })
        );
        // Dragged up and left, and past the edge.
        assert_eq!(
            fraction_rect(1000, 500, 0.5, 0.5, -0.7, -0.2),
            Some(PixelRect {
                x: 0,
                y: 150,
                width: 500,
                height: 100
            })
        );
        assert_eq!(fraction_rect(1000, 500, 0.5, 0.5, 0.0, 0.3), None);
        assert_eq!(fraction_rect(1000, 500, f64::NAN, 0.5, 0.1, 0.3), None);
    }

    fn shot(width: u32, height: u32) -> Shot {
        let bgra = (0..width * height).flat_map(|i| [i as u8, 0, 0, 255]).collect();
        Shot { width, height, bgra }
    }

    #[test]
    fn crops_rows_and_columns() {
        let cut = shot(4, 3).crop(PixelRect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        });
        assert_eq!((cut.width, cut.height), (2, 2));
        let blues: Vec<u8> = cut.bgra.chunks(4).map(|p| p[0]).collect();
        assert_eq!(blues, [5, 6, 9, 10]);
        // Past the edge: cut to fit.
        let edge = shot(4, 3).crop(PixelRect {
            x: 3,
            y: 2,
            width: 9,
            height: 9,
        });
        assert_eq!((edge.width, edge.height), (1, 1));
    }

    #[test]
    fn small_areas_are_scaled_up_for_reading() {
        let small = shot(200, 20).for_ocr();
        assert_eq!((small.width, small.height), (800, 80));
        let big = shot(300, 200).for_ocr();
        assert_eq!((big.width, big.height), (300, 200));
    }

    fn dib(width: i32, height: i32, bits: u16, pixels: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(&[0; 24]); // compression, size, resolution, colours
        out.extend_from_slice(pixels);
        out
    }

    #[test]
    fn reads_bottom_up_24_bit_bitmaps() {
        // 1×2, rows padded to 4 bytes; the bottom row comes first.
        let pixels = [1, 2, 3, 0, 4, 5, 6, 0];
        let shot = parse_dib(&dib(1, 2, 24, &pixels)).unwrap();
        assert_eq!((shot.width, shot.height), (1, 2));
        assert_eq!(shot.bgra, [4, 5, 6, 255, 1, 2, 3, 255]);
    }

    #[test]
    fn reads_top_down_32_bit_bitmaps_with_no_alpha_as_opaque() {
        let pixels = [1, 2, 3, 0, 4, 5, 6, 0];
        let shot = parse_dib(&dib(2, -1, 32, &pixels)).unwrap();
        assert_eq!(shot.bgra, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn rejects_odd_or_short_bitmaps() {
        assert_eq!(parse_dib(&dib(1, 1, 8, &[0; 4])), None);
        assert_eq!(parse_dib(&dib(4, 4, 32, &[0; 4])), None);
        assert_eq!(parse_dib(&[1, 2, 3]), None);
    }

    #[test]
    fn encodes_pictures() {
        let s = shot(3, 2);
        assert!(s.png_data_url().unwrap().starts_with("data:image/png;base64,"));
        assert!(s.jpeg_data_url().unwrap().starts_with("data:image/jpeg;base64,"));
    }
}
