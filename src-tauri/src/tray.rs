//! Tray icons: the apps in the Windows notification area, mirrored on the bar.
//!
//! Explorer stays the tray host. On Windows 11 its tray is XAML; UI Automation still sees its
//! buttons while the taskbar is hidden, so they are read from there and clicks are forwarded
//! to them (the element's context menu for a right click; a left click goes to the app the way
//! the taskbar sends it, see notify.rs, with Invoke as the fallback). UI Automation
//! gives no pixels, so the pictures come from the snapshots Windows keeps of every tray icon
//! under `HKCU\Control Panel\NotifyIconSettings`, matched by tooltip and program name.

use crate::worker::{com_mta, Worker};
use base64::Engine;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{Interface, BSTR, PCWSTR, PWSTR};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_ROUTINE_FLAGS,
    RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationElement3, IUIAutomationInvokePattern,
    TreeScope_Descendants, UIA_ClassNamePropertyId, UIA_InvokePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::FindWindowExW;

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrayIcon {
    /// Stable within a session: the tooltip's first line, plus a counter for duplicates.
    pub id: String,
    pub tooltip: String,
    /// PNG data URL of the icon, from Windows' snapshot.
    pub image: Option<String>,
    /// Icon key (see icons.rs) of the program, for when there is no snapshot.
    pub icon_key: Option<String>,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrayState {
    pub icons: Vec<TrayIcon>,
    /// The "Show hidden icons" chevron exists.
    pub has_overflow: bool,
    /// The Windows tray couldn't be read (e.g. the older Windows 10 tray).
    pub unavailable: bool,
}

#[derive(Clone)]
struct RegIcon {
    tooltip: String,
    exe: Option<String>,
    stem: String,
    promoted: bool,
    snapshot: Option<String>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Option<TrayState>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

thread_local! {
    static UIA: std::cell::RefCell<Option<IUIAutomation>> = const { std::cell::RefCell::new(None) };
    static REG: std::cell::RefCell<(Option<Instant>, Vec<RegIcon>)> = const { std::cell::RefCell::new((None, Vec::new())) };
}

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(com_mta, Duration::from_millis(1500), refresh));
}

pub fn current() -> TrayState {
    CURRENT.lock().unwrap().clone().unwrap_or_default()
}

fn enabled() -> bool {
    APP.get().map(|a| a.state::<crate::state::AppState>().settings.lock().unwrap().has_widget("tray")).unwrap_or(false)
}

// ---------------------------------------------------------------- UI Automation

fn uia() -> windows::core::Result<IUIAutomation> {
    UIA.with(|c| {
        if let Some(u) = c.borrow().as_ref() {
            return Ok(u.clone());
        }
        let u: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        *c.borrow_mut() = Some(u.clone());
        Ok(u)
    })
}

/// The XAML island inside the primary taskbar that holds its buttons.
fn bridge() -> Option<HWND> {
    let tray = crate::taskbar::primary_tray()?;
    let cls = crate::win::wide("Windows.UI.Composition.DesktopWindowContentBridge");
    unsafe { FindWindowExW(Some(tray), None, PCWSTR(cls.as_ptr()), PCWSTR::null()) }.ok().filter(|h| !h.is_invalid())
}

struct Button {
    el: IUIAutomationElement,
    /// The id the frontend knows it by (empty for the chevron).
    id: String,
    tooltip: String,
    overflow: bool,
}

/// The chevron and the app icons in the tray, left to right. None = no XAML tray here.
fn buttons() -> windows::core::Result<Option<Vec<Button>>> {
    let Some(h) = bridge() else { return Ok(None) };
    let u = uia()?;
    unsafe {
        let root = u.ElementFromHandle(h)?;
        let cond = u.CreatePropertyCondition(UIA_ClassNamePropertyId, &VARIANT::from(BSTR::from("SystemTray.NormalButton")))?;
        let all = root.FindAll(TreeScope_Descendants, &cond)?;
        let n = all.Length()?;
        let mut items = Vec::with_capacity(n as usize);
        for i in 0..n {
            let el = all.GetElement(i)?;
            let aid = el.CurrentAutomationId().map(|s| s.to_string()).unwrap_or_default();
            let name = el.CurrentName().map(|s| s.to_string()).unwrap_or_default();
            items.push((el, aid, name));
        }
        let mut out: Vec<Button> = Vec::new();
        for (i, (el, aid, name)) in items.iter().enumerate() {
            if aid == "NotifyItemIcon" {
                let tooltip = name.trim().to_string();
                let first = tooltip.lines().next().unwrap_or("").trim().to_string();
                let dupes = out.iter().filter(|b| !b.overflow && b.id.split('#').next() == Some(first.as_str())).count();
                let id = if dupes == 0 { first } else { format!("{first}#{}", dupes + 1) };
                out.push(Button { el: el.clone(), id, tooltip, overflow: false });
            } else if aid == "SystemTrayIcon" && out.is_empty() {
                // The chevron is the system button right before the app icons.
                let next_is_app = items.get(i + 1).is_some_and(|(_, a, _)| a == "NotifyItemIcon");
                if next_is_app || name.to_lowercase().contains("hidden") {
                    out.push(Button { el: el.clone(), id: String::new(), tooltip: name.clone(), overflow: true });
                }
            }
        }
        Ok(Some(out))
    }
}

