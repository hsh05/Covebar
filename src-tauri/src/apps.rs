//! The Apps widget backend: open windows grouped by app, taskbar pins, and window actions.

use crate::icons;
use crate::win::{self, hval, hwnd, wide};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::core::{Interface, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, WIN32_FIND_DATAW,
};
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_LWIN,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::UI::Shell::{
    IShellLinkW, SHFileOperationW, SHParseDisplayName, ShellExecuteW, ShellLink, FOF_ALLOWUNDO,
    FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WinInfo {
    pub hwnd: isize,
    pub title: String,
    pub minimized: bool,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DockApp {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub pinned: bool,
    pub launch: Option<String>,
    /// The program file, for "Open file location" (None for Store apps).
    pub location: Option<String>,
    pub windows: Vec<WinInfo>,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppsSnapshot {
    pub apps: Vec<DockApp>,
    pub active: isize,
}

static DIRTY: AtomicBool = AtomicBool::new(true);
/// Pins changed from the dock: re-read the pins folder on the next tick.
static PINS_DIRTY: AtomicBool = AtomicBool::new(false);
static LAST_FG: AtomicIsize = AtomicIsize::new(0);
static SEQ: AtomicU64 = AtomicU64::new(1);
/// Whether Start/search was the foreground window, and when it last stopped being it.
static SHELL_FG: AtomicBool = AtomicBool::new(false);
static SHELL_LOST_AT: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Default)]
struct Book {
    /// When each window was first seen, for a stable order.
    first_seen: HashMap<isize, u64>,
    /// When each window was last in the foreground.
    last_active: HashMap<isize, u64>,
    /// Order in which unpinned app groups first appeared.
    group_order: Vec<String>,
    names: HashMap<String, String>,
    pids: HashMap<u32, Option<String>>,
}

static BOOK: Mutex<Option<Book>> = Mutex::new(None);

fn with_book<R>(f: impl FnOnce(&mut Book) -> R) -> R {
    f(BOOK.lock().unwrap().get_or_insert_with(Book::default))
}

/// The last foreground window that isn't one of ours.
pub fn last_foreground() -> isize {
    LAST_FG.load(Ordering::Relaxed)
}

fn note_foreground(h: HWND) {
    if h.is_invalid() || win::window_pid(h) == std::process::id() {
        return;
    }
    LAST_FG.store(hval(h), Ordering::Relaxed);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    with_book(|b| b.last_active.insert(hval(h), seq));
}

// ---------------------------------------------------------------- window events

const EVENT_OBJECT_CLOAKED: u32 = 0x8017;
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;

unsafe extern "system" fn win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    h: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 {
        return;
    }
    if event == EVENT_SYSTEM_FOREGROUND {
        note_foreground(h);
        let shell = win::is_shell_ui(h);
        if SHELL_FG.swap(shell, Ordering::SeqCst) && !shell {
            *SHELL_LOST_AT.lock().unwrap() = Some(Instant::now());
        }
    }
    DIRTY.store(true, Ordering::Relaxed);
}

unsafe extern "system" fn own_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _h: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object == OBJID_WINDOW.0 && SHELL_FG.swap(false, Ordering::SeqCst) {
        *SHELL_LOST_AT.lock().unwrap() = Some(Instant::now());
    }
}

/// Start the event hook thread and the refresh loop that emits `apps-changed`.
pub fn start(app: AppHandle) {
    note_foreground(unsafe { GetForegroundWindow() });

    std::thread::spawn(|| unsafe {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        for (a, b) in [
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE),
            (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
        ] {
            SetWinEventHook(a, b, None, Some(win_event), 0, 0, flags);
        }
        // Also see our own windows take the foreground, so we know when a click on the
        // dock is what just dismissed Start.
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(own_foreground),
            std::process::id(),
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });

    std::thread::spawn(move || {
        icons::com_init();
        let mut last = AppsSnapshot::default();
        let mut last_full = Instant::now();
        let mut pins = read_pins();
        let mut pins_read = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(120));
            if PINS_DIRTY.swap(false, Ordering::Relaxed) || pins_read.elapsed() > Duration::from_secs(10) {
                pins = read_pins();
                pins_read = Instant::now();
                DIRTY.store(true, Ordering::Relaxed);
            }
            if !DIRTY.swap(false, Ordering::Relaxed) && last_full.elapsed() < Duration::from_secs(2) {
                continue;
            }
            last_full = Instant::now();
            let snap = build_snapshot(&pins);
            if snap != last {
                *CURRENT.lock().unwrap() = snap.clone();
                let _ = app.emit("apps-changed", &snap);
                last = snap;
            }
        }
    });
}

