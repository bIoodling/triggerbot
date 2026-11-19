use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub toggle: u16,
    pub hold: u16,
    pub output: u16,
    pub rgb: u32,
    pub fixed: bool,
    pub calibrated: bool,
    pub x: f64,
    pub y: f64,
    pub target_exe: String,
    pub radius: u32,
    pub tolerance: u32,
    pub clear_ms: u64,
    pub clear_frames: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            toggle: 0,
            hold: 0,
            output: 0,
            rgb: 0xffffff,
            fixed: false,
            calibrated: false,
            x: 0.5,
            y: 0.5,
            target_exe: String::new(),
            radius: 12,
            tolerance: 2,
            clear_ms: 8,
            clear_frames: 2,
        }
    }
}
pub fn path() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
        .join("Yami")
        .join("settings.json")
}
pub fn load() -> Config {
    let mut c: Config = fs::read(path())
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or_else(migrate);
    c.rgb &= 0xffffff;
    c.radius = c.radius.clamp(1, 128);
    c.tolerance = c.tolerance.min(16);
    c.clear_ms = c.clear_ms.min(100);
    c.clear_frames = c.clear_frames.clamp(2, 16);
    if !c.x.is_finite() || !c.y.is_finite() {
        c.calibrated = false;
        c.x = 0.5;
        c.y = 0.5;
    }
    c.x = c.x.clamp(0., 1.);
    c.y = c.y.clamp(0., 1.);
    for bind in [&mut c.toggle, &mut c.hold, &mut c.output] {
        if *bind > 254 {
            *bind = 0;
        }
    }
    c
}
fn migrate() -> Config {
    let mut c = Config::default();
    let old =
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("YamiTB/settings.ini");
    let Ok(bytes) = fs::read(old) else {
        return c;
    };
    let text = if bytes.starts_with(&[255, 254]) {
        String::from_utf16_lossy(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    let mut section = "";
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match (section, key) {
                ("Activation", "PrimaryVK") => c.toggle = value.parse().unwrap_or(0),
                ("Activation", "HoldVK") => c.hold = value.parse().unwrap_or(0),
                ("Output", "VK") => c.output = value.parse().unwrap_or(0),
                ("Detector", "TargetRGB") => c.rgb = parse(value).unwrap_or(0xffffff),
                ("Detection", "Cursorless") => c.fixed = value == "1",
                ("Detection", "Calibrated") => c.calibrated = value == "1",
                ("Detection", "NormalizedX") => c.x = value.parse().unwrap_or(0.5),
                ("Detection", "NormalizedY") => c.y = value.parse().unwrap_or(0.5),
                _ => (),
            }
        }
    }
    if c.calibrated {
        c.target_exe = "robloxplayerbeta.exe".into();
    }
    c
}
pub fn save(c: &Config) -> io::Result<()> {
    let p = path();
    fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("new");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(c)?)?;
    f.sync_all()?;
    drop(f);
    unsafe {
        use windows_sys::Win32::Storage::FileSystem::*;
        if MoveFileExW(
            crate::wide(tmp.to_str().unwrap()).as_ptr(),
            crate::wide(p.to_str().unwrap()).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
pub fn initialize_if_new(c: &Config) -> io::Result<()> {
    let legacy =
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("YamiTB/settings.ini");
    if !path().exists() && !legacy.exists() {
        save(c)?;
    }
    Ok(())
}
pub fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0xffffff)
}
pub fn parse(s: &str) -> Option<u32> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(s, 16).ok()
}