fn refresh() {
    let on = enabled();
    crate::notify::set_running(on);
    if !on {
        return;
    }
    let state = match buttons() {
        Ok(Some(list)) => {
            let reg = registry();
            let icons = list
                .iter()
                .filter(|b| !b.overflow)
                .map(|b| {
                    let m = best_match(&reg, b.tooltip.lines().next().unwrap_or(""));
                    TrayIcon {
                        id: b.id.clone(),
                        tooltip: b.tooltip.clone(),
                        image: m.and_then(|r| r.snapshot.clone()),
                        icon_key: m.and_then(|r| r.exe.as_ref()).map(|p| format!("path:{p}")),
                    }
                })
                .collect();
            TrayState { icons, has_overflow: list.iter().any(|b| b.overflow), unavailable: false }
        }
        Ok(None) => TrayState { unavailable: true, ..Default::default() },
        Err(e) => {
            crate::dlog!("tray: {e}");
            UIA.with(|c| *c.borrow_mut() = None);
            TrayState { unavailable: true, ..Default::default() }
        }
    };
    let mut cur = CURRENT.lock().unwrap();
    if cur.as_ref() != Some(&state) {
        *cur = Some(state.clone());
        if let Some(app) = APP.get() {
            let _ = app.emit("tray-changed", state);
        }
    }
}

/// Forward a click to a tray icon: `button` is "left" or "right"; "overflow" opens the
/// Windows hidden-icons flyout, moved next to the bar at screen-x `center` when given.
pub fn click(id: String, button: String, center: Option<i32>) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || {
        let Ok(Some(list)) = buttons() else { return };
        let target = list.into_iter().find(|b| if button == "overflow" { b.overflow } else { !b.overflow && b.id == id });
        let Some(b) = target else { return };
        if button == "left" {
            // Icons sharing a tooltip have ids like "Name#2".
            let nth = b.id.rsplit_once('#').and_then(|(_, n)| n.parse::<usize>().ok()).map_or(0, |n| n.saturating_sub(1));
            let reg = registry();
            let exe = best_match(&reg, b.tooltip.lines().next().unwrap_or("")).and_then(|r| r.exe.clone());
            if crate::notify::left_click(&b.tooltip, nth, exe.as_deref()) {
                return;
            }
            if exe.as_deref().is_some_and(crate::notify::show_program) {
                crate::dlog!("tray click: click message unknown, showed the app's window");
                return;
            }
            crate::dlog!("tray click: click message unknown, using UI Automation");
        }
        let r = unsafe {
            if button == "right" {
                b.el.cast::<IUIAutomationElement3>().and_then(|e| e.ShowContextMenu())
            } else {
                b.el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId).and_then(|p| p.Invoke())
            }
        };
        if let Err(e) = r {
            crate::dlog!("tray click ({button}): {e}");
        }
        if button == "overflow" {
            if let Some(c) = center {
                move_flyout(c);
            }
        }
        refresh();
    });
}

