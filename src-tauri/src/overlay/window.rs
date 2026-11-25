use super::capture::Area;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub id: u64,
    pub x: i32,
    pub y: i32,
    pub text: String,
}
#[derive(Default)]
struct Paint {
    labels: Vec<Label>,
    width: i32,
    height: i32,
}
pub struct Overlay {
    pub hwnd: HWND,
    data: Box<Paint>,
    area: Option<Area>,
}
impl Overlay {
    pub fn new() -> Option<Self> {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = windows_sys::core::w!("OracleAugmentOverlay");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let mut data = Box::<Paint>::default();
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST,
                class,
                windows_sys::core::w!("Oracle augment statistics"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (&mut *data as *mut Paint).cast(),
            );
            if hwnd.is_null() {
                return None;
            }
            if SetLayeredWindowAttributes(hwnd, 0, 255, LWA_COLORKEY | LWA_ALPHA) == 0 {
                DestroyWindow(hwnd);
                return None;
            }
            Some(Self {
                hwnd,
                data,
                area: None,
            })
        }
    }
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }
    pub fn show(&mut self, area: Area, labels: Vec<Label>) -> bool {
        unsafe {
            let changed = self.area != Some(area)
                || self.data.labels != labels
                || IsWindowVisible(self.hwnd) == 0;
            self.area = Some(area);
            self.data.labels = labels;
            self.data.width = area.w;
            self.data.height = area.h;
            let positioned = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                area.x,
                area.y,
                area.w,
                area.h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            ) != 0;
            if changed {
                InvalidateRect(self.hwnd, std::ptr::null(), 0);
                UpdateWindow(self.hwnd);
            }
            positioned
        }
    }
    pub fn pump() {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut client = RECT::default();
            GetClientRect(hwnd, &mut client);
            FillRect(dc, &client, GetStockObject(BLACK_BRUSH) as _);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Paint;
            if !ptr.is_null() {
                paint(dc, &*ptr);
            }
            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe fn paint(dc: HDC, data: &Paint) {
    let scale = (data.width as f64 / 1280.0).clamp(0.75, 2.0);
    let height = (34.0 * scale) as i32;
    let width = (data.width as f64 * 0.19) as i32;
    let font = CreateFontW(
        -(15.0 * scale) as i32,
        0,
        0,
        0,
        500,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        ANTIALIASED_QUALITY as u32,
        DEFAULT_PITCH as u32,
        windows_sys::core::w!("Segoe UI"),
    );
    let old = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, 0x00c5ddf3);
    let background = CreateSolidBrush(0x001a100f);
    let edge = CreateSolidBrush(0x006e9abe);
    for label in &data.labels {
        let mut rect = RECT {
            left: (label.x - width / 2).clamp(0, (data.width - width).max(0)),
            top: label.y.clamp(0, (data.height - height).max(0)),
            right: 0,
            bottom: 0,
        };
        rect.right = rect.left + width;
        rect.bottom = rect.top + height;
        FillRect(dc, &rect, background);
        FrameRect(dc, &rect, edge);
        rect.left += 5;
        rect.right -= 5;
        let text: Vec<u16> = label.text.encode_utf16().collect();
        DrawTextW(
            dc,
            text.as_ptr(),
            text.len() as i32,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
    SelectObject(dc, old);
    DeleteObject(font);
    DeleteObject(background);
    DeleteObject(edge);
}
