use super::recognition::Text;
use windows::{
    core::HSTRING,
    Globalization::Language,
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Security::Cryptography::CryptographicBuffer,
};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::{GetClientRect, GetForegroundWindow, IsIconic},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
pub fn area(hwnd: HWND) -> Option<Area> {
    unsafe {
        if hwnd.is_null() || IsIconic(hwnd) != 0 {
            return None;
        }
        let mut rect = RECT::default();
        let mut origin = POINT::default();
        if GetClientRect(hwnd, &mut rect) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }
        let a = Area {
            x: origin.x,
            y: origin.y,
            w: rect.right,
            h: rect.bottom,
        };
        (a.w >= 640 && a.h >= 400 && a.w <= 10000 && a.h <= 10000).then_some(a)
    }
}

pub struct Scanner {
    engine: OcrEngine,
}
impl Scanner {
    pub fn new() -> Result<Self, String> {
        let language = Language::CreateLanguage(&HSTRING::from("en-US"))
            .map_err(|_| "English text recognition is unavailable")?;
        let engine = OcrEngine::TryCreateFromLanguage(&language)
            .map_err(|_| "Install English OCR in Windows language options to use the overlay")?;
        Ok(Self { engine })
    }
    pub fn read(&self, hwnd: HWND, expected: Area) -> Result<(Vec<Text>, i32, i32), String> {
        if unsafe { GetForegroundWindow() } != hwnd || area(hwnd) != Some(expected) {
            return Err("Target changed".into());
        }
        let factor = (1600.0 / expected.w as f64)
            .min(1000.0 / expected.h as f64)
            .min(1.0);
        let w = (expected.w as f64 * factor).round() as i32;
        let h = (expected.h as f64 * factor).round() as i32;
        let bytes = unsafe { grab(expected, w, h) }
            .ok_or("Screen capture is unavailable in this display mode")?;
        if unsafe { GetForegroundWindow() } != hwnd || area(hwnd) != Some(expected) {
            return Err("Target changed".into());
        }
        self.recognize(&bytes, w, h)
    }
    fn recognize(&self, bytes: &[u8], w: i32, h: i32) -> Result<(Vec<Text>, i32, i32), String> {
        let buffer = CryptographicBuffer::CreateFromByteArray(bytes)
            .map_err(|_| "Could not prepare text recognition")?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            w,
            h,
            BitmapAlphaMode::Ignore,
        )
        .map_err(|_| "Could not prepare text recognition")?;
        let result = self
            .engine
            .RecognizeAsync(&bitmap)
            .and_then(|op| op.join())
            .map_err(|_| "Text recognition failed")?;
        let mut lines = Vec::new();
        for line in result.Lines().map_err(|_| "Text recognition failed")? {
            let mut left = f32::MAX;
            let mut top = f32::MAX;
            let mut right = 0f32;
            let mut bottom = 0f32;
            let mut words = Vec::new();
            for word in line.Words().map_err(|_| "Text recognition failed")? {
                let r = word.BoundingRect().map_err(|_| "Text recognition failed")?;
                left = left.min(r.X);
                top = top.min(r.Y);
                right = right.max(r.X + r.Width);
                bottom = bottom.max(r.Y + r.Height);
                words.push((
                    word.Text()
                        .map_err(|_| "Text recognition failed")?
                        .to_string(),
                    r,
                ));
            }
            if right > left && bottom > top {
                lines.push(Text {
                    text: line
                        .Text()
                        .map_err(|_| "Text recognition failed")?
                        .to_string(),
                    x: left,
                    y: top,
                    w: right - left,
                    h: bottom - top,
                });
            }
            // OCR may group all three card titles into one line. Keep bounded word
            // phrases too, so matching never depends on its column segmentation.
            for start in 0..words.len() {
                let mut text = String::new();
                let mut bounds = words[start].1;
                for end in start..(start + 7).min(words.len()) {
                    let (word, r) = &words[end];
                    if end > start
                        && r.X - (words[end - 1].1.X + words[end - 1].1.Width) > w as f32 * 0.035
                    {
                        break;
                    }
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(word);
                    let right = (bounds.X + bounds.Width).max(r.X + r.Width);
                    let bottom = (bounds.Y + bounds.Height).max(r.Y + r.Height);
                    bounds.X = bounds.X.min(r.X);
                    bounds.Y = bounds.Y.min(r.Y);
                    bounds.Width = right - bounds.X;
                    bounds.Height = bottom - bounds.Y;
                    lines.push(Text {
                        text: text.clone(),
                        x: bounds.X,
                        y: bounds.Y,
                        w: bounds.Width,
                        h: bounds.Height,
                    });
                }
            }
        }
        Ok((lines, w, h))
    }
}

