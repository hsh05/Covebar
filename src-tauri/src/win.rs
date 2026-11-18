//! Small Win32 helpers shared by the rest of the backend.

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

pub fn hval(h: HWND) -> isize {
    h.0 as isize
}

/// The native handle of a Tauri window, converted to this crate's `windows` version.
pub fn tauri_hwnd(w: &tauri::WebviewWindow) -> Option<HWND> {
    w.hwnd().ok().map(|h| HWND(h.0 as *mut _))
}

pub fn window_pid(h: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

pub fn window_title(h: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn file_stem_lower(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Hide a window from Alt+Tab and the taskbar.
pub fn make_tool_window(h: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let ex = (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex);
    }
}

/// Show a window on top without taking focus.
/// Hide a window shown with `show_no_activate`. Tauri's own `hide()` skips windows it
/// didn't show itself, so use the Win32 call directly.
pub fn hide(h: HWND) {
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
}

pub fn show_no_activate(h: HWND) {
    unsafe {
        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Put a topmost window back in front of other topmost windows (a no-op when it already is).
pub fn raise_topmost(h: HWND) {
    unsafe {
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
    }
}

pub fn primary_monitor() -> HMONITOR {
    unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) }
}

pub fn monitor_rect(m: HMONITOR) -> RECT {
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GetMonitorInfoW(m, &mut mi);
    }
    mi.rcMonitor
}

pub fn monitor_scale(m: HMONITOR) -> f64 {
    let (mut x, mut y) = (96u32, 96u32);
    unsafe {
        let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut x, &mut y);
    }
    x as f64 / 96.0
}

/// Windows build number (22000+ is Windows 11).
pub fn build_number() -> u32 {
    let mut buf = [0u16; 32];
    let mut size = (buf.len() * 2) as u32;
    let key = wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let val = wide("CurrentBuildNumber");
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(val.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return 0;
    }
    String::from_utf16_lossy(&buf)
        .trim_end_matches('\0')
        .trim()
        .parse()
        .unwrap_or(0)
}

pub fn is_windows_11() -> bool {
    build_number() >= 22000
}

/// Start, search and other shell flyouts (and the taskbar itself).
pub fn is_shell_ui(h: HWND) -> bool {
    if h.is_invalid() {
        return false;
    }
    let exe = process_path(window_pid(h)).unwrap_or_default();
    matches!(
        file_stem_lower(&exe).as_str(),
        "startmenuexperiencehost" | "searchhost" | "searchapp" | "shellexperiencehost" | "shellhost"
    ) || matches!(class_name(h).as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
        || is_start_ui(h)
}

/// The Start menu or search (newer Windows 11 builds run Start inside SearchHost). Alt+Tab
/// and Task View are Explorer windows and deliberately don't count.
pub fn is_start_ui(h: HWND) -> bool {
    if h.is_invalid() {
        return false;
    }
    let exe = process_path(window_pid(h)).unwrap_or_default();
    matches!(file_stem_lower(&exe).as_str(), "startmenuexperiencehost" | "searchhost" | "searchapp")
}

/// The window is hidden by DWM (closed Start, suspended UWP apps, other desktops).
pub fn is_cloaked(h: HWND) -> bool {
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    let mut v: u32 = 0;
    unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut v as *mut _ as *mut _, 4).is_ok() && v != 0 }
}

/// Start or search is on screen. Their windows stay around (cloaked) while closed.
pub fn start_open() -> bool {
    use windows::core::BOOL;
    use windows::Win32::Foundation::LPARAM;
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        if class_name(h) == "Windows.UI.Core.CoreWindow"
            && IsWindowVisible(h).as_bool()
            && !is_cloaked(h)
            && is_start_ui(h)
        {
            *(lp.0 as *mut bool) = true;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut open = false;
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut open as *mut bool as isize));
    }
    open
}
