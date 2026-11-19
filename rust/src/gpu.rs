use crate::config::Config;
use windows::{
    core::{s, Interface, Result},
    Win32::{
        Foundation::HMODULE,
        Graphics::{Direct3D::Fxc::*, Direct3D::*, Direct3D11::*, Dxgi::Common::*, Dxgi::*},
    },
};
pub struct Detector {
    pub monitor: isize,
    pub last_fresh: bool,
    owns_frame: bool,
    cached_roi: Option<((i32, i32), u32, [u32; 4])>,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    dup: IDXGIOutputDuplication,
    desc: DXGI_OUTPUT_DESC,
    shader: ID3D11ComputeShader,
    roi: ID3D11Texture2D,
    srv: ID3D11ShaderResourceView,
    result: ID3D11Buffer,
    read: ID3D11Buffer,
    uav: ID3D11UnorderedAccessView,
    params: ID3D11Buffer,
    sample: ID3D11Texture2D,
}
impl Drop for Detector {
    fn drop(&mut self) {
        unsafe {
            if self.owns_frame {
                let _ = self.dup.ReleaseFrame();
            }
            self.context.ClearState();
            self.context.Flush();
        }
    }
}

#[cfg(test)]
mod verification {
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn desktop_pixel_reaches_gpu_match_result() {
        unsafe {
            use windows_sys::Win32::{
                Foundation::POINT, Graphics::Gdi::*, UI::WindowsAndMessaging::*,
            };
            let mut point = POINT { x: 0, y: 0 };
            assert_ne!(GetCursorPos(&mut point), 0);
            let dc = GetDC(std::ptr::null_mut());
            let color = GetPixel(dc, point.x, point.y);
            ReleaseDC(std::ptr::null_mut(), dc);
            assert_ne!(color, 0xffffffff);
            let rgb = ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255);
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONULL);
            let mut detector =
                super::Detector::new(monitor as isize).expect("D3D11/DXGI/shader initialization");
            let cfg = crate::config::Config {
                rgb,
                tolerance: 2,
                radius: 1,
                ..Default::default()
            };
            for _ in 0..30 {
                match detector.frame((point.x, point.y), &cfg) {
                    Ok(Some(found)) => {
                        assert!(found, "actual desktop RGB #{rgb:06X} was not matched");
                        return;
                    }
                    Ok(None) => (),
                    Err(e) => panic!("capture/match error: {e:?}"),
                }
            }
            panic!("no acquired desktop frame");
        }
    }
}
impl Detector {
    pub unsafe fn new(monitor: isize) -> Result<Self> {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut ai = 0;
        loop {
            let adapter = factory.EnumAdapters1(ai)?;
            ai += 1;
            let mut oi = 0;
            while let Ok(output) = adapter.EnumOutputs(oi) {
                oi += 1;
                let desc = output.GetDesc()?;
                if desc.Monitor.0 as isize != monitor {
                    continue;
                }
                let mut device = None;
                let mut context = None;
                D3D11CreateDevice(
                    &adapter,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&[D3D_FEATURE_LEVEL_11_0]),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )?;
                let device = device.unwrap();
                let context = context.unwrap();
                let output: IDXGIOutput1 = output.cast()?;
                let dup = output.DuplicateOutput(&device)?;
                let code = include_bytes!("../assets/ColorDetect.hlsl");
                let mut compiled = None;
                D3DCompile(
                    code.as_ptr().cast(),
                    code.len(),
                    s!("ColorDetect.hlsl"),
                    None,
                    None,
                    s!("main"),
                    s!("cs_5_0"),
                    D3DCOMPILE_OPTIMIZATION_LEVEL3,
                    0,
                    &mut compiled,
                    None,
                )?;
                let blob = compiled.unwrap();
                let bytes = std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast::<u8>(),
                    blob.GetBufferSize(),
                );
                let mut shader = None;
                device.CreateComputeShader(bytes, None, Some(&mut shader))?;
                let td = D3D11_TEXTURE2D_DESC {
                    Width: 257,
                    Height: 257,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                };
                let mut roi = None;
                device.CreateTexture2D(&td, None, Some(&mut roi))?;
                let roi = roi.unwrap();
                let mut srv = None;
                device.CreateShaderResourceView(&roi, None, Some(&mut srv))?;
                let bd = D3D11_BUFFER_DESC {
                    ByteWidth: 4,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                    MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                    ..Default::default()
                };
                let mut result = None;
                device.CreateBuffer(&bd, None, Some(&mut result))?;
                let result = result.unwrap();
                let ud = D3D11_UNORDERED_ACCESS_VIEW_DESC {
                    Format: DXGI_FORMAT_R32_TYPELESS,
                    ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                    Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                        Buffer: D3D11_BUFFER_UAV {
                            FirstElement: 0,
                            NumElements: 1,
                            Flags: D3D11_BUFFER_UAV_FLAG_RAW.0 as u32,
                        },
                    },
                };
                let mut uav = None;
                device.CreateUnorderedAccessView(&result, Some(&ud), Some(&mut uav))?;
                let bd = D3D11_BUFFER_DESC {
                    ByteWidth: 4,
                    Usage: D3D11_USAGE_STAGING,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    ..Default::default()
                };
                let mut read = None;
                device.CreateBuffer(&bd, None, Some(&mut read))?;
                let bd = D3D11_BUFFER_DESC {
                    ByteWidth: 48,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                };
                let mut params = None;
                device.CreateBuffer(&bd, None, Some(&mut params))?;
                let td = D3D11_TEXTURE2D_DESC {
                    Width: 1,
                    Height: 1,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_STAGING,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    ..Default::default()
                };
                let mut sample = None;
                device.CreateTexture2D(&td, None, Some(&mut sample))?;
                return Ok(Self {
                    monitor,
                    last_fresh: false,
                    owns_frame: false,
                    cached_roi: None,
                    _device: device,
                    context,
                    dup,
                    desc,
                    shader: shader.unwrap(),
                    roi,
                    srv: srv.unwrap(),
                    result,
                    read: read.unwrap(),
                    uav: uav.unwrap(),
                    params: params.unwrap(),
                    sample: sample.unwrap(),
                });
            }
        }
    }
    unsafe fn release_owned_frame(&mut self) -> Result<()> {
        if self.owns_frame {
            self.owns_frame = false;
            self.dup.ReleaseFrame()?;
        }
        Ok(())
    }
    pub unsafe fn frame(&mut self, physical: (i32, i32), cfg: &Config) -> Result<Option<bool>> {
        self.last_fresh = false;
        self.release_owned_frame()?;
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        match self.dup.AcquireNextFrame(8, &mut info, &mut resource) {
            Ok(()) => {
                self.owns_frame = true;
                // Retain only our small copied ROI. Keep the duplication lease
                // until immediately before the next acquisition as DXGI advises.
                let copied = (|| -> Result<()> {
                    let frame: ID3D11Texture2D = resource.unwrap().cast()?;
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    frame.GetDesc(&mut desc);
                    let (mut x, mut y) = (
                        physical.0 - self.desc.DesktopCoordinates.left,
                        physical.1 - self.desc.DesktopCoordinates.top,
                    );
                    match self.desc.Rotation {
                        DXGI_MODE_ROTATION_ROTATE90 => (x, y) = (y, desc.Height as i32 - 1 - x),
                        DXGI_MODE_ROTATION_ROTATE180 => {
                            (x, y) = (desc.Width as i32 - 1 - x, desc.Height as i32 - 1 - y)
                        }
                        DXGI_MODE_ROTATION_ROTATE270 => (x, y) = (desc.Width as i32 - 1 - y, x),
                        _ => (),
                    }
                    let r = cfg.radius as i32;
                    let (left, top, right, bottom) = (
                        (x - r).max(0),
                        (y - r).max(0),
                        (x + r + 1).min(desc.Width as i32),
                        (y + r + 1).min(desc.Height as i32),
                    );
                    if right <= left || bottom <= top {
                        self.cached_roi = None;
                        return Ok(());
                    }
                    let w = (right - left) as u32;
                    let h = (bottom - top) as u32;
                    let box_ = D3D11_BOX {
                        left: left as u32,
                        top: top as u32,
                        front: 0,
                        right: right as u32,
                        bottom: bottom as u32,
                        back: 1,
                    };
                    self.context.CopySubresourceRegion(
                        &self.roi,
                        0,
                        0,
                        0,
                        0,
                        &frame,
                        0,
                        Some(&box_),
                    );
                    self.cached_roi = Some((
                        physical,
                        cfg.radius,
                        [w, h, (x - left) as u32, (y - top) as u32],
                    ));
                    Ok(())
                })();
                if let Err(e) = copied {
                    let _ = self.release_owned_frame();
                    return Err(e);
                }
                self.last_fresh = true;
            }
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                // DXGI explicitly confirms no desktop/pointer update. The cached
                // ROI is valid only at the identical physical point and radius.
                // Re-evaluate it with CURRENT RGB/tolerance, never count it as a
                // newly captured frame. Capture errors never enter this branch.
            }
            Err(e) => {
                self.cached_roi = None;
                return Err(e);
            }
        }
        let Some((at, radius, [w, h, cx, cy])) = self.cached_roi else {
            return Ok(None);
        };
        if at != physical || radius != cfg.radius {
            return Ok(None);
        }
        let p = [
            w,
            h,
            cx,
            cy,
            (cfg.rgb >> 16) & 255,
            (cfg.rgb >> 8) & 255,
            cfg.rgb & 255,
            cfg.tolerance,
            cfg.radius,
            0,
            0,
            0,
        ];
        self.context
            .UpdateSubresource(&self.params, 0, None, p.as_ptr().cast(), 0, 0);
        self.context
            .ClearUnorderedAccessViewUint(&self.uav, &[0; 4]);
        self.context.CSSetShader(&self.shader, None);
        self.context
            .CSSetShaderResources(0, Some(&[Some(self.srv.clone())]));
        self.context
            .CSSetUnorderedAccessViews(0, 1, Some([Some(self.uav.clone())].as_ptr()), None);
        self.context
            .CSSetConstantBuffers(0, Some(&[Some(self.params.clone())]));
        self.context.Dispatch((w + 7) / 8, (h + 7) / 8, 1);
        self.context.CSSetShaderResources(0, Some(&[None]));
        self.context
            .CSSetUnorderedAccessViews(0, 1, Some([None].as_ptr()), None);
        self.context.CopyResource(&self.read, &self.result);
        self.context.Flush();
        let mut map = D3D11_MAPPED_SUBRESOURCE::default();
        let started = std::time::Instant::now();
        loop {
            match self.context.Map(
                &self.read,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut map),
            ) {
                Ok(()) => break,
                Err(e)
                    if e.code() == DXGI_ERROR_WAS_STILL_DRAWING
                        && started.elapsed().as_millis() < 100 =>
                {
                    std::thread::yield_now()
                }
                Err(e) => return Err(e),
            }
        }
        let found = *(map.pData as *const u32) != 0;
        self.context.Unmap(&self.read, 0);
        Ok(Some(found))
    }
    pub unsafe fn sample_pixel(&mut self, physical: (i32, i32)) -> Result<Option<u32>> {
        self.cached_roi = None;
        self.release_owned_frame()?;
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        match self.dup.AcquireNextFrame(8, &mut info, &mut resource) {
            Ok(()) => (),
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
            Err(e) => return Err(e),
        }
        let result = (|| -> Result<Option<u32>> {
            let frame: ID3D11Texture2D = resource.unwrap().cast()?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            frame.GetDesc(&mut desc);
            let (mut x, mut y) = (
                physical.0 - self.desc.DesktopCoordinates.left,
                physical.1 - self.desc.DesktopCoordinates.top,
            );
            match self.desc.Rotation {
                DXGI_MODE_ROTATION_ROTATE90 => (x, y) = (y, desc.Height as i32 - 1 - x),
                DXGI_MODE_ROTATION_ROTATE180 => {
                    (x, y) = (desc.Width as i32 - 1 - x, desc.Height as i32 - 1 - y)
                }
                DXGI_MODE_ROTATION_ROTATE270 => (x, y) = (desc.Width as i32 - 1 - y, x),
                _ => (),
            }
            if x < 0 || y < 0 || x >= desc.Width as i32 || y >= desc.Height as i32 {
                return Ok(None);
            }
            let box_ = D3D11_BOX {
                left: x as u32,
                top: y as u32,
                front: 0,
                right: x as u32 + 1,
                bottom: y as u32 + 1,
                back: 1,
            };
            self.context
                .CopySubresourceRegion(&self.sample, 0, 0, 0, 0, &frame, 0, Some(&box_));
            self.context.Flush();
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            let start = std::time::Instant::now();
            loop {
                match self.context.Map(
                    &self.sample,
                    0,
                    D3D11_MAP_READ,
                    D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                    Some(&mut map),
                ) {
                    Ok(()) => break,
                    Err(e)
                        if e.code() == DXGI_ERROR_WAS_STILL_DRAWING
                            && start.elapsed().as_millis() < 100 =>
                    {
                        std::thread::yield_now()
                    }
                    Err(e) => return Err(e),
                }
            }
            let rgb = *(map.pData as *const u32) & 0xffffff;
            self.context.Unmap(&self.sample, 0);
            Ok(Some(rgb))
        })();
        self.dup.ReleaseFrame()?;
        result
    }
}
