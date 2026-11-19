use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Registry::*, Threading::*},
    UI::WindowsAndMessaging::*,
};
pub struct Job {
    pub progress: AtomicU32,
    pub result: Mutex<Option<Result<PathBuf, String>>>,
}
pub fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Yami")
}
fn safe_directory(p: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    if p != directory() {
        return Err("Unexpected installation directory".into());
    }
    if let Ok(m) = fs::symlink_metadata(p) {
        if m.file_attributes() & 0x400 != 0 {
            return Err("Installation directory must not be a link or junction".into());
        }
    }
    Ok(())
}
pub fn exists() -> bool {
    directory().join("Yami.exe").is_file()
}
unsafe fn stop_running() -> Result<(), String> {
    for class in ["Yami106Main", "YamiMain"] {
        let hwnd = FindWindowW(crate::wide(class).as_ptr(), std::ptr::null());
        if !hwnd.is_null() {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = OpenProcess(0x00100000, 0, pid);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            if !process.is_null() {
                let result = WaitForSingleObject(process, 8000);
                CloseHandle(process);
                if result != WAIT_OBJECT_0 {
                    return Err("Close the running Yami application and try again".into());
                }
            }
        }
    }
    Ok(())
}
unsafe fn reg_string(key: HKEY, name: &str, value: &str) -> Result<(), String> {
    let value = crate::wide(value);
    let status = RegSetValueExW(
        key,
        crate::wide(name).as_ptr(),
        0,
        REG_SZ,
        value.as_ptr().cast(),
        (value.len() * 2) as u32,
    );
    if status != 0 {
        Err(format!("Windows registration failed ({status})"))
    } else {
        Ok(())
    }
}
unsafe fn register(dir: &Path) -> Result<(), String> {
    let mut key = std::ptr::null_mut();
    let status = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        crate::wide("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Yami").as_ptr(),
        0,
        std::ptr::null(),
        0,
        KEY_SET_VALUE,
        std::ptr::null(),
        &mut key,
        std::ptr::null_mut(),
    );
    if status != 0 {
        return Err(format!("Windows registration failed ({status})"));
    }
    let result = (|| {
        reg_string(key, "DisplayName", "Yami")?;
        reg_string(key, "DisplayVersion", "1.0.6")?;
        reg_string(key, "InstallLocation", &dir.to_string_lossy())?;
        reg_string(key, "DisplayIcon", &dir.join("Yami.exe").to_string_lossy())?;
        reg_string(
            key,
            "UninstallString",
            &format!("\"{}\" --uninstall", dir.join("Uninstall.exe").display()),
        )?;
        let yes = 1u32;
        RegSetValueExW(
            key,
            crate::wide("NoModify").as_ptr(),
            0,
            REG_DWORD,
            (&yes as *const u32).cast(),
            4,
        );
        RegSetValueExW(
            key,
            crate::wide("NoRepair").as_ptr(),
            0,
            REG_DWORD,
            (&yes as *const u32).cast(),
            4,
        );
        Ok(())
    })();
    RegCloseKey(key);
    result
}
pub fn start(payload: &'static [u8]) -> Arc<Job> {
    let job = Arc::new(Job {
        progress: AtomicU32::new(0),
        result: Mutex::new(None),
    });
    let copy = job.clone();
    std::thread::spawn(move || {
        let result = perform(payload, &copy);
        *copy.result.lock().unwrap() = Some(result);
    });
    job
}
fn perform(payload: &[u8], job: &Job) -> Result<PathBuf, String> {
    let dir = directory();
    safe_directory(&dir)?;
    unsafe {
        stop_running()?;
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    job.progress.store(10, Ordering::Release);
    let app = dir.join("Yami.exe");
    let uninstaller = dir.join("Uninstall.exe");
    let files = [app, uninstaller];
    let setup = std::env::current_exe().map_err(|e| e.to_string())?;
    if setup.parent() == Some(dir.as_path()) {
        return Err("Run the installer from outside the installation directory".into());
    }
    let setup_bytes = fs::read(&setup).map_err(|e| e.to_string())?;
    let contents = [payload, setup_bytes.as_slice()];
    for (index, file) in files.iter().enumerate() {
        let temp = file.with_extension("new");
        fs::write(&temp, contents[index]).map_err(|e| e.to_string())?;
        if Sha256::digest(fs::read(&temp).map_err(|e| e.to_string())?).as_slice()
            != Sha256::digest(contents[index]).as_slice()
        {
            return Err("Installed file verification failed".into());
        }
        job.progress
            .store(30 + index as u32 * 25, Ordering::Release);
    }
    let mut replaced = 0usize;
    let mut backed = [false; 2];
    let operation = (|| -> Result<(), String> {
        for (i, file) in files.iter().enumerate() {
            let backup = file.with_extension("bak");
            if backup.exists() {
                return Err(
                    "A previous installation backup needs review before replacing files".into(),
                );
            }
            if file.exists() {
                fs::rename(file, &backup).map_err(|e| e.to_string())?;
                backed[i] = true;
            }
            fs::rename(file.with_extension("new"), file).map_err(|e| e.to_string())?;
            replaced = i + 1;
        }
        job.progress.store(85, Ordering::Release);
        unsafe {
            register(&dir)?;
        }
        Ok(())
    })();
    if let Err(error) = operation {
        for (i, file) in files.iter().enumerate() {
            if i < replaced {
                let _ = fs::remove_file(file);
            }
            if backed[i] {
                let _ = fs::rename(file.with_extension("bak"), file);
            }
            let _ = fs::remove_file(file.with_extension("new"));
        }
        return Err(error);
    }
    for file in &files {
        let _ = fs::remove_file(file.with_extension("bak"));
    }
    job.progress.store(100, Ordering::Release);
    Ok(dir.join("Yami.exe"))
}
pub fn uninstall_command() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--cleanup-yami") {
        let dir = directory();
        if safe_directory(&dir).is_err() {
            return true;
        }
        if let Some(pid) = args.get(2).and_then(|v| v.parse::<u32>().ok()) {
            unsafe {
                let p = OpenProcess(0x00100000, 0, pid);
                if !p.is_null() {
                    WaitForSingleObject(p, 10000);
                    CloseHandle(p);
                }
            }
        }
        for name in ["Yami.exe", "Uninstall.exe"] {
            let _ = fs::remove_file(dir.join(name));
        }
        let _ = fs::remove_dir(&dir);
        unsafe {
            RegDeleteTreeW(
                HKEY_CURRENT_USER,
                crate::wide("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Yami")
                    .as_ptr(),
            );
        }
        return true;
    }
    let is_uninstaller = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|v| v.to_string_lossy().to_lowercase()))
        .map(|n| n == "uninstall.exe")
        .unwrap_or(false);
    if !is_uninstaller && !args.iter().any(|a| a == "--uninstall") {
        return false;
    }
    unsafe {
        if MessageBoxW(
            std::ptr::null_mut(),
            crate::wide("Uninstall Yami? Your settings will be preserved.").as_ptr(),
            crate::wide("Yami").as_ptr(),
            MB_OKCANCEL | MB_ICONQUESTION,
        ) != IDOK
        {
            return true;
        }
        if let Err(e) = stop_running() {
            MessageBoxW(
                std::ptr::null_mut(),
                crate::wide(&e).as_ptr(),
                crate::wide("Yami").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return true;
        }
    }
    let helper = std::env::temp_dir().join(format!("Yami-uninstall-{}.exe", std::process::id()));
    if fs::copy(std::env::current_exe().unwrap(), &helper).is_ok() {
        let _ = std::process::Command::new(&helper)
            .arg("--cleanup-yami")
            .arg(std::process::id().to_string())
            .spawn();
    }
    true
}