unsafe fn grab(area: Area, w: i32, h: i32) -> Option<Vec<u8>> {
    let source = GetDC(std::ptr::null_mut());
    if source.is_null() {
        return None;
    }
    let dest = CreateCompatibleDC(source);
    if dest.is_null() {
        ReleaseDC(std::ptr::null_mut(), source);
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let bitmap = CreateDIBSection(
        dest,
        &info,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if bitmap.is_null() {
        DeleteDC(dest);
        ReleaseDC(std::ptr::null_mut(), source);
        return None;
    }
    let old = SelectObject(dest, bitmap);
    SetStretchBltMode(dest, HALFTONE);
    let ok = StretchBlt(
        dest, 0, 0, w, h, source, area.x, area.y, area.w, area.h, SRCCOPY,
    ) != 0;
    let data = if ok && !bits.is_null() {
        Some(std::slice::from_raw_parts(bits.cast::<u8>(), (w * h * 4) as usize).to_vec())
    } else {
        None
    };
    SelectObject(dest, old);
    DeleteObject(bitmap);
    DeleteDC(dest);
    ReleaseDC(std::ptr::null_mut(), source);
    data
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Explicit local Windows OCR acceptance against an owned preview bitmap"]
    fn real_windows_ocr_reads_owned_preview() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".test")
            .canonicalize()
            .unwrap();
        let path = std::path::PathBuf::from(
            std::env::var_os("ORACLE_OVERLAY_BITMAP")
                .expect("Pass owned preview BMP through ORACLE_OVERLAY_BITMAP"),
        )
        .canonicalize()
        .unwrap();
        assert!(path.starts_with(root));
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[..2], b"BM");
        let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
        let height = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
        let bits = u16::from_le_bytes(bytes[28..30].try_into().unwrap()) as usize;
        assert!((640..=1600).contains(&width) && (400..=1000).contains(&height.abs()));
        assert!(bits == 24 || bits == 32);
        assert_eq!(u32::from_le_bytes(bytes[30..34].try_into().unwrap()), 0);
        let stride = (width as usize * bits).div_ceil(32) * 4;
        let mut pixels = vec![0u8; (width * height.abs() * 4) as usize];
        for y in 0..height.unsigned_abs() as usize {
            for x in 0..width as usize {
                let row = if height > 0 {
                    height as usize - 1 - y
                } else {
                    y
                };
                let p = offset + row * stride + x * (bits / 8);
                let target = (y * width as usize + x) * 4;
                pixels[target..target + 3].copy_from_slice(&bytes[p..p + 3]);
            }
        }
        unsafe {
            windows::Win32::System::WinRT::RoInitialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            )
            .unwrap();
        }
        let scanner = super::Scanner::new().unwrap();
        let (lines, w, h) = scanner.recognize(&pixels, width, height.abs()).unwrap();
        let found = crate::overlay::recognition::detect(
            &lines,
            &crate::overlay::data::Catalogue::bundled(),
            w as f32,
            h as f32,
        )
        .expect("The real preview's titles and prompt must be recognized");
        assert_eq!(found.map(|c| c.id), [3055, 1205, 1415]);
        drop(scanner);
        unsafe {
            windows::Win32::System::WinRT::RoUninitialize();
        }
    }
}