static CURRENT: Mutex<AppsSnapshot> = Mutex::new(AppsSnapshot { apps: Vec::new(), active: 0 });

pub fn current() -> AppsSnapshot {
    let snap = CURRENT.lock().unwrap().clone();
    if snap.apps.is_empty() {
        icons::com_init();
        build_snapshot(&read_pins())
    } else {
        snap
    }
}

// ---------------------------------------------------------------- enumeration

struct RawWin {
    hwnd: isize,
    title: String,
    minimized: bool,
    exe: String,
    aumid: Option<String>,
}

fn is_cloaked(h: HWND) -> bool {
    let mut v: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut v as *mut _ as *mut _, 4).is_ok() && v != 0
    }
}

/// Alt+Tab-style filter for "real" app windows.
fn is_app_window(h: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(h).as_bool() || is_cloaked(h) {
            return false;
        }
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        let app_window = ex & WS_EX_APPWINDOW.0 != 0;
        if ex & WS_EX_TOOLWINDOW.0 != 0 && !app_window {
            return false;
        }
        if ex & WS_EX_NOACTIVATE.0 != 0 && !app_window {
            return false;
        }
        let owner = GetWindow(h, GW_OWNER).unwrap_or_default();
        if !owner.is_invalid() && !app_window {
            return false;
        }
    }
    !win::window_title(h).is_empty()
}

pub fn window_aumid(h: HWND) -> Option<String> {
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(h).ok()?;
        let v = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
        let s = v.to_string();
        (!s.is_empty()).then_some(s)
    }
}

unsafe extern "system" fn enum_cb(h: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<HWND>);
    out.push(h);
    true.into()
}

fn list_windows() -> Vec<RawWin> {
    let mut all: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut all as *mut _ as isize));
    }
    let own = std::process::id();
    let mut out = Vec::new();
    for h in all {
        if !is_app_window(h) {
            continue;
        }
        let pid = win::window_pid(h);
        if pid == own {
            continue;
        }
        let exe = with_book(|b| b.pids.get(&pid).cloned())
            .unwrap_or_else(|| {
                let p = win::process_path(pid);
                with_book(|b| b.pids.insert(pid, p.clone()));
                p
            })
            .unwrap_or_default();
        out.push(RawWin {
            hwnd: hval(h),
            title: win::window_title(h),
            minimized: unsafe { IsIconic(h).as_bool() },
            exe,
            aumid: window_aumid(h),
        });
    }
    out
}

fn exe_description(path: &str) -> Option<String> {
    let w = wide(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, data.as_mut_ptr() as *mut _).ok()?;
        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tq = wide("\\VarFileInfo\\Translation");
        if !VerQueryValueW(data.as_ptr() as *const _, PCWSTR(tq.as_ptr()), &mut ptr, &mut len).as_bool()
            || len < 4
        {
            return None;
        }
        let lang = *(ptr as *const u16);
        let cp = *(ptr as *const u16).add(1);
        let q = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription"));
        if !VerQueryValueW(data.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut ptr, &mut len).as_bool()
            || len == 0
        {
            return None;
        }
        let s = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let s = String::from_utf16_lossy(s).trim_end_matches('\0').trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn app_name(key: &str, aumid: Option<&str>, exe: &str) -> String {
    if let Some(n) = with_book(|b| b.names.get(key).cloned()) {
        return n;
    }
    let name = aumid
        .and_then(|a| icons::display_name(&format!("shell:AppsFolder\\{a}")))
        .or_else(|| exe_description(exe))
        .unwrap_or_else(|| {
            std::path::Path::new(exe)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "App".into())
        });
    with_book(|b| b.names.insert(key.to_string(), name.clone()));
    name
}

