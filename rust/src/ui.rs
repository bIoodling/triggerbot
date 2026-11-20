use crate::{
    render::{self, Canvas, Rect, ACCENT, BG, BORDER, DIM, HOVER, PANEL, RED, SECOND, TEXT},
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
    System::{LibraryLoader::*, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, Input::*, WindowsAndMessaging::*},
};
static mut UI: *mut Ui = null_mut();
#[derive(Clone, Copy, PartialEq)]
enum ScreenPick {
    Calibration,
    Colour,
}
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
    marker: Option<crate::calibration::Marker>,
    cursor_hide_calls: u32,
    screen_pick: Option<ScreenPick>,
    screen_pick_previous_focus: HWND,
    screen_pick_started: std::time::Instant,
    screen_pick_was_picker: bool,
    mouse_hook: HHOOK,
    keyboard_hook: HHOOK,
    sample_id: u64,
    sample_point: Option<(i32, i32)>,
    selection_point: Option<(i32, i32)>,
    pending_pick: Option<(std::time::Instant, u64, (i32, i32))>,
    confirm: HWND,
    confirm_exit_selected: bool,
    confirm_scale: f32,
    suppressed: [bool; 256],
    fade: crate::windowing::Fade,
    shutting_down: bool,
    displayed_progress: f32,
    progress_tick: std::time::Instant,
    pending_launch: Option<std::path::PathBuf>,
    radius_focus: bool,
    radius_text: String,
    hover: Option<(f32, f32)>,
    pressed_ui: bool,
    animate: bool,
    hover_motion: std::cell::RefCell<std::collections::HashMap<u64, crate::motion::Tween>>,
    toggle_motion: crate::motion::Tween,
    page_motion: crate::motion::Tween,
    panel_motion: crate::motion::Tween,
    panel_closing: bool,
    panel_left: bool,
    panel_extent_px: i32,
    closing: Option<std::time::Instant>,
    hex_edit: HWND,
    hex_syncing: bool,
    hex_font: HFONT,
    hex_font_scale: f32,
    hex_brush: HBRUSH,
    hex_original_proc: WNDPROC,
    scale: f32,
    picker_scale: f32,
    view: View,
    shared: Arc<Shared>,
    capture: Option<usize>,
    capture_candidate: Option<u16>,
    capture_invalid: bool,
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
    (240., 176. + i as f32 * 56., 136., 36.)
}
unsafe fn invalidate(hwnd: HWND) {
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn dimensions(u: &Ui) -> (f32, f32) {
    match u.view {
        View::Main | View::Settings => (400. + 248. * u.panel_motion.value(), 420.),
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
        .min(
            (mi.rcWork.right - mi.rcWork.left - 20) as f32
                / if u.payload.is_none() { 648. } else { w },
        );
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
    if u.payload.is_some() {
        crate::windowing::center(u.hwnd, MonitorFromWindow(u.hwnd, MONITOR_DEFAULTTONEAREST));
    }
    invalidate(u.hwnd);
}
unsafe fn pause(u: &Ui, value: bool) {
    u.shared.change(|c| {
        c.suspended = value;
        c.held = false;
    });
}
fn button(u: &Ui, g: &Canvas, label: &str, x: f32, y: f32, w: f32, h: f32, red: bool) {
    let hovered = u
        .hover
        .map(|(px, py)| hit(px, py, (x, y, w, h)))
        .unwrap_or(false);
    let id = ((x as u64) << 40) | ((y as u64) << 24) | ((w as u64) << 8) | u.view as u64;
    let mut motions = u.hover_motion.borrow_mut();
    let motion = motions
        .entry(id)
        .or_insert_with(|| crate::motion::Tween::new(0.));
    motion.set(
        if hovered {
            if u.pressed_ui {
                0.65
            } else {
                1.
            }
        } else {
            0.
        },
        if u.animate { 150 } else { 0 },
    );
    let t = motion.value();
    g.round(
        x,
        y,
        w,
        h,
        7.,
        crate::motion::mix(
            if red { ACCENT } else { PANEL },
            if red { HOVER } else { SECOND },
            t,
        ),
        crate::motion::mix(if red { ACCENT } else { BORDER }, HOVER, t),
    );
    g.text(label, x + 4., y, w - 8., h, 16., TEXT, 400, true);
}
unsafe fn draw(u: &Ui, g: &Canvas, picker: bool) {
    if picker {
        g.round(0.7, 0.7, 298.6, 386.6, 11., BG, BORDER);
        g.close(273., 25.);
        for a in 0..260 {
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
        // Native EDIT owns text, caret, selection, clipboard and navigation.
        g.round(238., 278., 42., 38., 7., PANEL, BORDER);
        g.pen(259., 297.);
        button(u, g, "Apply", 20., 330., 260., 38., true);
        let fade = (1. - u.panel_motion.value()).clamp(0., 1.);
        g.round(
            0.,
            0.,
            300.,
            388.,
            0.1,
            ((fade * 255.) as u32) << 24 | (BG & 0xffffff),
            0,
        );
        return;
    }
    let (mut w, h) = dimensions(u);
    if u.payload.is_none() {
        w = 400.;
        if u.panel_left {
            g.translate(u.panel_extent_px as f32 / u.scale, 0.);
        }
    }
    g.round(0.7, 0.7, w - 1.4, h - 1.4, 11., BG, BORDER);
    g.text("Yami", 24., 8., 180., 42., 23., TEXT, 600, false);
    g.close(w - 30., 28.);
    if u.view == View::Main || u.view == View::Install {
        g.line(w - 82., 28., w - 68., 28., TEXT, 1.2);
    }
    g.line(1., 56., w - 1., 56., BORDER, 0.7);
    match u.view {
        View::Main => {
            g.round(20., 80., 360., 64., 10., PANEL, BORDER);
            let status = u.shared.status.load(Ordering::Acquire);
            g.circle(
                38.,
                112.,
                7.,
                match status {
                    1 => ACCENT,
                    2 => DIM,
                    _ => 0xff00dc23,
                },
                0,
            );
            g.text(
                match status {
                    1 => "Active",
                    2 => "Disabled",
                    3 => "Set Hold binding",
                    4 => "Select target window",
                    _ => "Ready",
                },
                58.,
                80.,
                290.,
                64.,
                16.,
                TEXT,
                400,
                false,
            );
            let color = u.shared.control.lock().unwrap().config.rgb;
            g.text("Target colour", 24., 186., 165., 42., 16., TEXT, 500, false);
            g.round(236., 190., 32., 34., 6., 0xff000000 | color, 0);
            button(
                u,
                g,
                &crate::config::hex(color),
                280.,
                190.,
                96.,
                34.,
                false,
            );
            g.line(20., 354., 380., 354., BORDER, 0.7);
            g.gear(36., 386.);
            g.text("Settings", 62., 364., 220., 44., 16., DIM, 400, false);
        }
        View::Settings => {
            g.line(30., 77., 44., 77., TEXT, 1.5);
            g.line(30., 77., 37., 70., TEXT, 1.5);
            g.line(30., 77., 37., 84., TEXT, 1.5);
            g.text("Back", 62., 56., 250., 42., 17., TEXT, 500, false);
            g.text("Keybinds", 26., 104., 350., 36., 18., TEXT, 600, false);
            let c = u.shared.control.lock().unwrap();
            for (i, label) in ["Toggle", "Hold"].iter().enumerate() {
                let y = 176. + i as f32 * 56.;
                g.text(label, 26., y, 200., 36., 17., TEXT, 500, false);
                let value = if u.capture == Some(i) {
                    "Press a key...".into()
                } else {
                    crate::input::name([c.config.toggle, c.config.hold][i])
                };
                button(u, g, &value, 240., y, 136., 36., false);
            }
            g.text(
                "Press Esc to unbind a key",
                26.,
                138.,
                350.,
                28.,
                13.,
                DIM,
                400,
                false,
            );
            g.text("Radius", 26., 292., 180., 36., 16., TEXT, 400, false);
            let radius_valid = u
                .radius_text
                .parse::<u32>()
                .map(|n| (1..=128).contains(&n))
                .unwrap_or(false);
            g.round(
                240.,
                292.,
                136.,
                36.,
                6.,
                PANEL,
                if !radius_valid {
                    RED
                } else if u.radius_focus {
                    ACCENT
                } else {
                    BORDER
                },
            );
            g.text(&u.radius_text, 244., 292., 128., 36., 16., TEXT, 400, true);
            g.line(24., 358., 376., 358., BORDER, 0.7);
            g.text("Fixed Point", 26., 370., 140., 36., 17., TEXT, 500, false);
            g.round(
                324.,
                374.,
                52.,
                28.,
                14.,
                crate::motion::mix(SECOND, ACCENT, u.toggle_motion.value()),
                0,
            );
            g.circle(337. + 26. * u.toggle_motion.value(), 388., 11., TEXT, 0);
            if c.config.fixed {
                button(
                    u,
                    g,
                    if c.config.calibrated {
                        "Recalibrate"
                    } else {
                        "Calibrate"
                    },
                    180.,
                    372.,
                    130.,
                    32.,
                    false,
                );
            }
        }
        View::Install => {
            g.text("v1.0.6", 94., 10., 180., 40., 17., DIM, 400, false);
            let progress = u.displayed_progress.round() as u32;
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
                g.round(24., 116., 405. * progress as f32 / 100., 10., 5., ACCENT, 0);
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
                button(u, g, "Install", 374., 157., 106., 36., false);
                g.round(374., 157., 106., 36., 7., BG, ACCENT);
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
            button(u, g, "Cancel", 82., 148., 132., 40., false);
            button(u, g, "Replace", 228., 148., 136., 40., true);
        }
    }
    if u.payload.is_none() {
        let a = ((1. - u.page_motion.value()) * 255.).clamp(0., 255.) as u32;
        if a > 0 {
            g.round(1., 57., 398., 362., 0.1, (a << 24) | (BG & 0xffffff), 0);
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
            } else if hwnd == u.confirm {
                u.confirm_scale
            } else {
                u.scale
            },
        );
        if hwnd == u.confirm {
            draw_exit(u, &g);
        } else {
            draw(u, &g, hwnd == u.picker);
        }
    }
    BitBlt(dc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old);
    DeleteObject(bitmap);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}
unsafe fn layout_panel(u: &mut Ui) {
    if u.payload.is_some() {
        return;
    }
    let extent = (248. * u.scale * u.panel_motion.value()).round() as i32;
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(u.hwnd, &mut r);
    let work = crate::windowing::work_area(MonitorFromWindow(u.hwnd, MONITOR_DEFAULTTONEAREST));
    let width = (400. * u.scale).round() as i32 + extent;
    let x = (if u.panel_left {
        r.left + u.panel_extent_px - extent
    } else {
        r.left
    })
    .clamp(work.left, (work.right - width).max(work.left));
    u.panel_extent_px = extent;
    SetWindowPos(
        u.hwnd,
        null_mut(),
        x,
        r.top,
        width,
        (420. * u.scale).round() as i32,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    SetWindowRgn(
        u.hwnd,
        CreateRoundRectRgn(
            0,
            0,
            width + 1,
            (420. * u.scale).round() as i32 + 1,
            (18. * u.scale) as i32,
            (18. * u.scale) as i32,
        ),
        1,
    );
    if !u.picker.is_null() {
        u.picker_scale = u.scale * 0.76;
        let px = if u.panel_left {
            extent - (238. * u.scale) as i32
        } else {
            (410. * u.scale) as i32
        };
        MoveWindow(
            u.picker,
            px,
            ((420. - 388. * 0.76) / 2. * u.scale) as i32,
            (300. * u.picker_scale) as i32,
            (388. * u.picker_scale) as i32,
            1,
        );
        if !u.hex_edit.is_null() {
            if (u.hex_font_scale - u.picker_scale).abs() > 0.001 {
                let font = CreateFontW(
                    -(16. * u.picker_scale).round() as i32,
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    5,
                    0,
                    wide("Segoe UI").as_ptr(),
                );
                if !font.is_null() {
                    SendMessageW(u.hex_edit, WM_SETFONT, font as usize, 1);
                    if !u.hex_font.is_null() {
                        DeleteObject(u.hex_font);
                    }
                    u.hex_font = font;
                    u.hex_font_scale = u.picker_scale;
                }
            }
            MoveWindow(
                u.hex_edit,
                (34. * u.picker_scale) as i32,
                (286. * u.picker_scale) as i32,
                (180. * u.picker_scale) as i32,
                (22. * u.picker_scale) as i32,
                1,
            );
            ShowWindow(
                u.hex_edit,
                if !u.panel_motion.active() && !u.panel_closing {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
        }
        invalidate(u.picker);
    }
    invalidate(u.hwnd);
}
unsafe fn destroy_picker(u: &mut Ui) {
    let p = u.picker;
    u.picker = null_mut();
    u.hex_edit = null_mut();
    if !p.is_null() {
        DestroyWindow(p);
    }
    if !u.hex_font.is_null() {
        DeleteObject(u.hex_font);
        u.hex_font = null_mut();
    }
    if !u.hex_brush.is_null() {
        DeleteObject(u.hex_brush);
        u.hex_brush = null_mut();
    }
    u.hex_original_proc = None;
    u.panel_motion.reset(0.);
    u.panel_closing = false;
    u.hex_focus = false;
    u.eyedrop = false;
    u.drag = 0;
    ReleaseCapture();
    layout_panel(u);
    pause(
        u,
        u.capture.is_some() || !u.confirm.is_null() || u.screen_pick.is_some() || u.radius_focus,
    );
}
unsafe fn close_picker(u: &mut Ui) {
    if u.picker.is_null() {
        return;
    }
    u.panel_closing = true;
    u.panel_motion.set(0., if u.animate { 170 } else { 0 });
    u.hex_focus = false;
    u.drag = 0;
    ReleaseCapture();
    EnableWindow(u.picker, 0);
    SetFocus(u.hwnd);
    SetTimer(u.hwnd, 5, 16, None);
}
unsafe extern "system" fn hex_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if UI.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_KEYDOWN {
        if w == 13 {
            commit(&mut *UI);
            return 0;
        }
        if w == 27 {
            close_picker(&mut *UI);
            return 0;
        }
        if w == 65 && (crate::input::down(162) || crate::input::down(163)) {
            SendMessageW(hwnd, 0x00b1, 0, -1);
            return 0;
        }
    }
    if msg == WM_CHAR
        && w >= 32
        && !char::from_u32(w as u32)
            .map(|c| c == '#' || c.is_ascii_hexdigit())
            .unwrap_or(false)
    {
        return 0;
    }
    let original = (*UI).hex_original_proc;
    CallWindowProcW(original, hwnd, msg, w, l)
}
unsafe fn open_picker(u: &mut Ui) {
    if !u.picker.is_null() {
        SetFocus(u.hex_edit);
        return;
    }
    pause(u, true);
    u.draft = u.shared.control.lock().unwrap().config.rgb;
    u.hex = crate::config::hex(u.draft);
    (u.h, u.s, u.v) = render::to_hsv(u.draft);
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(u.hwnd, &mut r);
    let work = crate::windowing::work_area(MonitorFromWindow(u.hwnd, MONITOR_DEFAULTTONEAREST));
    u.panel_left =
        work.right - r.right < (248. * u.scale) as i32 && r.left - work.left > work.right - r.right;
    u.panel_closing = false;
    u.panel_motion.reset(0.);
    u.panel_motion.set(1., if u.animate { 170 } else { 0 });
    u.picker_scale = u.scale * 0.76;
    u.picker = CreateWindowExW(
        0,
        wide("Yami106Popup").as_ptr(),
        wide("Colour Picker").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
        0,
        0,
        1,
        1,
        u.hwnd,
        null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    if u.picker.is_null() {
        u.panel_motion.reset(0.);
        pause(u, false);
        return;
    }
    u.hex_edit = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wide(&u.hex).as_ptr(),
        WS_CHILD | WS_TABSTOP | 0x0080,
        0,
        0,
        1,
        1,
        u.picker,
        801usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    if u.hex_edit.is_null() {
        destroy_picker(u);
        return;
    }
    u.hex_font = CreateFontW(
        -(16. * u.picker_scale).round() as i32,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        wide("Segoe UI").as_ptr(),
    );
    u.hex_font_scale = u.picker_scale;
    SendMessageW(u.hex_edit, WM_SETFONT, u.hex_font as usize, 1);
    SendMessageW(u.hex_edit, 0x00c5, 7, 0); // EM_SETLIMITTEXT
    let old = SetWindowLongPtrW(u.hex_edit, GWLP_WNDPROC, hex_proc as *const () as isize);
    if old == 0 {
        destroy_picker(u);
        return;
    }
    u.hex_original_proc = Some(std::mem::transmute::<
        isize,
        unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    >(old));
    u.hex_brush = CreateSolidBrush(0x00231919);
    layout_panel(u);
    SetTimer(u.hwnd, 5, 16, None);
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
        if !u.hex_edit.is_null() {
            u.hex_syncing = true;
            SetWindowTextW(u.hex_edit, wide(&u.hex).as_ptr());
            u.hex_syncing = false;
        }
        invalidate(u.picker);
    }
}
unsafe fn bind(u: &mut Ui, key: u16) {
    if let Some(which) = u.capture.take() {
        let key = if key == 27 { 0 } else { key };
        if key != 0 {
            let c = u.shared.control.lock().unwrap();
            let other = if which == 0 {
                c.config.hold
            } else {
                c.config.toggle
            };
            if !crate::input::binding_allowed(key) || key == other {
                return;
            }
        }
        u.shared.change(|c| {
            match which {
                0 => c.config.toggle = key,
                1 => c.config.hold = key,
                _ => (),
            }
            c.suspended = false;
            c.held = false;
        });
        u.blocked = if crate::input::down(key) { key } else { 0 };
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
        for (a, b, key) in [
            (1, 2, 1),
            (4, 8, 2),
            (16, 32, 4),
            (64, 128, 5),
            (256, 512, 6),
        ] {
            if f & a != 0 {
                raw_edge(u, key, true);
            }
            if f & b != 0 {
                raw_edge(u, key, false);
            }
        }
        return;
    } else {
        return;
    };
    raw_edge(u, key, down);
}
unsafe fn raw_edge(u: &mut Ui, key: u16, down: bool) {
    if key == 0 || key > 254 {
        return;
    }
    if !u.confirm.is_null() || u.screen_pick.is_some() {
        u.suppressed[key as usize] = down;
        return;
    }
    if u.suppressed[key as usize] {
        if !down {
            u.suppressed[key as usize] = false;
        }
        return;
    }

    u.pressed[key as usize] = down;
    if u.blocked == key {
        if !down {
            u.blocked = 0;
        }
        return;
    }
    if let Some(index) = u.capture {
        if down && key == 27 {
            bind(u, key);
            return;
        }
        if key == 1 {
            if down {
                let mut p = POINT { x: 0, y: 0 };
                GetCursorPos(&mut p);
                ScreenToClient(u.hwnd, &mut p);
                if !hit(p.x as f32 / u.scale, p.y as f32 / u.scale, bindings(index)) {
                    u.capture = None;
                    u.capture_candidate = None;
                    pause(u, false);
                    invalidate(u.hwnd);
                }
            }
            return;
        }
        if down {
            let other = {
                let c = u.shared.control.lock().unwrap();
                if index == 0 {
                    c.config.hold
                } else {
                    c.config.toggle
                }
            };
            let held = (1..255)
                .filter(|k| u.pressed[*k as usize] || crate::input::down(*k as u16))
                .map(|k| k as u16);
            if u.capture_candidate.is_some() && u.capture_candidate != Some(key) {
                u.capture_invalid = true;
            }
            if !crate::input::binding_pair_allowed(key, other, held) {
                u.capture_invalid = true;
            }
            if u.capture_candidate.is_none() {
                u.capture_candidate = Some(key);
            }
        } else if u.capture_candidate == Some(key) {
            if !u.capture_invalid {
                bind(u, key);
            }
            u.capture_candidate = None;
            u.capture_invalid = false;
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
    crate::input::route_key(&mut c, key, down);
    u.shared.wake.notify_all();
}
unsafe fn suppress_held(u: &mut Ui) {
    for key in 1..255 {
        u.suppressed[key] = crate::input::down(key as u16);
    }
}
const PICK_ACCEPT: u32 = WM_APP + 31;
const PICK_CANCEL: u32 = WM_APP + 32;
unsafe extern "system" fn pick_mouse_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && !UI.is_null() && (*UI).screen_pick.is_some() {
        if w as u32 == WM_LBUTTONDOWN {
            return 1;
        }
        if w as u32 == WM_LBUTTONUP {
            let p = (*(l as *const MSLLHOOKSTRUCT)).pt;
            PostMessageW((*UI).hwnd, PICK_ACCEPT, p.x as usize, p.y as isize);
            return 1;
        }
    }
    CallNextHookEx(null_mut(), code, w, l)
}
unsafe extern "system" fn pick_keyboard_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0
        && !UI.is_null()
        && (*UI).screen_pick.is_some()
        && (*(l as *const KBDLLHOOKSTRUCT)).vkCode == 27
    {
        if w as u32 == WM_KEYDOWN || w as u32 == WM_SYSKEYDOWN {
            PostMessageW((*UI).hwnd, PICK_CANCEL, 0, 0);
        }
        return 1;
    }
    CallNextHookEx(null_mut(), code, w, l)
}
unsafe fn request_screen_sample(u: &mut Ui, point: (i32, i32), force: bool) -> u64 {
    if force || u.sample_point != Some(point) {
        u.sample_id = u.sample_id.wrapping_add(1);
        u.sample_point = Some(point);
        u.shared.control.lock().unwrap().sample_request = Some((u.sample_id, point));
        u.shared.wake.notify_all();
    }
    u.sample_id
}
unsafe fn screen_pick_tick(u: &mut Ui) {
    let Some(kind) = u.screen_pick else {
        return;
    };
    if crate::input::down(27) {
        finish_screen_pick(u, false, true);
        return;
    }
    let foreground = GetForegroundWindow();
    if u.screen_pick_started.elapsed().as_millis() < 150 {
        let mut pid = 0;
        GetWindowThreadProcessId(foreground, &mut pid);
        if !foreground.is_null() && pid != GetCurrentProcessId() {
            u.screen_pick_previous_focus = foreground;
        }
    } else if foreground != u.screen_pick_previous_focus && !foreground.is_null() {
        finish_screen_pick(u, false, false);
        return;
    }
    if kind == ScreenPick::Colour {
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        let point = if let Some((_, _, point)) = u.pending_pick {
            point
        } else {
            (cursor.x, cursor.y)
        };
        request_screen_sample(u, point, false);
        let sample = *u.shared.sampled_pixel.lock().unwrap();
        if let Some((id, at, rgb)) = sample {
            if at == point {
                if let Some(marker) = u.marker.as_mut() {
                    marker.color(rgb);
                }
                if let Some((_, wanted_id, wanted_point)) = u.pending_pick {
                    if id == wanted_id && at == wanted_point {
                        finish_screen_pick(u, true, true);
                        return;
                    }
                }
            }
        }
        if let Some((start, _, _)) = u.pending_pick {
            if start.elapsed().as_millis() > 1000 {
                finish_screen_pick(u, false, true);
                MessageBoxW(
                    u.hwnd,
                    wide("The selected screen pixel could not be captured. No colour was changed.")
                        .as_ptr(),
                    wide("Yami").as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
                return;
            }
        }
    }
    if let Some(marker) = u.marker.as_ref() {
        if !marker.update() {
            finish_screen_pick(u, false, true);
        }
    }
}
unsafe fn begin_screen_pick(u: &mut Ui, kind: ScreenPick) {
    if u.screen_pick.is_some() {
        return;
    }
    u.capture = None;
    u.radius_focus = false;
    u.pending_pick = None;
    u.selection_point = None;
    u.sample_point = None;
    pause(u, true);
    u.screen_pick_was_picker = !u.picker.is_null();
    if !u.picker.is_null() {
        ShowWindow(u.picker, SW_HIDE);
    }
    ShowWindow(u.hwnd, SW_HIDE);
    crate::windowing::flush_hidden_windows();
    u.screen_pick_started = std::time::Instant::now();
    u.screen_pick_previous_focus = GetForegroundWindow();
    u.screen_pick = Some(kind);
    u.marker = crate::calibration::Marker::new(u.hwnd, kind == ScreenPick::Colour);
    if let Some(marker) = u.marker.as_ref() {
        u.calibration = marker.hwnd;
    } else {
        finish_screen_pick(u, false, true);
        return;
    }
    // The GUI is hidden. Temporary low-level callbacks receive only selection
    // clicks/Escape; hidden windows cannot reliably own desktop-wide capture.
    u.mouse_hook = SetWindowsHookExW(
        WH_MOUSE_LL,
        Some(pick_mouse_hook),
        GetModuleHandleW(null()),
        0,
    );
    u.keyboard_hook = SetWindowsHookExW(
        WH_KEYBOARD_LL,
        Some(pick_keyboard_hook),
        GetModuleHandleW(null()),
        0,
    );
    if u.mouse_hook.is_null() || u.keyboard_hook.is_null() {
        finish_screen_pick(u, false, true);
        MessageBoxW(
            u.hwnd,
            wide("Screen selection could not start.").as_ptr(),
            wide("Yami").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
        return;
    }
    if kind == ScreenPick::Calibration {
        // Balance exactly our own display-counter changes on every exit path.
        loop {
            u.cursor_hide_calls += 1;
            if ShowCursor(0) < 0 || u.cursor_hide_calls >= 32 {
                break;
            }
        }
        SetCursor(null_mut());
    }
    SetTimer(u.hwnd, 4, 16, None);
    screen_pick_tick(u);
}
unsafe fn finish_screen_pick(u: &mut Ui, accept: bool, restore_focus: bool) {
    let Some(kind) = u.screen_pick.take() else {
        return;
    };
    KillTimer(u.hwnd, 4);
    while u.cursor_hide_calls > 0 {
        ShowCursor(1);
        u.cursor_hide_calls -= 1;
    }
    if !u.mouse_hook.is_null() {
        UnhookWindowsHookEx(u.mouse_hook);
        u.mouse_hook = null_mut();
    }
    if !u.keyboard_hook.is_null() {
        UnhookWindowsHookEx(u.keyboard_hook);
        u.keyboard_hook = null_mut();
    }
    u.shared.control.lock().unwrap().sample_request = None;
    u.shared.wake.notify_all();
    u.calibration = null_mut();
    ReleaseCapture();
    // Capture the cursor location before removing the non-intercepting marker.
    let mut p = POINT { x: 0, y: 0 };
    GetCursorPos(&mut p);
    if let Some(point) = u.selection_point {
        p.x = point.0;
        p.y = point.1;
    }
    drop(u.marker.take());
    if accept {
        match kind {
            ScreenPick::Calibration => {
                let target = GetAncestor(WindowFromPoint(p), GA_ROOT);
                let mut pid = 0;
                GetWindowThreadProcessId(target, &mut pid);
                let mut r: RECT = std::mem::zeroed();
                GetClientRect(target, &mut r);
                let mut origin = POINT { x: 0, y: 0 };
                ClientToScreen(target, &mut origin);
                let exe = crate::capture::exe(target);
                if pid != GetCurrentProcessId()
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
            }
            ScreenPick::Colour => {
                let sample = *u.shared.sampled_pixel.lock().unwrap();
                if let Some((_, at, rgb)) = sample {
                    if at == (p.x, p.y) {
                        u.draft = rgb;
                        u.hex = crate::config::hex(rgb);
                        if !u.hex_edit.is_null() {
                            SetWindowTextW(u.hex_edit, wide(&u.hex).as_ptr());
                        }
                        (u.h, u.s, u.v) = render::to_hsv(rgb);
                        u.shared.change(|c| c.config.rgb = rgb);
                        u.shared.save();
                    }
                }
            }
        }
    }
    u.pending_pick = None;
    u.selection_point = None;
    u.sample_point = None;
    suppress_held(u);
    ShowWindow(u.hwnd, SW_SHOWNOACTIVATE);
    if u.screen_pick_was_picker && !u.picker.is_null() {
        ShowWindow(u.picker, SW_SHOWNOACTIVATE);
    }
    pause(u, !u.picker.is_null());
    if restore_focus {
        let target = if !u.picker.is_null() {
            u.picker
        } else {
            u.hwnd
        };
        SetForegroundWindow(u.hwnd);
        SetFocus(if !u.hex_edit.is_null() {
            u.hex_edit
        } else {
            target
        });
    }
    invalidate(u.hwnd);
    if !u.picker.is_null() {
        invalidate(u.picker);
    }
}
unsafe fn calibration(u: &mut Ui) {
    begin_screen_pick(u, ScreenPick::Calibration);
}
unsafe extern "system" fn calibration_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        return HTTRANSPARENT as isize;
    }
    DefWindowProcW(hwnd, msg, w, l)
}
unsafe fn clean_shutdown(u: &mut Ui) {
    if u.shutting_down || u.closing.is_some() {
        return;
    }
    finish_screen_pick(u, false, false);
    pause(u, true);
    u.shared.running.store(false, Ordering::Release);
    u.shared.wake.notify_all();
    if IsWindowVisible(u.hwnd) == 0 {
        destroy_app(u);
        return;
    }
    if !u.confirm.is_null() {
        let dialog = u.confirm;
        u.confirm = null_mut();
        DestroyWindow(dialog);
    }
    EnableWindow(u.hwnd, 0);
    KillTimer(u.hwnd, 2);
    u.closing = Some(std::time::Instant::now());
    SetTimer(u.hwnd, 5, 16, None);
}
unsafe fn destroy_app(u: &mut Ui) {
    if u.shutting_down {
        return;
    }
    u.shutting_down = true;
    finish_screen_pick(u, false, false);
    u.capture = None;
    pause(u, true);
    u.shared.running.store(false, Ordering::Release);
    u.shared.wake.notify_all();
    u.shared.save();
    crate::input::release_pending();
    if !u.confirm.is_null() {
        let dialog = u.confirm;
        u.confirm = null_mut();
        DestroyWindow(dialog);
    }
    if !u.picker.is_null() {
        destroy_picker(u);
    }
    crate::tray::remove(u.hwnd);
    KillTimer(u.hwnd, 1);
    KillTimer(u.hwnd, 2);
    KillTimer(u.hwnd, 3);
    DestroyWindow(u.hwnd);
}
unsafe fn cancel_exit(u: &mut Ui) {
    if u.confirm.is_null() {
        return;
    }
    let dialog = u.confirm;
    u.confirm = null_mut();
    DestroyWindow(dialog);
    EnableWindow(u.hwnd, 1);
    suppress_held(u);
    pause(u, false);
    SetForegroundWindow(u.hwnd);
    SetFocus(u.hwnd);
    invalidate(u.hwnd);
}
unsafe fn request_exit(u: &mut Ui) {
    if !u.confirm.is_null() || u.shutting_down {
        return;
    }
    finish_screen_pick(u, false, false);
    if !u.picker.is_null() {
        close_picker(u);
    }
    u.capture = None;
    u.radius_focus = false;
    pause(u, true);
    suppress_held(u);
    u.confirm_exit_selected = false;
    u.confirm_scale = u.scale;
    let w = (360. * u.confirm_scale) as i32;
    let h = (184. * u.confirm_scale) as i32;
    let (x, y) = crate::windowing::centered_dialog(u.hwnd, w, h);
    let dialog = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        wide("Yami106Confirm").as_ptr(),
        wide("Close Yami?").as_ptr(),
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
    if dialog.is_null() {
        pause(u, false);
        return;
    }
    u.confirm = dialog;
    SetWindowRgn(dialog, CreateRoundRectRgn(0, 0, w + 1, h + 1, 18, 18), 1);
    EnableWindow(u.hwnd, 0);
    ShowWindow(dialog, SW_SHOW);
    SetForegroundWindow(dialog);
    SetFocus(dialog);
}
unsafe fn draw_exit(u: &Ui, g: &Canvas) {
    g.round(0.7, 0.7, 358.6, 182.6, 10., BG, BORDER);
    g.text("Close Yami?", 24., 10., 280., 40., 21., TEXT, 600, false);
    g.close(332., 28.);
    g.text(
        "Are you sure you want to exit?",
        24.,
        61.,
        320.,
        32.,
        16.,
        TEXT,
        400,
        false,
    );
    button(u, g, "Cancel", 24., 124., 144., 38., false);
    button(u, g, "Exit", 192., 124., 144., 38., true);
    g.round(
        if u.confirm_exit_selected { 192. } else { 24. },
        124.,
        144.,
        38.,
        7.,
        0,
        TEXT,
    );
}
unsafe extern "system" fn confirm_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if UI.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let u = &mut *UI;
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(u, hwnd);
            0
        }
        WM_KEYDOWN => {
            match w {
                27 => cancel_exit(u),
                9 | 37 | 39 => {
                    u.confirm_exit_selected = !u.confirm_exit_selected;
                    invalidate(hwnd);
                }
                13 | 32 => {
                    if u.confirm_exit_selected {
                        clean_shutdown(u);
                    } else {
                        cancel_exit(u);
                    }
                }
                _ => (),
            }
            0
        }
        WM_LBUTTONUP => {
            let x = (l as i16) as f32 / u.confirm_scale;
            let y = ((l >> 16) as i16) as f32 / u.confirm_scale;
            if hit(x, y, (192., 124., 144., 38.)) {
                clean_shutdown(u);
            } else if hit(x, y, (24., 124., 144., 38.)) || hit(x, y, (310., 8., 40., 40.)) {
                cancel_exit(u);
            }
            0
        }
        WM_CLOSE => {
            cancel_exit(u);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe fn hide_to_tray(u: &mut Ui) {
    finish_screen_pick(u, false, false);
    if !u.picker.is_null() {
        close_picker(u);
    }
    u.capture = None;
    u.radius_focus = false;
    pause(u, false);
    suppress_held(u);
    ShowWindow(u.hwnd, SW_HIDE);
}
unsafe fn start_install(u: &mut Ui) {
    u.displayed_progress = 0.;
    u.last_progress = 0;
    u.progress_tick = std::time::Instant::now();
    u.pending_launch = None;
    u.job = Some(crate::install::start(u.payload.unwrap()));
    SetTimer(u.hwnd, 3, 16, None);
}
unsafe fn apply_radius(u: &mut Ui) {
    if let Ok(value) = u.radius_text.parse::<u32>() {
        if (1..=128).contains(&value) {
            u.shared.change(|c| c.config.radius = value);
            u.shared.save();
        }
    }
    invalidate(u.hwnd);
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
    if u.screen_pick.is_some() {
        match msg {
            WM_MOUSEMOVE => {
                if let Some(marker) = u.marker.as_ref() {
                    marker.update();
                }
                return 0;
            }
            WM_LBUTTONDOWN => return 0,
            WM_LBUTTONUP => {
                finish_screen_pick(u, true, true);
                return 0;
            }
            WM_KEYDOWN if w == 27 => {
                finish_screen_pick(u, false, true);
                return 0;
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => finish_screen_pick(u, false, false),
            WM_CAPTURECHANGED if l as HWND != u.hwnd => finish_screen_pick(u, false, false),
            _ => (),
        }
    }
    if msg == PICK_CANCEL {
        finish_screen_pick(u, false, true);
        return 0;
    }
    if msg == PICK_ACCEPT {
        if let Some(kind) = u.screen_pick {
            let point = (w as i32, l as i32);
            u.selection_point = Some(point);
            if kind == ScreenPick::Colour {
                let id = request_screen_sample(u, point, true);
                u.pending_pick = Some((std::time::Instant::now(), id, point));
            } else {
                finish_screen_pick(u, true, true);
            }
        }
        return 0;
    }
    let picker = hwnd == u.picker;
    let s = if picker { u.picker_scale } else { u.scale };
    let x = (l as i16) as f32 / s
        - if !picker && u.panel_left {
            u.panel_extent_px as f32 / u.scale
        } else {
            0.
        };
    let y = ((l >> 16) as i16) as f32 / s;
    match msg {
        WM_COMMAND if picker && (w >> 16) & 0xffff == 0x0300 && l as HWND == u.hex_edit => {
            let mut text = [0u16; 16];
            let n = GetWindowTextW(u.hex_edit, text.as_mut_ptr(), 16);
            u.hex = String::from_utf16_lossy(&text[..n as usize]);
            if !u.hex_syncing {
                if let Some(rgb) = crate::config::parse(&u.hex) {
                    u.draft = rgb;
                    (u.h, u.s, u.v) = render::to_hsv(rgb);
                }
            }
            invalidate(u.picker);
            0
        }
        WM_CTLCOLOREDIT if picker => {
            SetTextColor(w as HDC, 0x00fff3f5);
            SetBkColor(w as HDC, 0x00231919);
            u.hex_brush as isize
        }
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
                layout_panel(u);
            }
            0
        }
        WM_LBUTTONDOWN => {
            u.pressed_ui = true;
            SetTimer(u.hwnd, 5, 16, None);
            if picker {
                if y < 40. && x < 250. {
                    ReleaseCapture();
                    let mut p = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut p);
                    SendMessageW(
                        u.hwnd,
                        WM_NCLBUTTONDOWN,
                        HTCAPTION as usize,
                        ((p.x as u16 as usize) | ((p.y as u16 as usize) << 16)) as isize,
                    );
                    return 0;
                }
                u.hex_focus = hit(x, y, (20., 278., 208., 38.));
                if u.hex_focus {
                    SetFocus(u.hex_edit);
                } else {
                    SetFocus(u.picker);
                }
                picker_point(u, x, y, true);
                if u.drag != 0 {
                    SetCapture(hwnd);
                }
                return 0;
            }
            let width = if u.payload.is_none() {
                400.
            } else {
                dimensions(u).0
            };
            if u.payload.is_none() && y < 54. && x < width - 100. {
                ReleaseCapture();
                let mut position = POINT { x: 0, y: 0 };
                GetCursorPos(&mut position);
                let screen = (position.x as u16 as usize) | ((position.y as u16 as usize) << 16);
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, screen as isize);
            }
            0
        }
        WM_MOUSEMOVE => {
            u.hover = Some((x, y));
            SetTimer(u.hwnd, 5, 16, None);
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            invalidate(hwnd);
            if picker {
                if u.drag != 0 {
                    picker_point(u, x, y, false);
                }
            }
            0
        }
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_NCLBUTTONUP => {
            u.pressed_ui = false;
            SetTimer(u.hwnd, 5, 16, None);
            0
        }
        0x02a3 => {
            // WM_MOUSELEAVE (SDK constant outside the enabled bindings)
            u.hover = None;
            u.pressed_ui = false;
            SetTimer(u.hwnd, 5, 16, None);
            invalidate(hwnd);
            0
        }
        WM_LBUTTONUP => {
            u.pressed_ui = false;
            SetTimer(u.hwnd, 5, 16, None);
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
                    begin_screen_pick(u, ScreenPick::Colour);
                }
                return 0;
            }
            let (width, _) = dimensions(u);
            if hit(x, y, (width - 50., 8., 42., 40.)) {
                if u.payload.is_none() {
                    request_exit(u);
                } else {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
                return 0;
            }
            if (u.view == View::Main || u.view == View::Install)
                && hit(x, y, (width - 100., 8., 40., 40.))
            {
                if u.payload.is_none() {
                    hide_to_tray(u);
                } else {
                    ShowWindow(hwnd, SW_MINIMIZE);
                }
                return 0;
            }
            match u.view {
                View::Main => {
                    if hit(x, y, (20., 360., 356., 52.)) {
                        close_picker(u);
                        u.view = View::Settings;
                        u.page_motion.reset(0.);
                        u.page_motion.set(1., if u.animate { 160 } else { 0 });
                        SetTimer(u.hwnd, 5, 16, None);
                        u.radius_text = u.shared.control.lock().unwrap().config.radius.to_string();
                        pause(u, !u.picker.is_null());
                        invalidate(u.hwnd);
                    } else if hit(x, y, (232., 184., 148., 46.)) {
                        open_picker(u);
                    }
                }
                View::Settings => {
                    if hit(x, y, (240., 292., 136., 36.)) {
                        u.radius_focus = true;
                        u.select_all = true;
                        pause(u, true);
                        invalidate(hwnd);
                        return 0;
                    }
                    if u.radius_focus {
                        u.radius_focus = false;
                        pause(u, false);
                    }
                    if hit(x, y, (0., 56., 400., 42.)) {
                        u.capture = None;
                        pause(u, false);
                        u.view = View::Main;
                        u.page_motion.reset(0.);
                        u.page_motion.set(1., if u.animate { 160 } else { 0 });
                        SetTimer(u.hwnd, 5, 16, None);
                        invalidate(u.hwnd);
                    } else {
                        let mut bound = false;
                        for i in 0..2 {
                            if hit(x, y, bindings(i)) {
                                u.capture = Some(i);
                                u.capture_candidate = None;
                                u.capture_invalid = false;
                                pause(u, true);
                                bound = true;
                                break;
                            }
                        }
                        if !bound {
                            if u.capture.is_some() {
                                u.capture = None;
                                pause(u, false);
                            }
                            if hit(x, y, (318., 370., 64., 36.)) {
                                u.shared.change(|c| c.config.fixed = !c.config.fixed);
                                u.toggle_motion.set(
                                    if u.shared.control.lock().unwrap().config.fixed {
                                        1.
                                    } else {
                                        0.
                                    },
                                    if u.animate { 150 } else { 0 },
                                );
                                SetTimer(u.hwnd, 5, 16, None);
                                u.shared.save();
                                invalidate(u.hwnd);
                            } else if u.shared.control.lock().unwrap().config.fixed
                                && hit(x, y, (180., 372., 130., 32.))
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
                            start_install(u);
                        }
                    }
                }
                View::Replace => {
                    if hit(x, y, (82., 148., 132., 40.)) {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    } else if hit(x, y, (228., 148., 136., 40.)) {
                        u.view = View::Install;
                        start_install(u);
                        resize(u);
                    }
                }
            }
            invalidate(hwnd);
            0
        }
        WM_KEYDOWN => {
            if !picker && u.radius_focus {
                if w == 8 {
                    if u.select_all {
                        u.radius_text.clear();
                        u.select_all = false;
                    } else {
                        u.radius_text.pop();
                    }
                    apply_radius(u);
                } else if w == 13 || w == 27 {
                    u.radius_focus = false;
                    u.radius_text = u.shared.control.lock().unwrap().config.radius.to_string();
                    pause(u, false);
                    invalidate(hwnd);
                }
                return 0;
            }
            if picker {
                if w == 27 {
                    close_picker(u);
                } else if w == 13 {
                    commit(u);
                }
            } else if w == 27 && u.capture.is_some() {
                bind(u, 27);
            }
            0
        }
        WM_CHAR => {
            if !picker && u.radius_focus {
                if let Some(ch) = char::from_u32(w as u32) {
                    if ch.is_ascii_digit() && (u.select_all || u.radius_text.len() < 3) {
                        if u.select_all {
                            u.radius_text.clear();
                            u.select_all = false;
                        }
                        u.radius_text.push(ch);
                        apply_radius(u);
                    }
                }
                return 0;
            }
            0
        }
        WM_ACTIVATE => {
            if w as u16 == WA_INACTIVE as u16 && u.radius_focus {
                u.radius_focus = false;
                u.radius_text = u.shared.control.lock().unwrap().config.radius.to_string();
                pause(u, false);
                invalidate(u.hwnd);
            }
            if w as u16 == WA_INACTIVE as u16 && u.capture.is_some() {
                u.capture = None;
                pause(u, false);
                invalidate(u.hwnd);
            }
            0
        }
        WM_TIMER => {
            if w == 5 {
                if let Some(start) = u.closing {
                    let t = if u.animate {
                        (start.elapsed().as_secs_f32() / 0.15).clamp(0., 1.)
                    } else {
                        1.
                    };
                    SetLayeredWindowAttributes(u.hwnd, 0, ((1. - t) * 255.) as u8, LWA_ALPHA);
                    if t >= 1. {
                        KillTimer(u.hwnd, 5);
                        destroy_app(u);
                    }
                    return 0;
                }
                if !u.picker.is_null() {
                    if u.panel_motion.active()
                        || (248. * u.scale * u.panel_motion.value()).round() as i32
                            != u.panel_extent_px
                    {
                        layout_panel(u);
                    }
                    if !u.panel_motion.active()
                        && !u.panel_closing
                        && !u.hex_edit.is_null()
                        && IsWindowVisible(u.hex_edit) == 0
                    {
                        ShowWindow(u.hex_edit, SW_SHOW);
                    }
                    if u.panel_closing && !u.panel_motion.active() {
                        destroy_picker(u);
                    }
                }
                invalidate(u.hwnd);
                let active = u.page_motion.active()
                    || u.toggle_motion.active()
                    || u.panel_motion.active()
                    || u.hover_motion.borrow().values().any(|m| m.active());
                if !active {
                    KillTimer(u.hwnd, 5);
                    if !u.hex_edit.is_null() && !u.panel_closing {
                        ShowWindow(u.hex_edit, SW_SHOW);
                    }
                }
                return 0;
            }
            if w == 2 {
                u.fade.tick(u.hwnd);
                return 0;
            }
            if w == 4 {
                screen_pick_tick(u);
                return 0;
            }
            if w == 3 {
                if let Some(job) = u.job.as_ref() {
                    let target = job
                        .progress
                        .load(Ordering::Acquire)
                        .max(u.last_progress)
                        .min(100);
                    u.last_progress = target;
                    let dt = u.progress_tick.elapsed().as_secs_f32();
                    u.progress_tick = std::time::Instant::now();
                    u.displayed_progress = (u.displayed_progress
                        + (target as f32 - u.displayed_progress) * (1. - (-dt / 0.055).exp()))
                    .min(target as f32);
                    if target == 100 && u.displayed_progress > 99.8 {
                        u.displayed_progress = 100.;
                    }
                    invalidate(u.hwnd);
                    if u.displayed_progress >= 100. {
                        if let Some(path) = u.pending_launch.take() {
                            UpdateWindow(u.hwnd);
                            let _ = std::process::Command::new(path).spawn();
                            KillTimer(u.hwnd, 3);
                            DestroyWindow(u.hwnd);
                        }
                    }
                }
                return 0;
            }
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
                    match result {
                        Ok(path) => {
                            u.pending_launch = Some(path);
                        }
                        Err(error) => {
                            u.job = None;
                            KillTimer(u.hwnd, 3);
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
        WM_DISPLAYCHANGE => {
            finish_screen_pick(u, false, false);
            resize(u);
            0
        }
        WM_SYSCOMMAND if w & 0xfff0 == SC_CLOSE as usize && u.payload.is_none() => {
            request_exit(u);
            0
        }
        WM_SYSCOMMAND
            if w & 0xfff0 == SC_SIZE as usize
                || (u.payload.is_some() && w & 0xfff0 == SC_MOVE as usize) =>
        {
            0
        }
        WM_SYSCOMMAND if w & 0xfff0 == SC_MINIMIZE as usize => {
            if u.payload.is_none() {
                hide_to_tray(u);
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
                if u.payload.is_none() {
                    clean_shutdown(u);
                } else {
                    DestroyWindow(hwnd);
                }
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
                if !hwnd.is_null()
                    && GetPropW(hwnd, wide("Yami.Build.106.NativeAHKScan").as_ptr()).is_null()
                {
                    MessageBoxW(null_mut(), wide("Another Yami version is still running. Exit it from its tray icon before starting this build.").as_ptr(), wide("Yami").as_ptr(), MB_OK | MB_ICONINFORMATION);
                } else if !hwnd.is_null() {
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
            marker: None,
            cursor_hide_calls: 0,
            screen_pick: None,
            screen_pick_previous_focus: null_mut(),
            screen_pick_started: std::time::Instant::now(),
            screen_pick_was_picker: false,
            mouse_hook: null_mut(),
            keyboard_hook: null_mut(),
            sample_id: 0,
            sample_point: None,
            selection_point: None,
            pending_pick: None,
            confirm: null_mut(),
            confirm_exit_selected: false,
            confirm_scale: 1.,
            suppressed: [false; 256],
            fade: crate::windowing::Fade::new(),
            shutting_down: false,
            displayed_progress: 0.,
            progress_tick: std::time::Instant::now(),
            pending_launch: None,
            radius_focus: false,
            radius_text: "12".into(),
            hover: None,
            pressed_ui: false,
            animate: crate::windowing::animations_enabled(),
            hover_motion: std::cell::RefCell::new(std::collections::HashMap::new()),
            toggle_motion: crate::motion::Tween::new(
                if shared.control.lock().unwrap().config.fixed {
                    1.
                } else {
                    0.
                },
            ),
            page_motion: crate::motion::Tween::new(1.),
            panel_motion: crate::motion::Tween::new(0.),
            panel_closing: false,
            panel_left: false,
            panel_extent_px: 0,
            closing: None,
            hex_edit: null_mut(),
            hex_syncing: false,
            hex_font: null_mut(),
            hex_font_scale: 0.,
            hex_brush: null_mut(),
            hex_original_proc: None,
            scale: 1.,
            picker_scale: 1.,
            view,
            shared: shared.clone(),
            capture: None,
            capture_candidate: None,
            capture_invalid: false,
            blocked: 0,
            pressed: [false; 256],
            draft: 0xffffff,
            hex: "#FFFFFF".into(),
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
                "Yami106Confirm",
                Some(confirm_proc as unsafe extern "system" fn(_, _, _, _) -> _),
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
                hIcon: LoadIconW(instance, 101usize as *const u16),
                hIconSm: LoadIconW(instance, 101usize as *const u16),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&wc);
        }
        let u = &mut *UI;
        let monitor = crate::windowing::active_monitor();
        let work = crate::windowing::work_area(monitor);
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
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
            WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
            work.left + 10,
            work.top + 10,
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
        if payload.is_none() {
            SetPropW(
                hwnd,
                wide("Yami.Build.106.PurpleRepair").as_ptr(),
                1usize as HANDLE,
            );
        }
        u.shared.control.lock().unwrap().ui_hwnd = hwnd as usize;
        resize(u);
        crate::windowing::center(hwnd, monitor);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if payload.is_none() {
            let config = u.shared.control.lock().unwrap().config.clone();
            if let Err(error) = crate::config::initialize_if_new(&config) {
                MessageBoxW(
                    hwnd,
                    wide(&format!("Default settings could not be saved: {error}")).as_ptr(),
                    wide("Yami").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
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
        u.fade.show(hwnd);
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
