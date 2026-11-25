//! Windows monitor capture, cropped in memory to the eligible foreground game.
//! No game injection or versioned offsets. Frames are never persisted in live mode.
use super::capture::Area;
use std::{sync::mpsc, time::Duration};
use windows::{
    core::{factory, Interface},
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::HMODULE,
        Graphics::{Direct3D::*, Direct3D11::*, Dxgi::IDXGIDevice},
        System::WinRT::{Direct3D11::*, Graphics::Capture::IGraphicsCaptureItemInterop},
    },
};
pub struct Capture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    staging: Option<ID3D11Texture2D>,
    size: (u32, u32),
    area: Area,
    hwnd: usize,
    frames: mpsc::Receiver<()>,
    event: i64,
    origin: (i32, i32),
}
impl Capture {
    pub fn new(hwnd: windows_sys::Win32::Foundation::HWND, area: Area) -> Result<Self, String> {
        unsafe {
            if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                return Err("Windows game capture is unavailable".into());
            }
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(|_| "Game capture graphics device is unavailable")?;
            let device = device.ok_or("Missing capture device")?;
            let dxgi: IDXGIDevice = device.cast().map_err(|_| "Invalid capture device")?;
            let direct: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .and_then(|v| v.cast())
                .map_err(|_| "Windows game capture could not initialize")?;
            let interop: IGraphicsCaptureItemInterop =
                factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                    .map_err(|_| "Window capture is unavailable")?;
            let monitor = windows_sys::Win32::Graphics::Gdi::MonitorFromWindow(
                hwnd,
                windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
            );
            let mut monitor_info = windows_sys::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: std::mem::size_of::<windows_sys::Win32::Graphics::Gdi::MONITORINFO>()
                    as u32,
                ..Default::default()
            };
            if windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut monitor_info) == 0 {
                return Err("Game monitor is unavailable".into());
            }
            let item: GraphicsCaptureItem = interop
                .CreateForMonitor(windows::Win32::Graphics::Gdi::HMONITOR(monitor))
                .map_err(|_| "Windows cannot capture this game monitor")?;
            let size = item.Size().map_err(|_| "Game window has no capture size")?;
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &direct,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                size,
            )
            .map_err(|_| "Could not create game frame pool")?;
            let (send, frames) = mpsc::sync_channel(1);
            let event = pool
                .FrameArrived(&TypedEventHandler::<
                    Direct3D11CaptureFramePool,
                    windows::core::IInspectable,
                >::new(move |_, _| {
                    let _ = send.try_send(());
                    Ok(())
                }))
                .map_err(|_| "Could not observe game frames")?;
            let session = pool
                .CreateCaptureSession(&item)
                .map_err(|_| "Could not create game capture session")?;
            let _ = session.SetIsCursorCaptureEnabled(false);
            session
                .StartCapture()
                .map_err(|_| "Windows denied game capture")?;
            Ok(Self {
                device,
                context: context.ok_or("Missing graphics context")?,
                pool,
                session,
                staging: None,
                size: (0, 0),
                area,
                hwnd: hwnd as usize,
                frames,
                event,
                origin: (monitor_info.rcMonitor.left, monitor_info.rcMonitor.top),
            })
        }
    }
    pub fn matches(&self, hwnd: windows_sys::Win32::Foundation::HWND, area: Area) -> bool {
        self.hwnd == hwnd as usize && self.area == area
    }
    pub fn read(&mut self, area: Area, width: i32, height: i32) -> Result<Vec<u8>, String> {
        unsafe {
            let _ = self.frames.recv_timeout(Duration::from_millis(150));
            let mut frame = self
                .pool
                .TryGetNextFrame()
                .map_err(|_| "Waiting for a game frame")?;
            while let Ok(newer) = self.pool.TryGetNextFrame() {
                let _ = frame.Close();
                frame = newer;
            }
            let result = (|| {
                let content = frame
                    .ContentSize()
                    .map_err(|_| "Game frame size unavailable")?;
                let left = area.x - self.origin.0;
                let top = area.y - self.origin.1;
                if left < 0
                    || top < 0
                    || left + area.w > content.Width
                    || top + area.h > content.Height
                {
                    return Err("Game display size changed; restarting capture".into());
                }
                let surface = frame.Surface().map_err(|_| "Game surface unavailable")?;
                let access: IDirect3DDxgiInterfaceAccess =
                    surface.cast().map_err(|_| "Game frame cannot be read")?;
                let texture: ID3D11Texture2D = access
                    .GetInterface()
                    .map_err(|_| "Game frame has no graphics texture")?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                texture.GetDesc(&mut desc);
                if self.size != (desc.Width, desc.Height) {
                    let staging = D3D11_TEXTURE2D_DESC {
                        Usage: D3D11_USAGE_STAGING,
                        BindFlags: 0,
                        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                        MiscFlags: 0,
                        ..desc
                    };
                    let mut saved = None;
                    self.device
                        .CreateTexture2D(&staging, None, Some(&mut saved))
                        .map_err(|_| "Could not allocate game frame buffer")?;
                    self.staging = saved;
                    self.size = (desc.Width, desc.Height);
                }
                let staging = self
                    .staging
                    .as_ref()
                    .ok_or("Game frame buffer unavailable")?;
                self.context.CopyResource(staging, &texture);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                self.context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|_| "Could not read game pixels")?;
                let mut bytes = vec![0u8; (width * height * 4) as usize];
                // Pixel-center bilinear sampling preserves title strokes when the
                // display is scaled. Nearest-neighbour aliasing changed OCR names.
                let columns: Vec<_> = (0..width)
                    .map(|x| {
                        let source = ((x as f64 + 0.5) * area.w as f64 / width as f64 - 0.5)
                            .clamp(0., (area.w - 1) as f64);
                        let a = source.floor() as usize;
                        (
                            a + left as usize,
                            (a + 1).min(area.w as usize - 1) + left as usize,
                            ((source - a as f64) * 256.).round() as u32,
                        )
                    })
                    .collect();
                for y in 0..height {
                    let source = ((y as f64 + 0.5) * area.h as f64 / height as f64 - 0.5)
                        .clamp(0., (area.h - 1) as f64);
                    let sy = source.floor() as usize;
                    let fy = ((source - sy as f64) * 256.).round() as u32;
                    let rows = [sy, (sy + 1).min(area.h as usize - 1)].map(|row| {
                        std::slice::from_raw_parts(
                            mapped
                                .pData
                                .cast::<u8>()
                                .add((top as usize + row) * mapped.RowPitch as usize),
                            desc.Width as usize * 4,
                        )
                    });
                    for (x, &(a, b, fx)) in columns.iter().enumerate() {
                        let dest = (y as usize * width as usize + x) * 4;
                        for c in 0..3 {
                            let upper = rows[0][a * 4 + c] as u32 * (256 - fx)
                                + rows[0][b * 4 + c] as u32 * fx;
                            let lower = rows[1][a * 4 + c] as u32 * (256 - fx)
                                + rows[1][b * 4 + c] as u32 * fx;
                            bytes[dest + c] =
                                ((upper * (256 - fy) + lower * fy + 32768) >> 16) as u8;
                        }
                        bytes[dest + 3] = 255;
                    }
                }
                self.context.Unmap(staging, 0);
                Ok(bytes)
            })();
            let _ = frame.Close();
            result
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.pool.RemoveFrameArrived(self.event);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