fn icon_key(aumid: Option<&str>, exe: &str) -> String {
    match aumid {
        Some(a) if !exe.is_empty() => format!("aumid:{a}|path:{exe}"),
        Some(a) => format!("aumid:{a}"),
        None => format!("path:{exe}"),
    }
}

// ---------------------------------------------------------------- pins

#[derive(Clone, Debug)]
pub struct Pin {
    lnk: String,
    name: String,
    target: Option<String>,
    aumid: Option<String>,
    /// For pins that start an app through a launcher (Discord, Slack, Teams classic and other
    /// Squirrel apps run `Update.exe --processStart App.exe`): the folder the real program
    /// lives under and its file name, both lowercase.
    launched: Option<(String, String)>,
}

/// `Update.exe --processStart App.exe` -> (Update.exe's folder, "app.exe").
fn launched_program(target: Option<&str>, args: &str) -> Option<(String, String)> {
    let target = target?;
    if win::file_stem_lower(target) != "update" {
        return None;
    }
    let mut it = args.split_whitespace();
    while let Some(a) = it.next() {
        if a.eq_ignore_ascii_case("--processStart") || a.eq_ignore_ascii_case("--processStartAndWait") {
            let exe = it.next()?.trim_matches('"').to_lowercase();
            let dir = std::path::Path::new(target).parent()?.to_string_lossy().to_lowercase();
            return Some((dir, exe));
        }
    }
    None
}

fn pins_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| {
        PathBuf::from(a).join("Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar")
    })
}

/// Read the user's Windows taskbar pins (the .lnk files Explorer keeps for them).
pub fn read_pins() -> Vec<Pin> {
    let Some(dir) = pins_dir() else { return vec![] };
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("lnk")).unwrap_or(false))
        .collect();
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.created()).ok());
    files
        .into_iter()
        .map(|p| {
            let lnk = p.to_string_lossy().to_string();
            let (target, aumid, args) = resolve_link(&lnk);
            Pin {
                name: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                launched: launched_program(target.as_deref(), &args),
                lnk,
                target,
                aumid,
            }
        })
        .collect()
}

/// A shortcut's target program, AppUserModelID and arguments.
fn resolve_link(lnk: &str) -> (Option<String>, Option<String>, String) {
    unsafe {
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) else {
            return (None, None, String::new());
        };
        let w = wide(lnk);
        let Ok(pf) = link.cast::<IPersistFile>() else { return (None, None, String::new()) };
        if pf.Load(PCWSTR(w.as_ptr()), STGM_READ).is_err() {
            return (None, None, String::new());
        }
        let mut abuf = [0u16; 1024];
        let args = link
            .GetArguments(&mut abuf)
            .ok()
            .map(|_| String::from_utf16_lossy(&abuf[..abuf.iter().position(|&c| c == 0).unwrap_or(0)]))
            .unwrap_or_default();
        let mut buf = [0u16; 1024];
        let mut fd = WIN32_FIND_DATAW::default();
        let target = link
            .GetPath(&mut buf, &mut fd, 0)
            .ok()
            .map(|_| String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]))
            .filter(|s| !s.is_empty());
        let aumid = link
            .cast::<IPropertyStore>()
            .ok()
            .and_then(|s| s.GetValue(&PKEY_AppUserModel_ID).ok())
            .map(|v| v.to_string())
            .filter(|s| !s.is_empty());
        (target, aumid, args)
    }
}

// ---------------------------------------------------------------- snapshot