/// Windows places the hidden-icons flyout over its own (hidden) taskbar; move it next to ours.
fn move_flyout(center: i32) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowRect, IsWindowVisible, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    let Some(app) = APP.get() else { return };
    let classes = ["TopLevelWindowForOverflowXamlIsland", "NotifyIconOverflowWindow"];
    let mut placed = 0;
    // It appears a moment after the click, and Explorer may place it once more as it opens.
    for _ in 0..40 {
        for cls in classes {
            let c = crate::win::wide(cls);
            let Ok(h) = (unsafe { FindWindowW(PCWSTR(c.as_ptr()), PCWSTR::null()) }) else { continue };
            if h.is_invalid() || !unsafe { IsWindowVisible(h) }.as_bool() {
                continue;
            }
            let mut r = RECT::default();
            if unsafe { GetWindowRect(h, &mut r) }.is_err() {
                continue;
            }
            let (w, ht) = (r.right - r.left, r.bottom - r.top);
            if w <= 0 || ht <= 0 {
                continue;
            }
            let (x, y) = crate::popups::content_position(app, center, w, ht);
            if (r.left, r.top) != (x, y) {
                unsafe {
                    let _ = SetWindowPos(h, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                }
            }
            placed += 1;
        }
        if placed >= 8 {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

// ---------------------------------------------------------------- icon snapshots

fn norm(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' }).collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn best_match<'a>(reg: &'a [RegIcon], tooltip: &str) -> Option<&'a RegIcon> {
    let name = norm(tooltip);
    if name.is_empty() {
        return None;
    }
    let overlaps = |a: &str, b: &str| a.len() >= 3 && b.len() >= 3 && (a.contains(b) || b.contains(a));
    let words: Vec<&str> = name.split(' ').filter(|w| w.len() >= 4).collect();
    let score = |r: &RegIcon| {
        let tip = norm(&r.tooltip);
        let mut s = if !tip.is_empty() && tip == name {
            100
        } else if overlaps(&tip, &name) {
            80
        } else if overlaps(&r.stem, &name) {
            60
        } else if words.iter().any(|w| r.stem.contains(w) || tip.contains(w)) {
            40
        } else {
            return 0;
        };
        if r.promoted {
            s += 5;
        }
        if r.snapshot.is_some() {
            s += 2;
        }
        s
    };
    reg.iter().map(|r| (score(r), r)).filter(|(s, _)| *s >= 40).max_by_key(|(s, _)| *s).map(|(_, r)| r)
}

/// Windows' list of tray icons, re-read at most every 20 s.
fn registry() -> Vec<RegIcon> {
    REG.with(|c| {
        let mut c = c.borrow_mut();
        if c.0.is_none_or(|t| t.elapsed() > Duration::from_secs(20)) {
            c.1 = read_registry();
            c.0 = Some(Instant::now());
        }
        c.1.clone()
    })
}

/// Paths are stored with a known-folder GUID in place of the folder.
fn expand_known_folder(p: &str) -> String {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let map = [
        ("{6D809377-6AF0-444B-8957-A3773F02200E}", env("ProgramW6432")),
        ("{905E63B6-C1BF-494E-B29C-65B732D3D21A}", env("ProgramFiles")),
        ("{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}", env("ProgramFiles(x86)")),
        ("{F38BF404-1D43-42F2-9305-67DE0B28FC23}", env("SystemRoot")),
        ("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}", format!("{}\\System32", env("SystemRoot"))),
        ("{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}", format!("{}\\SysWOW64", env("SystemRoot"))),
    ];
    for (guid, dir) in map {
        if p.len() >= guid.len() && p[..guid.len()].eq_ignore_ascii_case(guid) && !dir.is_empty() {
            return format!("{dir}{}", &p[guid.len()..]);
        }
    }
    p.to_string()
}

fn read_registry() -> Vec<RegIcon> {
    let mut out = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        let path = crate::win::wide("Control Panel\\NotifyIconSettings");
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), None, KEY_READ, &mut key).is_err() {
            return out;
        }
        let mut i = 0;
        loop {
            let mut name = [0u16; 128];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
                break;
            }
            i += 1;
            let mut sub = name[..len as usize].to_vec();
            sub.push(0);
            let read = |value: &str, flags: REG_ROUTINE_FLAGS| -> Option<Vec<u8>> {
                let v = crate::win::wide(value);
                let mut size = 0u32;
                RegGetValueW(key, PCWSTR(sub.as_ptr()), PCWSTR(v.as_ptr()), flags, None, None, Some(&mut size)).ok().ok()?;
                let mut buf = vec![0u8; size as usize];
                RegGetValueW(key, PCWSTR(sub.as_ptr()), PCWSTR(v.as_ptr()), flags, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size))
                    .ok()
                    .ok()?;
                buf.truncate(size as usize);
                Some(buf)
            };
            let text = |b: Vec<u8>| {
                let w: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|c| *c != 0).collect();
                String::from_utf16_lossy(&w)
            };
            let exe = read("ExecutablePath", RRF_RT_REG_SZ).map(text).map(|p| expand_known_folder(&p));
            let tooltip = read("InitialTooltip", RRF_RT_REG_SZ).map(text).unwrap_or_default();
            let promoted = read("IsPromoted", RRF_RT_REG_DWORD).is_some_and(|b| b.first().copied().unwrap_or(0) != 0);
            let snapshot = read("IconSnapshot", RRF_RT_REG_BINARY)
                .filter(|b| b.starts_with(&[0x89, b'P', b'N', b'G']))
                .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
            let stem = exe
                .as_deref()
                .map(|p| {
                    let file = p.rsplit(['\\', '/']).next().unwrap_or("");
                    norm(file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE")).unwrap_or(file))
                })
                .unwrap_or_default();
            out.push(RegIcon { tooltip: tooltip.lines().next().unwrap_or("").to_string(), exe, stem, promoted, snapshot });
        }
        let _ = RegCloseKey(key);
    }
    out
}
