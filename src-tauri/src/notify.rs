//! Learns how to click each tray icon the way the Windows taskbar does.
//!
//! Apps add and update their tray icons by sending `WM_COPYDATA` to the first window of class
//! `Shell_TrayWnd` (`Shell_NotifyIcon` does this for them). Only those messages carry the
//! window and callback message an icon expects its clicks on; UI Automation's Invoke is a
//! keyboard-style select that many apps (Spotify, Steam, …) ignore.
//!
//! So while the tray widget is on, a stand-in window of that class sits in front of Explorer's
//! taskbar. It notes each icon's window, id, callback message and version, and passes every
//! message on to Explorer unchanged, returning Explorer's answer, so the Windows tray and
//! SHAppBarMessage callers see no difference. It copies the taskbar's position and visibility
//! (invisible and click-through) for programs that measure the taskbar through it. If Covebar
//! stops, `Shell_NotifyIcon` reaches Explorer directly again.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// What an icon needs to receive a click.
#[derive(Clone, Debug)]
struct Icon {
    hwnd: isize,
    uid: u32,
    guid: Option<GUID>,
    msg: u32,
    version: u32,
    tip: String,
}

static ICONS: Mutex<Vec<Icon>> = Mutex::new(Vec::new());
/// Our stand-in window, 0 when not running.
static HOST: AtomicIsize = AtomicIsize::new(0);
static LAST_BROADCAST: Mutex<Option<Instant>> = Mutex::new(None);

const SIGNATURE: u32 = 0x3475_3423;
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIM_SETVERSION: u32 = 4;
const NIF_MESSAGE: u32 = 0x1;
const NIF_TIP: u32 = 0x4;
const NIF_GUID: u32 = 0x20;
const NIN_SELECT: u32 = WM_USER;
const TIMER_SYNC: usize = 1;

/// Start or stop the stand-in window (it runs while the tray widget is on).
pub fn set_running(on: bool) {
    let h = HOST.load(Ordering::SeqCst);
    if on && h == 0 {
        // Claim the slot first so a second call can't start another one.
        if HOST.compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            std::thread::spawn(run);
        }
    } else if !on && h > 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            HOST.store(0, Ordering::SeqCst);
            return;
        };
        let class = w!("Shell_TrayWnd");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
        let h = match CreateWindowExW(ex, class, PCWSTR::null(), WS_POPUP, 0, 0, 0, 0, None, None, Some(module.into()), None) {
            Ok(h) => h,
            Err(e) => {
                crate::dlog!("tray stand-in window failed: {e}");
                HOST.store(0, Ordering::SeqCst);
                return;
            }
        };
        // Fully transparent: never drawn, never in the way of the mouse.
        let _ = SetLayeredWindowAttributes(h, Default::default(), 0, LWA_ALPHA);
        HOST.store(h.0 as isize, Ordering::SeqCst);
        sync(h);
        SetTimer(Some(h), TIMER_SYNC, 500, None);
        // Ask running apps to add their icons again, which now reaches us first.
        broadcast_taskbar_created();
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        HOST.store(0, Ordering::SeqCst);
        ICONS.lock().unwrap().clear();
    }
}

fn broadcast_taskbar_created() {
    *LAST_BROADCAST.lock().unwrap() = Some(Instant::now());
    unsafe {
        let m = RegisterWindowMessageW(w!("TaskbarCreated"));
        let _ = SendNotifyMessageW(HWND_BROADCAST, m, WPARAM(0), LPARAM(0));
    }
}

