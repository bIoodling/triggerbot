use crate::{
    render::{self, Canvas, Rect, BG, BORDER, DIM, PANEL, RED, SECOND, TEXT},
    trigger::Shared,
    wide,
};
use std::{
    ptr::{null, null_mut},
    sync::{atomic::Ordering, Arc},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{DataExchange::*, LibraryLoader::*, Memory::*, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, Input::*, WindowsAndMessaging::*},
};
static mut UI: *mut Ui = null_mut();
#[derive(Clone, Copy, PartialEq)]
enum View {
    Main,
    Settings,
    Install,
    Replace,
}
struct Ui {
    hwnd: HWND,
    picker: HWND,
    calibration: HWND,
    scale: f32,
    picker_scale: f32,
    view: View,
    shared: Arc<Shared>,
    capture: Option<usize>,
    blocked: u16,
    pressed: [bool; 256],
    draft: u32,
    hex: String,
    hex_focus: bool,
    select_all: bool,
    h: f64,
    s: f64,
    v: f64,
    drag: u8,
    eyedrop: bool,
    swallow: bool,
    payload: Option<&'static [u8]>,
    job: Option<Arc<crate::install::Job>>,
    last_status: u8,
    last_progress: u32,
    taskbar: u32,
}
fn hit(x: f32, y: f32, r: (f32, f32, f32, f32)) -> bool {
    x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3
}
fn bindings(i: usize) -> (f32, f32, f32, f32) {
    (240., 148. + i as f32 * 52., 136., 36.)
}
unsafe fn invalidate(hwnd: HWND) {
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn dimensions(u: &Ui) -> (f32, f32) {
    match u.view {
        View::Main => (360., 250.),
        View::Settings => (
            400.,
            if u.shared.control.lock().unwrap().config.fixed {
                462.
            } else {
                416.
            },
        ),
        View::Install => (500., 212.),
        View::Replace => (390., 200.),
    }
}
unsafe fn resize(u: &mut Ui) {
    let (w, h) = dimensions(u);
    let mut mi: MONITORINFO = std::mem::zeroed();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromWindow(u.hwnd, MONITOR_DEFAULTTONEAREST), &mut mi);
    u.scale = (GetDpiForWindow(u.hwnd) as f32 / 96.)
        .min((mi.rcWork.bottom - mi.rcWork.top - 20) as f32 / h)
        .min((mi.rcWork.right - mi.rcWork.left - 20) as f32 / w);
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(u.hwnd, &mut r);
    let width = (w * u.scale) as i32;
    let height = (h * u.scale) as i32;
    SetWindowPos(
        u.hwnd,
        null_mut(),
        r.left.clamp(mi.rcWork.left, mi.rcWork.right - width),
        r.top.clamp(mi.rcWork.top, mi.rcWork.bottom - height),
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    SetWindowRgn(
        u.hwnd,
        CreateRoundRectRgn(
            0,
            0,
            width + 1,
            height + 1,
            (18. * u.scale) as i32,
            (18. * u.scale) as i32,
        ),
        1,
    );
    invalidate(u.hwnd);
}
unsafe fn pause(u: &Ui, value: bool) {
    u.shared.change(|c| {
        c.suspended = value;
        c.held = false;
    });
}
fn button(g: &Canvas, label: &str, x: f32, y: f32, w: f32, h: f32, red: bool) {
    g.round(
        x,
        y,
        w,
        h,
        7.,
        if red { RED } else { PANEL },
        if red { RED } else { BORDER },
    );
    g.text(label, x + 4., y, w - 8., h, 16., TEXT, 400, true);
}
unsafe fn draw(u: &Ui, g: &Canvas, picker: bool) {
    if picker {
        g.round(0.7, 0.7, 298.6, 386.6, 11., BG, BORDER);
        g.close(273., 25.);
        for a in 0..360 {
            g.arc(
                49.,
                47.,
                202.,
                202.,
                -(a as f32),
                -1.5,
                0xff000000 | render::hsv(a as f64, 1., 1.),
                18.,
            );
        }
        let angle = (-u.h).to_radians();
        g.circle(
            150. + 101. * angle.cos() as f32,
            148. + 101. * angle.sin() as f32,
            8.,
            0xff000000 | render::hsv(u.h, 1., 1.),
            TEXT,
        );
        g.gradient(
            Rect {
                x: 96.,
                y: 94.,
                w: 108.,
                h: 108.,
            },
            0xffffffff,
            0xff000000 | render::hsv(u.h, 1., 1.),
            false,
        );
        g.gradient(
            Rect {
                x: 96.,
                y: 94.,
                w: 108.,
                h: 108.,
            },
            0,
            0xff000000,
            true,
        );
        g.circle(
            96. + u.s as f32 * 108.,
            94. + (1. - u.v) as f32 * 108.,
            6.,
            0,
            TEXT,
        );
        g.round(
            20.,
            278.,
            208.,
            38.,
            7.,
            PANEL,
            if crate::config::parse(&u.hex).is_some() {
                BORDER
            } else {
                RED
            },
        );
        g.text(&u.hex, 34., 278., 180., 38., 16., TEXT, 400, false);
        g.round(238., 278., 42., 38., 7., PANEL, BORDER);
        g.line(249., 304., 267., 286., TEXT, 2.);
        g.line(261., 285., 269., 293., TEXT, 2.);
        button(g, "Apply", 20., 330., 260., 38., true);
        return;
    }
    let (w, h) = dimensions(u);
    g.round(0.7, 0.7, w - 1.4, h - 1.4, 11., BG, BORDER);
    g.text("Yami", 24., 8., 180., 42., 23., TEXT, 600, false);
    g.close(w - 30., 28.);
    if u.view == View::Main || u.view == View::Install {
        g.line(w - 82., 28., w - 68., 28., TEXT, 1.2);
    }
    g.line(1., 56., w - 1., 56., BORDER, 0.7);
    match u.view {
        View::Main => {
            g.round(16., 66., 328., 54., 9., PANEL, BORDER);
            let status = u.shared.status.load(Ordering::Acquire);
            g.circle(
                38.,
                93.,
                7.,
                match status {
                    1 => RED,
                    2 => DIM,
                    _ => 0xff00dc23,
                },
                0,
            );
            g.text(
                match status {
                    1 => "Active",
                    2 => "Disabled",
                    _ => "Ready",
                },
                58.,
                66.,
                250.,
                54.,
                16.,
                TEXT,
                400,
                false,
            );
            let color = u.shared.control.lock().unwrap().config.rgb;
            g.text("Target colour", 20., 145., 165., 42., 16., TEXT, 500, false);
            g.round(200., 150., 32., 34., 6., 0xff000000 | color, 0);
            button(g, &crate::config::hex(color), 244., 150., 100., 34., false);
            g.line(1., 198., 359., 198., BORDER, 0.7);
            g.gear(31., 224.);
        }
        View::Settings => {
            g.round(1., 56., 398., 42., 0.1, PANEL, 0);
            g.line(30., 77., 44., 77., TEXT, 1.5);
            g.line(30., 77., 37., 70., TEXT, 1.5);
            g.line(30., 77., 37., 84., TEXT, 1.5);
            g.text("Back", 62., 56., 250., 42., 17., TEXT, 500, false);
            g.text("Keybinds", 26., 104., 350., 36., 18., TEXT, 600, false);
            let c = u.shared.control.lock().unwrap();
            for (i, label) in ["Toggle", "Hold", "Trigger"].iter().enumerate() {
                let y = 148. + i as f32 * 52.;
                g.text(label, 26., y, 200., 36., 17., TEXT, 500, false);
                let value = if u.capture == Some(i) {
                    "Press a key...".into()
                } else {
                    crate::input::name([c.config.toggle, c.config.hold, c.config.output][i])
                };
                button(g, &value, 240., y, 136., 36., false);
            }
            g.text(
                "Press Esc to unbind a key",
                26.,
                310.,
                350.,
                28.,
                13.,
                DIM,
                400,
                false,
            );
            g.line(24., 346., 376., 346., BORDER, 0.7);
            g.text("Fixed Point", 26., 360., 250., 44., 17., TEXT, 500, false);
            g.round(
                324.,
                369.,
                52.,
                28.,
                14.,
                if c.config.fixed { RED } else { SECOND },
                0,
            );
            g.circle(if c.config.fixed { 363. } else { 337. }, 383., 11., TEXT, 0);
            if c.config.fixed {
                button(
                    g,
                    if c.config.calibrated {
                        "Recalibrate"
                    } else {
                        "Calibrate"
                    },
                    26.,
                    414.,
                    350.,
                    32.,
                    false,
                );
            }
        }
        View::Install => {
            g.text("v1.0.6", 94., 10., 180., 40., 17., DIM, 400, false);
            let progress = u
                .job
                .as_ref()
                .map(|j| j.progress.load(Ordering::Acquire))
                .unwrap_or(0);
            g.text(
                if u.job.is_some() {
                    "Installing..."
                } else {
                    "Ready to install"
                },
                24.,
                70.,
                420.,
                38.,
                17.,
                TEXT,
                400,
                false,
            );
            g.round(24., 116., 405., 10., 5., SECOND, 0);
            if progress > 0 {
                g.round(24., 116., 405. * progress as f32 / 100., 10., 5., RED, 0);
            }
            g.text(
                &format!("{progress}%"),
                438.,
                99.,
                50.,
                42.,
                16.,
                TEXT,
                400,
                false,
            );
            if u.job.is_none() {
                button(g, "Install", 374., 157., 106., 36., false);
                g.round(374., 157., 106., 36., 7., BG, RED);
                g.text("Install", 374., 157., 106., 36., 16., TEXT, 400, true);
            }
        }
        View::Replace => {
            g.text(
                "Replace existing installation?",
                24.,
                67.,
                340.,
                32.,
                18.,
                TEXT,
                500,
                false,
            );
            g.text(
                "Current application files will be replaced.",
                24.,
                100.,
                345.,
                28.,
                13.,
                DIM,
                400,
                false,
            );
            button(g, "Cancel", 82., 148., 132., 40., false);
            button(g, "Replace", 228., 148., 136., 40., true);
        }
    }
}
unsafe fn paint(u: &Ui, hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let mut r: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut r);
    let mem = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, r.right.max(1), r.bottom.max(1));
    let old = SelectObject(mem, bitmap);
    {
        let g = Canvas::new(
            mem,
            if hwnd == u.picker {
                u.picker_scale
            } else {
                u.scale
            },
        );
        draw(u, &g, hwnd == u.picker);
    }
    BitBlt(dc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old);
    DeleteObject(bitmap);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}
