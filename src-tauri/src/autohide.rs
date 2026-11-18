//! Auto-hide: the bar slides off its screen edge and slides back when the cursor touches that
//! edge, like the Windows taskbar's auto-hide. Works for both the pill and the full bar.

use crate::settings::Edge;
use crate::state::{AppState, DockGeom};
use crate::{appbar, win};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE,
};

/// How far (physical px) the window is currently slid off its edge; 0 = fully shown.
static OFFSET: AtomicI32 = AtomicI32::new(0);
/// The clickable part of the dock as the dock last asked for it (window coords, physical px).
/// None = the whole window.
static REGION: Mutex<(Option<RECT>, i32)> = Mutex::new((None, 0));
/// Set by `reset` after the dock was re-placed, so the loop restarts its hide countdown.
static RESET: AtomicBool = AtomicBool::new(false);

const HIDE_DELAY: Duration = Duration::from_millis(650);
const SLIDE_MS: u64 = 200;
/// Cursor within this many px of the screen edge brings the bar back.
const TRIGGER: i32 = 2;

/// The dock asks for its click region through here, so a slid-away bar stays clipped.
pub fn set_region(h: HWND, r: Option<RECT>, radius: i32, d: &DockGeom) {
    *REGION.lock().unwrap() = (r, radius);
    apply_region(h, OFFSET.load(Ordering::SeqCst), (d.w, d.h, d.edge));
}

/// The dock was just put back on its edge at full visibility.
pub fn reset() {
    OFFSET.store(0, Ordering::SeqCst);
    RESET.store(true, Ordering::SeqCst);
}

pub fn is_hidden() -> bool {
    OFFSET.load(Ordering::SeqCst) != 0
}

/// Clip the stored region to the part of the window still on screen.
fn apply_region(h: HWND, off: i32, (w, wh, edge): (i32, i32, Edge)) {
    let (region, radius) = *REGION.lock().unwrap();
    if off == 0 {
        appbar::set_hit_region(h, region, radius);
        return;
    }
    let mut r = region.unwrap_or(RECT { left: 0, top: 0, right: w, bottom: wh });
    match edge {
        Edge::Bottom => r.bottom = r.bottom.min(wh - off),
        Edge::Top => r.top = r.top.max(off),
    }
    if r.bottom <= r.top {
        r = RECT::default();
    }
    appbar::set_hit_region(h, Some(r), radius);
}

fn slide(app: &AppHandle, to: i32) {
    let Some(h) = app.get_webview_window("dock").and_then(|d| win::tauri_hwnd(&d)) else { return };
    let from = OFFSET.load(Ordering::SeqCst);
    if from == to {
        return;
    }
    let d = *app.state::<AppState>().dock.lock().unwrap();
    let frames = (SLIDE_MS / 12).max(1);
    for i in 1..=frames {
        let t = i as f64 / frames as f64;
        // Ease out when showing, ease in when hiding: quick to answer, calm to leave.
        let e = if to < from { 1.0 - (1.0 - t).powi(3) } else { t * t * t * 0.5 + t * 0.5 };
        let off = from + ((to - from) as f64 * e).round() as i32;
        let y = match d.edge {
            Edge::Bottom => d.y + off,
            Edge::Top => d.y - off,
        };
        // Clip before moving down, after moving up, so nothing ever peeks past the edge.
        if to > from {
            apply_region(h, off, (d.w, d.h, d.edge));
        }
        unsafe {
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), d.x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
        }
        if to <= from {
            apply_region(h, off, (d.w, d.h, d.edge));
        }
        OFFSET.store(off, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(12));
    }
}

/// A card, menu or preview is up, or the settings window is open (so you can see your
/// changes on the bar).
fn popups_open(app: &AppHandle) -> bool {
    let shown = |l: &str| {
        app.get_webview_window(l)
            .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
            .unwrap_or(false)
    };
    ["card", "menu", "preview", "settings"].into_iter().any(shown)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_needed = Instant::now();
        loop {
            let st = app.state::<AppState>();
            let enabled = st.settings.lock().unwrap().auto_hide;
            if !enabled || st.fullscreen.load(Ordering::SeqCst) {
                if !enabled && is_hidden() {
                    slide(&app, 0);
                }
                last_needed = Instant::now();
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            std::thread::sleep(Duration::from_millis(40));
            if RESET.swap(false, Ordering::SeqCst) {
                last_needed = Instant::now();
            }

            let d = *st.dock.lock().unwrap();
            let mut pt = POINT::default();
            if unsafe { GetCursorPos(&mut pt) }.is_err() {
                continue;
            }
            let in_x = pt.x >= d.x && pt.x < d.x + d.w;
            let at_edge = in_x
                && match d.edge {
                    Edge::Bottom => pt.y >= d.y + d.h - TRIGGER,
                    Edge::Top => pt.y < d.y + TRIGGER,
                };

            if is_hidden() {
                // Opening Start (or search) brings the bar up with it, like the taskbar.
                let start = win::is_start_ui(unsafe { GetForegroundWindow() });
                if (at_edge && !buttons_down()) || popups_open(&app) || start {
                    slide(&app, 0);
                    last_needed = Instant::now();
                }
                continue;
            }

            // Shown: stay while the cursor is over the bar, a popup or the settings window is
            // open, a drag is going on or Start/search is up.
            let bar = REGION.lock().unwrap().0.unwrap_or(RECT { left: 0, top: 0, right: d.w, bottom: d.h });
            let slack = 6;
            let over_bar = pt.x >= d.x + bar.left - slack
                && pt.x < d.x + bar.right + slack
                && pt.y >= d.y + bar.top - slack
                && pt.y < d.y + bar.bottom + slack;
            let shell = win::is_shell_ui(unsafe { GetForegroundWindow() });
            if over_bar || at_edge || popups_open(&app) || buttons_down() || shell {
                last_needed = Instant::now();
            } else if last_needed.elapsed() >= HIDE_DELAY {
                slide(&app, d.h);
            }
        }
    });
}

fn buttons_down() -> bool {
    let down = |vk: i32| unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
    down(VK_LBUTTON.0 as i32) || down(VK_RBUTTON.0 as i32)
}