fn build_snapshot(pins: &[Pin]) -> AppsSnapshot {
    let raws = list_windows();

    // Group windows by AppUserModelID, else by executable path.
    let mut groups: Vec<(String, Option<String>, String, Vec<WinInfo>)> = Vec::new();
    with_book(|b| {
        let live: std::collections::HashSet<isize> = raws.iter().map(|r| r.hwnd).collect();
        b.first_seen.retain(|h, _| live.contains(h));
        b.last_active.retain(|h, _| live.contains(h));
        for r in &raws {
            b.first_seen
                .entry(r.hwnd)
                .or_insert_with(|| SEQ.fetch_add(1, Ordering::Relaxed));
        }
    });
    let mut raws = raws;
    with_book(|b| raws.sort_by_key(|r| b.first_seen.get(&r.hwnd).copied().unwrap_or(0)));
    for r in raws {
        let key = r.aumid.clone().unwrap_or_else(|| r.exe.to_lowercase());
        let info = WinInfo { hwnd: r.hwnd, title: r.title, minimized: r.minimized };
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.3.push(info),
            None => groups.push((key, r.aumid, r.exe, vec![info])),
        }
    }

    let mut apps = Vec::new();
    for pin in pins {
        let tgt = pin.target.as_ref().map(|t| t.to_lowercase());
        let mut windows = Vec::new();
        groups.retain(|(key, aumid, exe, wins)| {
            let explorer = pin.aumid.as_deref() == Some("Microsoft.Windows.Explorer")
                && aumid.is_none()
                && win::file_stem_lower(exe) == "explorer";
            let hit = explorer
                || (pin.aumid.is_some() && pin.aumid == *aumid)
                || (aumid.is_none() && tgt.as_deref() == Some(exe.to_lowercase().as_str()))
                || (pin.aumid.is_some() && pin.aumid.as_deref() == Some(key.as_str()))
                || pin.launched.as_ref().is_some_and(|(dir, name)| {
                    let e = exe.to_lowercase();
                    e.starts_with(dir.as_str()) && e.rsplit(['\\', '/']).next() == Some(name.as_str())
                });
            if hit {
                windows.extend(wins.iter().cloned());
            }
            !hit
        });
        apps.push(DockApp {
            key: format!("pin:{}", pin.lnk),
            name: pin.name.clone(),
            icon: format!("path:{}", pin.lnk),
            pinned: true,
            launch: Some(pin.lnk.clone()),
            location: pin.target.clone().filter(|t| is_program_file(t)),
            windows,
        });
    }

    with_book(|b| {
        for g in &groups {
            if !b.group_order.contains(&g.0) {
                b.group_order.push(g.0.clone());
            }
        }
        let order = b.group_order.clone();
        groups.sort_by_key(|g| order.iter().position(|k| *k == g.0).unwrap_or(usize::MAX));
    });
    for (key, aumid, exe, windows) in groups {
        let launch = match &aumid {
            Some(a) => Some(format!("shell:AppsFolder\\{a}")),
            None if !exe.is_empty() => Some(exe.clone()),
            None => None,
        };
        apps.push(DockApp {
            name: app_name(&key, aumid.as_deref(), &exe),
            icon: icon_key(aumid.as_deref(), &exe),
            key,
            pinned: false,
            location: Some(exe.clone()).filter(|e| is_program_file(e)),
            launch,
            windows,
        });
    }
    AppsSnapshot { apps, active: last_foreground() }
}

// ---------------------------------------------------------------- actions

pub fn activate(h: HWND) {
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if !SetForegroundWindow(h).as_bool() {
            // Fall back: a no-op Alt press unlocks SetForegroundWindow for this process.
            let alt = |up: bool| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VK_MENU,
                        dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                        ..Default::default()
                    },
                },
            };
            SendInput(&[alt(false), alt(true)], std::mem::size_of::<INPUT>() as i32);
            let _ = SetForegroundWindow(h);
        }
    }
    note_foreground(h);
    DIRTY.store(true, Ordering::Relaxed);
}

/// Taskbar-style click on an app: restore/focus, minimize if already active,
/// or cycle when the app has several windows.
pub fn click_app(hwnds: &[isize]) {
    if hwnds.is_empty() {
        return;
    }
    let fg = last_foreground();
    let alive: Vec<isize> = hwnds
        .iter()
        .copied()
        .filter(|&h| unsafe { IsWindow(Some(hwnd(h))).as_bool() })
        .collect();
    if alive.is_empty() {
        return;
    }
    if alive.len() == 1 {
        let h = hwnd(alive[0]);
        if alive[0] == fg && unsafe { !IsIconic(h).as_bool() } {
            unsafe {
                let _ = ShowWindow(h, SW_MINIMIZE);
            }
            DIRTY.store(true, Ordering::Relaxed);
        } else {
            activate(h);
        }
        return;
    }
    let target = if let Some(i) = alive.iter().position(|&h| h == fg) {
        alive[(i + 1) % alive.len()]
    } else {
        with_book(|b| {
            *alive
                .iter()
                .max_by_key(|h| b.last_active.get(h).copied().unwrap_or(0))
                .unwrap()
        })
    };
    activate(hwnd(target));
}

