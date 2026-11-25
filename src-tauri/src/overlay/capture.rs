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
    gpu: Option<super::gpu::Capture>,
}
impl Scanner {
    pub fn pause(&mut self) {
        self.gpu = None;
    }
    pub fn new() -> Result<Self, String> {
        let language = Language::CreateLanguage(&HSTRING::from("en-US"))
            .map_err(|_| "English text recognition is unavailable")?;
        let engine = OcrEngine::TryCreateFromLanguage(&language)
            .map_err(|_| "Install English OCR in Windows language options to use the overlay")?;
        Ok(Self { engine, gpu: None })
    }
    pub fn read(
        &mut self,
        hwnd: HWND,
        expected: Area,
        fullscreen: bool,
        evidence: Option<&std::path::Path>,
    ) -> Result<(Vec<Text>, i32, i32), String> {
        if unsafe { GetForegroundWindow() } != hwnd || area(hwnd) != Some(expected) {
            return Err("Target changed".into());
        }
        let factor = (1920.0 / expected.w as f64)
            .min(1080.0 / expected.h as f64)
            .min(1.0);
        let w = (expected.w as f64 * factor).round() as i32;
        let h = (expected.h as f64 * factor).round() as i32;
        let bytes = if fullscreen {
            if self.gpu.as_ref().is_none_or(|g| !g.matches(hwnd, expected)) {
                self.gpu = Some(super::gpu::Capture::new(hwnd, expected)?);
            }
            match self.gpu.as_mut().unwrap().read(expected, w, h) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.gpu = None;
                    return Err(error);
                }
            }
        } else {
            unsafe { grab(expected, w, h) }
                .ok_or("Screen capture is unavailable in this display mode")?
        };
        if unsafe { GetForegroundWindow() } != hwnd || area(hwnd) != Some(expected) {
            return Err("Target changed".into());
        }
        let recognized = self.recognize(&bytes, w, h)?;
        if fullscreen
            && recognized
                .0
                .iter()
                .any(|l| super::data::key(&l.text).contains("chooseanaugment"))
        {
            if let Some(path) = evidence.filter(|p| !p.exists()) {
                let mut file = Vec::with_capacity(bytes.len() + 54);
                file.extend_from_slice(b"BM");
                file.extend_from_slice(&((bytes.len() + 54) as u32).to_le_bytes());
                file.extend_from_slice(&[0; 4]);
                file.extend_from_slice(&54u32.to_le_bytes());
                file.extend_from_slice(&40u32.to_le_bytes());
                file.extend_from_slice(&w.to_le_bytes());
                file.extend_from_slice(&(-h).to_le_bytes());
                file.extend_from_slice(&1u16.to_le_bytes());
                file.extend_from_slice(&32u16.to_le_bytes());
                file.extend_from_slice(&[0; 24]);
                file.extend_from_slice(&bytes);
                let _ = std::fs::write(path, file);
            }
        }
        Ok(recognized)
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
            let mut words = Vec::new();
            for word in line.Words().map_err(|_| "Text recognition failed")? {
                let r = word.BoundingRect().map_err(|_| "Text recognition failed")?;
                words.push(Text {
                    text: word
                        .Text()
                        .map_err(|_| "Text recognition failed")?
                        .to_string(),
                    x: r.X,
                    y: r.Y,
                    w: r.Width,
                    h: r.Height,
                });
            }
            lines.extend(text_blocks(words, w as f32));
        }
        Ok((lines, w, h))
    }
}

// OCR sometimes merges columns. Split only at substantial spatial gaps,
// never into suffixes: damaged "Ultimate Recursion" must not become "Recursion".
fn text_blocks(words: Vec<Text>, width: f32) -> Vec<Text> {
    let mut blocks: Vec<Text> = Vec::new();
    for word in words {
        if let Some(last) = blocks
            .last_mut()
            .filter(|last| word.x - last.x - last.w <= width * 0.035)
        {
            let right = (last.x + last.w).max(word.x + word.w);
            let bottom = (last.y + last.h).max(word.y + word.h);
            last.x = last.x.min(word.x);
            last.y = last.y.min(word.y);
            last.w = right - last.x;
            last.h = bottom - last.y;
            last.text.push(' ');
            last.text.push_str(&word.text);
        } else {
            blocks.push(word);
        }
    }
    blocks
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
    fn merged_columns_split_without_exposing_title_fragments() {
        let words = [
            ("Ultilnatc", 100., 80.),
            ("Recursion", 190., 90.),
            ("ADAPt", 440., 60.),
            ("Twin", 740., 45.),
            ("Fire", 795., 35.),
        ]
        .map(|(text, x, w)| super::Text {
            text: text.into(),
            x,
            y: 220.,
            w,
            h: 25.,
        })
        .to_vec();
        let lines = super::text_blocks(words, 1000.);
        assert_eq!(
            lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["Ultilnatc Recursion", "ADAPt", "Twin Fire"]
        );
        let c = crate::overlay::data::Catalogue::bundled();
        assert!(
            c.resolve(&lines[0].text, true).is_none(),
            "an unreadable prefix must not silently become Recursion"
        );
    }
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
        assert!((640..=1920).contains(&width) && (400..=1080).contains(&height.abs()));
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
        .unwrap_or_else(|| {
            panic!(
                "Owned preview recognition failed: {:?}",
                lines
                    .iter()
                    .filter(|l| l.y > h as f32 * 0.3 && l.y < h as f32 * 0.5)
                    .map(|l| &l.text)
                    .collect::<Vec<_>>()
            )
        });
        assert_eq!(
            found.map(|c| c.id),
            [3055, 1205, 1415],
            "Owned fixture titles: {:?}",
            lines
                .iter()
                .filter(|l| l.y > h as f32 * 0.3 && l.y < h as f32 * 0.5)
                .map(|l| &l.text)
                .collect::<Vec<_>>()
        );
        drop(scanner);
        unsafe {
            windows::Win32::System::WinRT::RoUninitialize();
        }
    }
}