unsafe fn close_picker(u: &mut Ui) {
    if !u.picker.is_null() {
        let p = u.picker;
        u.picker = null_mut();
        DestroyWindow(p);
    }
    u.hex_focus = false;
    u.eyedrop = false;
    u.drag = 0;
    ReleaseCapture();
    pause(u, false);
    invalidate(u.hwnd);
}
unsafe fn open_picker(u: &mut Ui) {
    if !u.picker.is_null() {
        SetForegroundWindow(u.picker);
        return;
    }
    pause(u, true);
    u.draft = u.shared.control.lock().unwrap().config.rgb;
    u.hex = crate::config::hex(u.draft);
    (u.h, u.s, u.v) = render::to_hsv(u.draft);
    u.picker_scale = GetDpiForWindow(u.hwnd) as f32 / 96.;
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(u.hwnd, &mut r);
    let mut mi: MONITORINFO = std::mem::zeroed();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromWindow(u.hwnd, MONITOR_DEFAULTTONEAREST), &mut mi);
    u.picker_scale = u
        .picker_scale
        .min((mi.rcWork.bottom - mi.rcWork.top - 20) as f32 / 388.)
        .min((mi.rcWork.right - mi.rcWork.left - 20) as f32 / 300.);
    let w = (300. * u.picker_scale) as i32;
    let h = (388. * u.picker_scale) as i32;
    let x = (r.right + 12).clamp(mi.rcWork.left, mi.rcWork.right - w);
    let y = r.top.clamp(mi.rcWork.top, mi.rcWork.bottom - h);
    let p = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        wide("Yami106Popup").as_ptr(),
        wide("Colour Picker").as_ptr(),
        WS_POPUP,
        x,
        y,
        w,
        h,
        u.hwnd,
        null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    u.picker = p;
    SetWindowRgn(p, CreateRoundRectRgn(0, 0, w + 1, h + 1, 18, 18), 1);
    ShowWindow(p, SW_SHOW);
    SetForegroundWindow(p);
    SetFocus(p);
}
unsafe fn commit(u: &mut Ui) {
    if let Some(c) = crate::config::parse(&u.hex) {
        u.shared.change(|control| control.config.rgb = c);
        u.shared.save();
        close_picker(u);
    }
}
unsafe fn picker_point(u: &mut Ui, x: f32, y: f32, first: bool) {
    let dx = x - 150.;
    let dy = y - 148.;
    let distance = (dx * dx + dy * dy).sqrt();
    if first {
        u.drag = if (89.0..=113.0).contains(&distance) {
            1
        } else if hit(x, y, (96., 94., 108., 108.)) {
            2
        } else {
            0
        };
    }
    if u.drag == 1 {
        u.h = (-(dy as f64).atan2(dx as f64).to_degrees()).rem_euclid(360.);
    } else if u.drag == 2 {
        u.s = ((x - 96.) as f64 / 108.).clamp(0., 1.);
        u.v = 1. - ((y - 94.) as f64 / 108.).clamp(0., 1.);
    }
    if u.drag != 0 {
        u.draft = render::hsv(u.h, u.s, u.v);
        u.hex = crate::config::hex(u.draft);
        invalidate(u.picker);
    }
}
unsafe fn bind(u: &mut Ui, key: u16) {
    if let Some(which) = u.capture.take() {
        let key = if key == 27 { 0 } else { key };
        u.shared.change(|c| {
            match which {
                0 => c.config.toggle = key,
                1 => c.config.hold = key,
                _ => c.config.output = key,
            }
            c.suspended = false;
            c.held = false;
        });
        u.blocked = key;
        u.shared.save();
        invalidate(u.hwnd);
    }
}
unsafe fn raw(u: &mut Ui, l: LPARAM) {
    let mut buffer = [0usize; 64];
    let mut length = std::mem::size_of_val(&buffer) as u32;
    if GetRawInputData(
        l as HRAWINPUT,
        RID_INPUT,
        buffer.as_mut_ptr().cast(),
        &mut length,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    ) == u32::MAX
    {
        return;
    }
    let raw = &*(buffer.as_ptr() as *const RAWINPUT);
    let (key, down) = if raw.header.dwType == RIM_TYPEKEYBOARD {
        let k = raw.data.keyboard;
        if k.ExtraInformation as usize == crate::input::TAG {
            return;
        }
        let scan = k.MakeCode as u32 | if k.Flags & 2 != 0 { 0xe000 } else { 0 };
        let mapped = MapVirtualKeyW(scan, MAPVK_VSC_TO_VK_EX);
        (
            if mapped == 0 { k.VKey } else { mapped as u16 },
            k.Flags & 1 == 0,
        )
    } else if raw.header.dwType == RIM_TYPEMOUSE {
        let m = raw.data.mouse;
        if m.ulExtraInformation as usize == crate::input::TAG {
            return;
        }
        let f = m.Anonymous.Anonymous.usButtonFlags;
        let mut item = None;
        for (a, b, key) in [
            (1, 2, 1),
            (4, 8, 2),
            (16, 32, 4),
            (64, 128, 5),
            (256, 512, 6),
        ] {
            if f & a != 0 {
                item = Some((key, true));
                break;
            }
            if f & b != 0 {
                item = Some((key, false));
                break;
            }
        }
        let Some(v) = item else {
            return;
        };
        v
    } else {
        return;
    };
    if key == 0 || key > 254 {
        return;
    }
    let was = u.pressed[key as usize];
    u.pressed[key as usize] = down;
    if u.blocked == key {
        if !down {
            u.blocked = 0;
        }
        return;
    }
    if u.capture.is_some() {
        if down {
            if key == 1 {
                let mut p = POINT { x: 0, y: 0 };
                GetCursorPos(&mut p);
                ScreenToClient(u.hwnd, &mut p);
                let index = u.capture.unwrap();
                if !hit(p.x as f32 / u.scale, p.y as f32 / u.scale, bindings(index)) {
                    u.capture = None;
                    pause(u, false);
                    invalidate(u.hwnd);
                    return;
                }
                u.swallow = true;
            }
            bind(u, key);
        }
        return;
    }
    if !u.picker.is_null() || !u.calibration.is_null() {
        return;
    }
    if down && [1, 2, 4, 5, 6].contains(&key) {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        if GetAncestor(WindowFromPoint(p), GA_ROOT) == u.hwnd {
            return;
        }
    }
    let mut c = u.shared.control.lock().unwrap();
    if down && key == c.config.toggle && !was {
        c.enabled = !c.enabled;
        c.generation += 1;
    }
    if key == c.config.hold {
        c.held = down;
        if !down {
            c.generation += 1;
        }
    }
    u.shared.wake.notify_all();
}
unsafe fn calibration(u: &mut Ui) {
    pause(u, true);
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        wide("Yami106Calibration").as_ptr(),
        wide("Yami calibration").as_ptr(),
        WS_POPUP,
        GetSystemMetrics(SM_XVIRTUALSCREEN),
        GetSystemMetrics(SM_YVIRTUALSCREEN),
        GetSystemMetrics(SM_CXVIRTUALSCREEN),
        GetSystemMetrics(SM_CYVIRTUALSCREEN),
        u.hwnd,
        null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    u.calibration = hwnd;
    SetLayeredWindowAttributes(hwnd, 0x00010101, 0, LWA_COLORKEY);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    SetCapture(hwnd);
}
unsafe extern "system" fn calibration_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if UI.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let u = &mut *UI;
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            let b = CreateSolidBrush(0x00010101);
            FillRect(dc, &r, b);
            DeleteObject(b);
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            ScreenToClient(hwnd, &mut p);
            let g = Canvas::new(dc, 1.);
            g.line(
                p.x as f32 - 12.,
                p.y as f32,
                p.x as f32 + 12.,
                p.y as f32,
                RED,
                2.,
            );
            g.line(
                p.x as f32,
                p.y as f32 - 12.,
                p.x as f32,
                p.y as f32 + 12.,
                RED,
                2.,
            );
            drop(g);
            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEMOVE => {
            invalidate(hwnd);
            0
        }
        WM_LBUTTONUP => {
            ShowWindow(hwnd, SW_HIDE);
            ReleaseCapture();
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            let target = GetAncestor(WindowFromPoint(p), GA_ROOT);
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(target, &mut r);
            let mut origin = POINT { x: 0, y: 0 };
            ClientToScreen(target, &mut origin);
            let exe = crate::capture::exe(target);
            if target != u.hwnd
                && !exe.is_empty()
                && r.right > 0
                && r.bottom > 0
                && p.x >= origin.x
                && p.x < origin.x + r.right
                && p.y >= origin.y
                && p.y < origin.y + r.bottom
            {
                u.shared.change(|c| {
                    c.config.x = (p.x - origin.x) as f64 / r.right as f64;
                    c.config.y = (p.y - origin.y) as f64 / r.bottom as f64;
                    c.config.target_exe = exe;
                    c.config.calibrated = true;
                    c.viewport = target as usize;
                });
                u.shared.save();
            }
            u.calibration = null_mut();
            DestroyWindow(hwnd);
            pause(u, false);
            SetForegroundWindow(u.hwnd);
            invalidate(u.hwnd);
            0
        }
        WM_KEYDOWN if w == 27 => {
            ShowWindow(hwnd, SW_HIDE);
            ReleaseCapture();
            u.calibration = null_mut();
            DestroyWindow(hwnd);
            pause(u, false);
            SetForegroundWindow(u.hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if UI.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let u = &mut *UI;
    if u.taskbar != 0 && msg == u.taskbar {
        crate::tray::add(u.hwnd);
        return 0;
    }
    let picker = hwnd == u.picker;
    let s = if picker { u.picker_scale } else { u.scale };
    let x = (l as i16) as f32 / s;
    let y = ((l >> 16) as i16) as f32 / s;
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(u, hwnd);
            0
        }
        WM_INPUT => {
            raw(u, l);
            DefWindowProcW(hwnd, msg, w, l)
        }
        WM_DPICHANGED => {
            if picker {
                close_picker(u);
            } else {
                resize(u);
            }
            0
        }
        WM_LBUTTONDOWN => {
            if picker {
                u.hex_focus = hit(x, y, (20., 278., 208., 38.));
                if u.hex_focus {
                    u.select_all = true;
                }
                picker_point(u, x, y, true);
                if u.drag != 0 {
                    SetCapture(hwnd);
                }
                return 0;
            }
            let (width, _) = dimensions(u);
            if y < 54. && x < width - 100. {
                ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, l);
            }
            0
        }
        WM_MOUSEMOVE => {
            if picker {
                if u.eyedrop {
                    let mut p = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut p);
                    let dc = GetDC(null_mut());
                    let c = GetPixel(dc, p.x, p.y);
                    ReleaseDC(null_mut(), dc);
                    if c != 0xffffffff {
                        u.draft = ((c & 255) << 16) | (c & 0xff00) | ((c >> 16) & 255);
                        u.hex = crate::config::hex(u.draft);
                        (u.h, u.s, u.v) = render::to_hsv(u.draft);
                    }
                    SetCursor(LoadCursorW(null_mut(), IDC_CROSS));
                    invalidate(hwnd);
                } else if u.drag != 0 {
                    picker_point(u, x, y, false);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            if u.swallow {
                u.swallow = false;
                return 0;
            }
            if picker {
                if u.eyedrop || u.drag != 0 {
                    u.eyedrop = false;
                    u.drag = 0;
                    ReleaseCapture();
                    return 0;
                }
                if hit(x, y, (256., 8., 36., 36.)) {
                    close_picker(u);
                } else if hit(x, y, (20., 330., 260., 38.)) {
                    commit(u);
                } else if hit(x, y, (238., 278., 42., 38.)) {
                    u.eyedrop = true;
                    SetCapture(hwnd);
                }
                return 0;
            }
            let (width, _) = dimensions(u);
            if hit(x, y, (width - 50., 8., 42., 40.)) {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                return 0;
            }
            if (u.view == View::Main || u.view == View::Install)
                && hit(x, y, (width - 100., 8., 40., 40.))
            {
                if u.payload.is_none() {
                    ShowWindow(hwnd, SW_HIDE);
                } else {
                    ShowWindow(hwnd, SW_MINIMIZE);
                }
                return 0;
            }
            match u.view {
                View::Main => {
                    if hit(x, y, (16., 204., 40., 40.)) {
                        u.view = View::Settings;
                        pause(u, true);
                        resize(u);
                    } else if hit(x, y, (196., 144., 152., 46.)) {
                        open_picker(u);
                    }
                }
                View::Settings => {
                    if hit(x, y, (0., 56., 400., 42.)) {
                        u.capture = None;
                        pause(u, false);
                        u.view = View::Main;
                        resize(u);
                    } else {
                        let mut bound = false;
                        for i in 0..3 {
                            if hit(x, y, bindings(i)) {
                                u.capture = Some(i);
                                pause(u, true);
                                bound = true;
                                break;
                            }
                        }
                        if !bound {
                            if u.capture.is_some() {
                                u.capture = None;
                                pause(u, true);
                            }
                            if hit(x, y, (318., 360., 64., 44.)) {
                                u.shared.change(|c| c.config.fixed = !c.config.fixed);
                                u.shared.save();
                                resize(u);
                            } else if u.shared.control.lock().unwrap().config.fixed
                                && hit(x, y, (26., 414., 350., 32.))
                            {
                                calibration(u);
                            }
                        }
                    }
                }
                View::Install => {
                    if u.job.is_none() && hit(x, y, (374., 157., 106., 36.)) {
                        if crate::install::exists() {
                            u.view = View::Replace;
                            resize(u);
                        } else {
                            u.job = Some(crate::install::start(u.payload.unwrap()));
                        }
                    }
                }
                View::Replace => {
                    if hit(x, y, (82., 148., 132., 40.)) {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    } else if hit(x, y, (228., 148., 136., 40.)) {
                        u.view = View::Install;
                        u.job = Some(crate::install::start(u.payload.unwrap()));
                        resize(u);
                    }
                }
            }
            invalidate(hwnd);
            0
        }
        WM_KEYDOWN => {
            if picker {
                if w == 27 {
                    close_picker(u);
                } else if w == 13 {
                    commit(u);
                } else if u.hex_focus && w == 8 {
                    if u.select_all {
                        u.hex.clear();
                        u.select_all = false;
                    } else {
                        u.hex.pop();
                    }
                    invalidate(hwnd);
                } else if u.hex_focus && w == 65 && crate::input::down(162) {
                    u.select_all = true;
                } else if u.hex_focus
                    && w == 86
                    && (crate::input::down(162) || crate::input::down(163))
                {
                    if OpenClipboard(hwnd) != 0 {
                        let data = GetClipboardData(13);
                        if !data.is_null() {
                            let p = GlobalLock(data) as *const u16;
                            if !p.is_null() {
                                let mut n = 0;
                                while *p.add(n) != 0 && n < 64 {
                                    n += 1;
                                }
                                u.hex = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                                GlobalUnlock(data);
                            }
                        }
                        CloseClipboard();
                        if let Some(c) = crate::config::parse(&u.hex) {
                            u.draft = c;
                            (u.h, u.s, u.v) = render::to_hsv(c);
                        }
                        invalidate(hwnd);
                    }
                }
            } else if w == 27 && u.capture.is_some() {
                bind(u, 27);
            }
            0
        }
        WM_CHAR => {
            if picker && u.hex_focus {
                if let Some(c) = char::from_u32(w as u32) {
                    if c == '#' || c.is_ascii_hexdigit() {
                        if u.select_all {
                            u.hex.clear();
                            u.select_all = false;
                        }
                        if u.hex.len() < 7 {
                            u.hex.push(c.to_ascii_uppercase());
                        }
                        if let Some(c) = crate::config::parse(&u.hex) {
                            u.draft = c;
                            (u.h, u.s, u.v) = render::to_hsv(c);
                        }
                        invalidate(hwnd);
                    }
                }
            }
            0
        }
        WM_ACTIVATE => {
            if w as u16 == WA_INACTIVE as u16 && u.capture.is_some() {
                u.capture = None;
                pause(u, u.view == View::Settings);
                invalidate(u.hwnd);
            }
            0
        }
        WM_TIMER => {
            let status = u.shared.status.load(Ordering::Acquire);
            if status != u.last_status {
                u.last_status = status;
                invalidate(u.hwnd);
            }
            if let Some(job) = u.job.clone() {
                let p = job.progress.load(Ordering::Acquire);
                if p != u.last_progress {
                    u.last_progress = p;
                    invalidate(u.hwnd);
                }
                let result = job.result.lock().unwrap().take();
                if let Some(result) = result {
                    u.job = None;
                    match result {
                        Ok(path) => {
                            let _ = std::process::Command::new(path).spawn();
                            DestroyWindow(u.hwnd);
                        }
                        Err(error) => {
                            MessageBoxW(
                                u.hwnd,
                                wide(&error).as_ptr(),
                                wide("Yami installation failed").as_ptr(),
                                MB_OK | MB_ICONERROR,
                            );
                            invalidate(u.hwnd);
                        }
                    }
                }
            }
            0
        }
        WM_SYSCOMMAND if w & 0xfff0 == SC_MINIMIZE as usize => {
            if u.payload.is_none() {
                ShowWindow(hwnd, SW_HIDE);
                0
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        crate::tray::CALLBACK => {
            if l as u32 == WM_LBUTTONDBLCLK {
                ShowWindow(u.hwnd, SW_RESTORE);
                SetForegroundWindow(u.hwnd);
            } else if l as u32 == WM_RBUTTONUP {
                crate::tray::menu(u.hwnd);
            }
            0
        }
        WM_CLOSE => {
            if picker {
                close_picker(u);
            } else if u.job.is_none() {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            if hwnd == u.hwnd {
                if u.payload.is_none() {
                    crate::tray::remove(hwnd);
                }
                u.shared.running.store(false, Ordering::Release);
                u.shared.wake.notify_all();
                PostQuitMessage(0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub fn run(payload: Option<&'static [u8]>) {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let mut mutex = null_mut();
        if payload.is_none() {
            mutex = CreateMutexW(
                null(),
                0,
                wide("Local\\Yami.Desktop.SingleInstance").as_ptr(),
            );
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let mut hwnd = FindWindowW(wide("Yami106Main").as_ptr(), null());
                if hwnd.is_null() {
                    hwnd = FindWindowW(wide("YamiMain").as_ptr(), null());
                }
                if !hwnd.is_null() {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                }
                if !mutex.is_null() {
                    CloseHandle(mutex);
                }
                return;
            }
        }
        let token = render::startup();
        let shared = Shared::new();
        let view = if payload.is_some() {
            View::Install
        } else {
            View::Main
        };
        let state = Box::new(Ui {
            hwnd: null_mut(),
            picker: null_mut(),
            calibration: null_mut(),
            scale: 1.,
            picker_scale: 1.,
            view,
            shared: shared.clone(),
            capture: None,
            blocked: 0,
            pressed: [false; 256],
            draft: 0x00ff00,
            hex: "#00FF00".into(),
            hex_focus: false,
            select_all: false,
            h: 120.,
            s: 1.,
            v: 1.,
            drag: 0,
            eyedrop: false,
            swallow: false,
            payload,
            job: None,
            last_status: 255,
            last_progress: 0,
            taskbar: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        });
        UI = Box::into_raw(state);
        for (name, proc) in [
            (
                "Yami106Main",
                Some(wndproc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
            (
                "Yami106Installer",
                Some(wndproc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
            (
                "Yami106Popup",
                Some(wndproc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
            (
                "Yami106Calibration",
                Some(calibration_proc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
        ] {
            let class_name = wide(name);
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: proc,
                hInstance: instance,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
        }
        let u = &mut *UI;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            wide(if payload.is_some() {
                "Yami106Installer"
            } else {
                "Yami106Main"
            })
            .as_ptr(),
            wide(if payload.is_some() {
                "Yami v1.0.6"
            } else {
                "Yami"
            })
            .as_ptr(),
            WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX,
            500,
            240,
            360,
            250,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        u.hwnd = hwnd;
        resize(u);
        let worker = if payload.is_none() {
            let devices = [
                RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
                RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 2,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
            ];
            if RegisterRawInputDevices(
                devices.as_ptr(),
                2,
                std::mem::size_of::<RAWINPUTDEVICE>() as u32,
            ) == 0
            {
                MessageBoxW(
                    hwnd,
                    wide("Input registration failed.").as_ptr(),
                    wide("Yami").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
                DestroyWindow(hwnd);
                None
            } else {
                crate::tray::add(hwnd);
                Some(crate::trigger::start(shared.clone()))
            }
        } else {
            None
        };
        ShowWindow(hwnd, SW_SHOW);
        SetTimer(hwnd, 1, 80, None);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        shared.running.store(false, Ordering::Release);
        shared.wake.notify_all();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        drop(Box::from_raw(UI));
        UI = null_mut();
        render::shutdown(token);
        if !mutex.is_null() {
            CloseHandle(mutex);
        }
    }
}
