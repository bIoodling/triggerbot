use windows_sys::Win32::{
    Foundation::*,
    UI::{Shell::*, WindowsAndMessaging::*},
};
pub const CALLBACK: u32 = WM_APP + 10;
pub unsafe fn add(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = CALLBACK;
    data.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
    let text = crate::wide("Yami");
    data.szTip[..text.len()].copy_from_slice(&text);
    Shell_NotifyIconW(NIM_ADD, &data);
}
pub unsafe fn remove(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &data);
}
pub unsafe fn menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    AppendMenuW(menu, MF_STRING, 701, crate::wide("Open").as_ptr());
    AppendMenuW(menu, MF_STRING, 702, crate::wide("Exit").as_ptr());
    let mut p = POINT { x: 0, y: 0 };
    GetCursorPos(&mut p);
    SetForegroundWindow(hwnd);
    let id = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        p.x,
        p.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    DestroyMenu(menu);
    if id == 701 {
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    } else if id == 702 {
        PostMessageW(hwnd, WM_CLOSE, 0, 0);
    }
}