/// Bring the app playing media (by its media-session app id) to the front, most recently
/// used window first; start it if it has no window.
pub fn open_media_source(aumid: &str) {
    let tokens = crate::audio::app_tokens(aumid);
    let want = aumid.to_lowercase();
    let wins: Vec<isize> = list_windows()
        .into_iter()
        .filter(|w| {
            if w.aumid.as_deref().is_some_and(|a| a.to_lowercase() == want) {
                return true;
            }
            let path = w.exe.to_lowercase();
            let stem = win::file_stem_lower(&path);
            !stem.is_empty() && tokens.iter().any(|t| *t == stem || path.contains(t.as_str()) || t.contains(stem.as_str()))
        })
        .map(|w| w.hwnd)
        .collect();
    let best = with_book(|b| wins.iter().copied().max_by_key(|h| b.last_active.get(h).copied().unwrap_or(0)));
    if let Some(h) = best {
        crate::dlog!("media source {aumid}: activating window");
        activate(hwnd(h));
        return;
    }
    // No window (e.g. Spotify closed to its tray icon): starting the program again makes a
    // single-instance app show its window. Store apps start through their app id.
    if aumid.contains('!') {
        crate::dlog!("media source {aumid}: launching app id");
        launch(&format!("shell:AppsFolder\\{aumid}"));
    } else if let Some(exe) = running_exe(&tokens) {
        crate::dlog!("media source {aumid}: relaunching its program");
        launch(&exe);
    } else {
        crate::dlog!("media source {aumid}: nothing to open");
    }
}

/// Path of a running program whose file name matches one of `tokens`.
fn running_exe(tokens: &[String]) -> Option<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = None;
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
            let stem = name.trim_end_matches(".exe");
            if tokens.iter().any(|t| t == stem) {
                if let Some(p) = win::process_path(e.th32ProcessID) {
                    found = Some(p);
                    break;
                }
            }
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        found
    }
}