/// Stay the first `Shell_TrayWnd` in z-order, at the taskbar's place, shown when it is.
unsafe fn sync(h: HWND) {
    let Some(tray) = crate::taskbar::primary_tray() else { return };
    let mut r = RECT::default();
    let _ = GetWindowRect(tray, &mut r);
    let mut ours = RECT::default();
    let _ = GetWindowRect(h, &mut ours);
    let first = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok() == Some(h);
    let want_visible = IsWindowVisible(tray).as_bool();
    let visible = IsWindowVisible(h).as_bool();
    if first && ours == r && visible == want_visible {
        return;
    }
    let mut flags = SWP_NOACTIVATE | if want_visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
    if first {
        flags |= SWP_NOZORDER;
    }
    let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, flags);
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA => {
            let cds = &*(lp.0 as *const COPYDATASTRUCT);
            if cds.dwData == 1 && !cds.lpData.is_null() {
                let data = std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize);
                record(data);
            }
            forward(msg, wp, lp)
        }
        WM_TIMER if wp.0 == TIMER_SYNC => {
            sync(h);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(h);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        // Private taskbar messages that tools send to Shell_TrayWnd.
        WM_COMMAND => forward(msg, wp, lp),
        m if (WM_USER..0xC000).contains(&m) => forward(msg, wp, lp),
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// Pass a message on to Explorer's taskbar the way it reached us, with Explorer's answer.
unsafe fn forward(msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(tray) = crate::taskbar::primary_tray() else { return LRESULT(0) };
    if InSendMessage().as_bool() {
        let mut result = 0usize;
        SendMessageTimeoutW(tray, msg, wp, lp, SMTO_ABORTIFHUNG, 4000, Some(&mut result));
        LRESULT(result as isize)
    } else {
        let _ = PostMessageW(Some(tray), msg, wp, lp);
        LRESULT(0)
    }
}

// Layout of the TRAYNOTIFYDATAW that Shell_NotifyIcon sends: a signature, the NIM_ command and a
// NOTIFYICONDATAW whose handles are stored as 32-bit values.
const OFF_MESSAGE: usize = 4;
const OFF_HWND: usize = 12;
const OFF_UID: usize = 16;
const OFF_FLAGS: usize = 20;
const OFF_CALLBACK: usize = 24;
const OFF_TIP: usize = 32;
const TIP_LEN: usize = 128;
const OFF_VERSION: usize = 808;
const OFF_GUID: usize = 944;

fn u32_at(d: &[u8], off: usize) -> Option<u32> {
    d.get(off..off + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn text_at(d: &[u8], off: usize, len: usize) -> String {
    let Some(b) = d.get(off..off + len * 2) else { return String::new() };
    let w: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|c| *c != 0).collect();
    String::from_utf16_lossy(&w)
}

fn record(d: &[u8]) {
    if u32_at(d, 0) != Some(SIGNATURE) {
        return;
    }
    let (Some(cmd), Some(hwnd), Some(uid), Some(flags)) =
        (u32_at(d, OFF_MESSAGE), u32_at(d, OFF_HWND), u32_at(d, OFF_UID), u32_at(d, OFF_FLAGS))
    else {
        return;
    };
    let hwnd = hwnd as i32 as isize;
    let guid = (flags & NIF_GUID != 0)
        .then(|| d.get(OFF_GUID..OFF_GUID + 16))
        .flatten()
        .map(|b| GUID::from_u128(u128::from_be_bytes(guid_bytes(b))));
    let mut icons = ICONS.lock().unwrap();
    let pos = icons.iter().position(|i| match guid {
        Some(g) => i.guid == Some(g),
        None => i.hwnd == hwnd && i.uid == uid,
    });
    match cmd {
        NIM_DELETE => {
            if let Some(p) = pos {
                icons.remove(p);
            }
        }
        NIM_ADD | NIM_MODIFY | NIM_SETVERSION => {
            let i = match pos {
                Some(p) => &mut icons[p],
                None => {
                    icons.push(Icon { hwnd, uid, guid, msg: 0, version: 0, tip: String::new() });
                    icons.last_mut().unwrap()
                }
            };
            if hwnd != 0 {
                i.hwnd = hwnd;
                i.uid = uid;
            }
            if cmd == NIM_SETVERSION {
                i.version = u32_at(d, OFF_VERSION).unwrap_or(0);
            }
            if flags & NIF_MESSAGE != 0 {
                i.msg = u32_at(d, OFF_CALLBACK).unwrap_or(0);
            }
            if flags & NIF_TIP != 0 {
                i.tip = text_at(d, OFF_TIP, TIP_LEN);
            }
        }
        _ => {}
    }
    icons.retain(|i| unsafe { IsWindow(Some(HWND(i.hwnd as *mut _))).as_bool() });
}

/// A GUID's in-memory bytes (little-endian fields) in the big-endian order `from_u128` wants.
fn guid_bytes(b: &[u8]) -> [u8; 16] {
    [b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// File name of a program path, lowercased.
fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase()
}

fn icon_exe(i: &Icon) -> String {
    crate::win::process_path(crate::win::window_pid(HWND(i.hwnd as *mut _))).map(|p| exe_name(&p)).unwrap_or_default()
}

/// The `nth` icon (0-based, for icons sharing a tooltip) whose tooltip matches `tooltip`, or
/// else that belongs to the program `exe` (some apps leave the tooltip empty).
fn find(tooltip: &str, nth: usize, exe: Option<&str>) -> Option<Icon> {
    let want = norm(tooltip);
    let first = norm(tooltip.lines().next().unwrap_or(""));
    let icons = ICONS.lock().unwrap();
    let usable = || icons.iter().filter(|i| i.msg != 0);
    let mut pick: Vec<&Icon> = usable().filter(|i| norm(&i.tip) == want).collect();
    if pick.is_empty() && !first.is_empty() {
        pick = usable().filter(|i| norm(i.tip.lines().next().unwrap_or("")) == first).collect();
    }
    if pick.is_empty() {
        if let Some(exe) = exe.map(exe_name).filter(|e| !e.is_empty()) {
            pick = usable().filter(|i| icon_exe(i) == exe).collect();
        }
    }
    pick.get(nth).or(pick.first()).map(|i| (*i).clone())
}

/// Send TaskbarCreated straight to every window of the program `exe`, including message-only
/// windows, which the broadcast never reaches (Spotify keeps its tray window there).
fn ask_program(exe: &str) {
    use windows::core::BOOL;
    let exe = exe_name(exe);
    if exe.is_empty() {
        return;
    }
    let mut windows: Vec<HWND> = Vec::new();
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        (*(lp.0 as *mut Vec<HWND>)).push(h);
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut windows as *mut _ as isize));
        let mut prev: Option<HWND> = None;
        while let Ok(h) = FindWindowExW(Some(HWND_MESSAGE), prev, PCWSTR::null(), PCWSTR::null()) {
            if h.is_invalid() {
                break;
            }
            windows.push(h);
            prev = Some(h);
        }
        let m = RegisterWindowMessageW(w!("TaskbarCreated"));
        let mut pids: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
        for h in windows {
            let pid = crate::win::window_pid(h);
            let ours = *pids.entry(pid).or_insert_with(|| crate::win::process_path(pid).is_some_and(|p| exe_name(&p) == exe));
            if ours {
                let _ = SendNotifyMessageW(h, m, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// For an icon whose app never told us its click message (it was running before the bar and
/// ignores TaskbarCreated): bring back the program's main window, which is what a left click
/// on such an icon does. Its window may be hidden while the app sits in the tray.
pub fn show_program(exe: &str) -> bool {
    use windows::core::BOOL;
    let exe = exe_name(exe);
    if exe.is_empty() {
        return false;
    }
    let mut all: Vec<HWND> = Vec::new();
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        (*(lp.0 as *mut Vec<HWND>)).push(h);
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut all as *mut _ as isize));
        let mut pids: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
        let mut main = |h: &HWND| {
            let pid = crate::win::window_pid(*h);
            *pids.entry(pid).or_insert_with(|| crate::win::process_path(pid).is_some_and(|p| exe_name(&p) == exe))
                && GetWindow(*h, GW_OWNER).map_or(true, |o| o.is_invalid())
                && GetWindowLongW(*h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
                && GetWindowLongW(*h, GWL_STYLE) as u32 & WS_CAPTION.0 == WS_CAPTION.0
                && !crate::win::window_title(*h).is_empty()
        };
        let found: Vec<HWND> = all.into_iter().filter(|h| main(h)).collect();
        // Prefer a window that's already showing.
        let Some(h) = found.iter().find(|h| IsWindowVisible(**h).as_bool()).or(found.first()).copied() else {
            return false;
        };
        if !IsWindowVisible(h).as_bool() {
            let _ = ShowWindow(h, SW_SHOW);
        }
        crate::apps::activate(h);
    }
    true
}

/// Left-click the icon with this tooltip like the Windows taskbar would. False when it isn't
/// known (the caller falls back to UI Automation).
pub fn left_click(tooltip: &str, nth: usize, exe: Option<&str>) -> bool {
    if HOST.load(Ordering::SeqCst) <= 0 {
        return false;
    }
    let mut icon = find(tooltip, nth, exe);
    if icon.is_none() {
        // Its app added it before we were listening and didn't hear the broadcast: ask it
        // directly (or everyone, once in a while, when the program isn't known).
        let recent = LAST_BROADCAST.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_secs(30));
        if let Some(exe) = exe {
            ask_program(exe);
        } else if !recent {
            broadcast_taskbar_created();
        }
        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(60));
            icon = find(tooltip, nth, exe);
            if icon.is_some() {
                break;
            }
        }
    }
    let Some(i) = icon else {
        return false;
    };
    crate::dlog!("tray click: sending a click (version {}, tooltip set: {}, program {})", i.version, !i.tip.is_empty(), icon_exe(&i));
    let h = HWND(i.hwnd as *mut _);
    unsafe {
        let _ = AllowSetForegroundWindow(crate::win::window_pid(h));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let send = |m: u32| {
            let (wp, lp) = if i.version >= 4 {
                let wp = (pt.x as u16 as usize) | ((pt.y as u16 as usize) << 16);
                let lp = (m & 0xFFFF) as isize | (((i.uid & 0xFFFF) as isize) << 16);
                (wp, lp)
            } else {
                (i.uid as usize, m as isize)
            };
            let _ = SendNotifyMessageW(h, i.msg, WPARAM(wp), LPARAM(lp));
        };
        send(WM_LBUTTONDOWN);
        send(WM_LBUTTONUP);
        if i.version >= 4 {
            send(NIN_SELECT);
        }
    }
    true
}