pub fn close_window(h: isize) {
    unsafe {
        let _ = PostMessageW(Some(hwnd(h)), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

pub fn launch(target: &str) {
    let op = wide("open");
    let t = wide(target);
    unsafe {
        ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(t.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

/// Run a program or shortcut elevated (Windows shows its UAC prompt).
pub fn run_as_admin(target: &str) {
    if target.starts_with("shell:") {
        return;
    }
    let op = wide("runas");
    let t = wide(target);
    unsafe {
        ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(t.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

fn is_program_file(p: &str) -> bool {
    !p.to_ascii_lowercase().contains("\\windowsapps\\") && std::path::Path::new(p).is_file()
}

/// Open Explorer with the program file selected.
pub fn open_location(path: &str) {
    if !is_program_file(path) {
        return;
    }
    let op = wide("open");
    let exe = wide("explorer.exe");
    let args = wide(&format!("/select,\"{path}\""));
    unsafe {
        ShellExecuteW(None, PCWSTR(op.as_ptr()), PCWSTR(exe.as_ptr()), PCWSTR(args.as_ptr()), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

/// Minimize or restore a set of windows.
pub fn show_windows(hwnds: &[isize], minimize: bool) {
    for &h in hwnds {
        let h = hwnd(h);
        unsafe {
            if IsWindow(Some(h)).as_bool() {
                let _ = ShowWindow(h, if minimize { SW_MINIMIZE } else { SW_RESTORE });
            }
        }
    }
    DIRTY.store(true, Ordering::Relaxed);
}

/// Pin an app by adding a shortcut to the taskbar pins folder (the one this dock reads).
pub fn pin(launch: &str, name: &str, location: Option<String>) -> Result<(), String> {
    let dir = pins_dir().ok_or("No pins folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let path = dir.join(format!("{}.lnk", safe.trim()));
    if path.exists() {
        PINS_DIRTY.store(true, Ordering::Relaxed);
        return Ok(());
    }
    let (launch, path) = (launch.to_string(), path.to_string_lossy().to_string());
    // COM work on a thread of its own with COM set up.
    std::thread::spawn(move || unsafe {
        icons::com_init();
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        let set_path = |target: &str| -> Result<(), String> {
            let w = wide(target);
            link.SetPath(PCWSTR(w.as_ptr())).map_err(|e| e.to_string())?;
            if let Some(parent) = std::path::Path::new(target).parent() {
                let d = wide(&parent.to_string_lossy());
                let _ = link.SetWorkingDirectory(PCWSTR(d.as_ptr()));
            }
            Ok(())
        };
        let aumid = launch.strip_prefix("shell:AppsFolder").map(|a| a.trim_start_matches(['\\', '/']));
        if let Some(aumid) = aumid {
            // Store apps resolve through the Apps folder; some desktop apps register an app ID
            // that it can't resolve, so fall back to their program file.
            let w = wide(&launch);
            let mut pidl = std::ptr::null_mut();
            let parsed = SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None).is_ok();
            if parsed {
                let r = link.SetIDList(pidl);
                CoTaskMemFree(Some(pidl as *const _));
                r.map_err(|e| e.to_string())?;
            } else {
                set_path(location.as_deref().ok_or("Can't find this app's program")?)?;
            }
            // The app ID lets the pin pick up the app's open windows.
            let store: IPropertyStore = link.cast().map_err(|e| e.to_string())?;
            store
                .SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(aumid))
                .and_then(|_| store.Commit())
                .map_err(|e| e.to_string())?;
        } else {
            set_path(&launch)?;
        }
        let pf: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        let p = wide(&path);
        pf.Save(PCWSTR(p.as_ptr()), true).map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "Pinning failed".to_string())??;
    PINS_DIRTY.store(true, Ordering::Relaxed);
    Ok(())
}

/// Unpin: move the pin's shortcut to the Recycle Bin (only shortcuts inside the pins folder).
pub fn unpin(lnk: &str) -> Result<(), String> {
    let dir = pins_dir().ok_or("No pins folder")?;
    let p = std::path::Path::new(lnk);
    let inside = p.parent().map(|d| d.eq(dir.as_path())).unwrap_or(false);
    if !inside || !p.is_file() || !lnk.to_ascii_lowercase().ends_with(".lnk") {
        return Err("Not a pin".into());
    }
    let mut from: Vec<u16> = lnk.encode_utf16().collect();
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI).0 as u16,
        ..Default::default()
    };
    let r = unsafe { SHFileOperationW(&mut op) };
    PINS_DIRTY.store(true, Ordering::Relaxed);
    if r != 0 { Err(format!("Couldn't remove the pin ({r})")) } else { Ok(()) }
}

/// Toggle the Start menu by pressing the Windows key. If clicking the dock is what just
/// closed Start, leave it closed.
pub fn open_start() {
    let just_closed = SHELL_LOST_AT
        .lock()
        .unwrap()
        .take()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(600));
    if just_closed {
        return;
    }
    let key = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    unsafe {
        SendInput(&[key(false), key(true)], std::mem::size_of::<INPUT>() as i32);
    }
}

/// Debug aid: write every dock app's icon (as the bar would get it) to `dir` as PNG files.
pub fn dump_icons(dir: &str, size: i32) {
    icons::com_init();
    let snap = build_snapshot(&read_pins());
    let _ = std::fs::create_dir_all(dir);
    for (i, a) in snap.apps.iter().enumerate() {
        let url = icons::icon_data_url(&a.icon, size);
        println!("{i:02} pinned={} name={:?} key={:?} ok={}", a.pinned, a.name, a.icon, url.is_some());
        if let Some(b64) = url.as_deref().and_then(|u| u.split_once(',')).map(|(_, b)| b) {
            use base64::Engine;
            if let Ok(png) = base64::engine::general_purpose::STANDARD.decode(b64) {
                let _ = std::fs::write(format!("{dir}/{i:02}.png"), png);
            }
        }
    }
}
