//! Dock placement, look (theme + Mica/Acrylic), taskbar hiding and the fullscreen watcher.

use crate::settings::{Material, Settings, Shape, ThemePref};
use crate::state::{AppState, DockGeom};
use crate::{appbar, popups, taskbar, win};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager, Theme};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONULL};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongW, GetWindowRect, IsWindowVisible, IsZoomed, GWL_STYLE, WS_CAPTION,
};

const WINDOWS: [&str; 5] = ["dock", "card", "preview", "menu", "settings"];

/// Reserve space with the AppBar and move the dock into it.
/// The bar has keyboard focus from the "focus the bar" shortcut: auto-hide keeps it up.
pub static KEYBOARD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Give the bar keyboard focus so its tiles can be reached with the arrow keys.
pub fn focus_bar(app: &AppHandle) {
    let Some(w) = app.get_webview_window("dock") else { return };
    let Some(h) = win::tauri_hwnd(&w) else { return };
    popups::close_card(app, None);
    popups::close_menu(app);
    KEYBOARD.store(true, Ordering::SeqCst);
    crate::apps::activate(h);
    let _ = w.set_focus();
    let _ = tauri::Emitter::emit_to(app, "dock", "dock-focus", ());
}

/// The bar lost keyboard focus; with `restore`, go back to the window that had it before.
pub fn leave_bar(restore: bool) {
    KEYBOARD.store(false, Ordering::SeqCst);
    let fg = crate::apps::last_foreground();
    if restore && fg != 0 {
        crate::apps::activate(win::hwnd(fg));
    }
}

pub fn place(app: &AppHandle) {
    let st = app.state::<AppState>();
    let s = st.settings.lock().unwrap().clone();
    let Some(dock) = app.get_webview_window("dock") else { return };
    let Some(h) = win::tauri_hwnd(&dock) else { return };
    let scale = win::monitor_scale(win::primary_monitor());
    let px = |dip: f64| (dip * scale).round() as i32;
    let rc = appbar::place(h, s.edge, px(s.reserve_dip()), px(s.window_dip()));
    *st.dock.lock().unwrap() = DockGeom {
        x: rc.left,
        y: rc.top,
        w: rc.right - rc.left,
        h: rc.bottom - rc.top,
        scale,
        edge: s.edge,
        inset: px(s.inset_dip()),
    };
    crate::autohide::reset();
}

/// Apply theme and backdrop material to every open Covebar window.
pub fn apply_look(app: &AppHandle, s: &Settings) {
    let theme = match s.theme {
        ThemePref::System => None,
        ThemePref::Dark => Some(Theme::Dark),
        ThemePref::Light => Some(Theme::Light),
    };
    let win11 = win::is_windows_11();
    for label in WINDOWS {
        let Some(w) = app.get_webview_window(label) else { continue };
        let _ = w.set_theme(theme);
        // Popups draw their own surface (so they can animate); only the bar and the
        // settings window get a system backdrop. Mica only exists on Windows 11.
        let effect = match (label, s.material) {
            ("card" | "preview" | "menu", _) => continue,
            // The pill draws its own rounded surface (glass included): a system backdrop
            // ignores the rounded window region and paints the strip around it black.
            ("dock", _) if s.shape == Shape::Pill => {
                let _ = w.set_effects(None::<tauri::utils::config::WindowEffectsConfig>);
                continue;
            }
            // Glass on the full bar: a clear blur of the desktop behind it.
            ("dock", Material::Glass) => Effect::Blur,
            ("dock", Material::Mica) if win11 => Effect::Mica,
            ("settings", _) if win11 => Effect::Mica,
            _ => Effect::Acrylic,
        };
        let _ = w.set_effects(EffectsBuilder::new().effect(effect).build());
    }
}

pub fn set_taskbar_hidden(app: &AppHandle, hide: bool) {
    let st = app.state::<AppState>();
    if hide {
        taskbar::hide(&st.marker_path);
        crate::watchdog::spawn(&st.marker_path);
    } else {
        taskbar::restore(&st.marker_path);
    }
    st.taskbar_hidden.store(hide, Ordering::SeqCst);
}

/// `inner` covers `outer`, allowing a few pixels of slack: browsers in F11 mode leave a 1-2 px
/// strip on the edge of an auto-hide taskbar so it can still be revealed.
fn covers(outer: &RECT, inner: &RECT) -> bool {
    const SLACK: i32 = 4;
    inner.left <= outer.left + SLACK
        && inner.top <= outer.top + SLACK
        && inner.right >= outer.right - SLACK
        && inner.bottom >= outer.bottom - SLACK
}

/// True when the foreground window covers the whole primary monitor (games, video, presentations).
fn fullscreen_on_primary() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() || win::window_pid(fg) == std::process::id() {
            return false;
        }
        let class = win::class_name(fg);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        let mon = win::primary_monitor();
        if MonitorFromWindow(fg, MONITOR_DEFAULTTONULL) != mon {
            return false;
        }
        // A normal maximized window (title bar intact) also fills the screen when the bar
        // reserves no space, but it is not fullscreen: keep the bar on top of it.
        if IsZoomed(fg).as_bool() && (GetWindowLongW(fg, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0 {
            return false;
        }
        let mut r = RECT::default();
        if GetWindowRect(fg, &mut r).is_err() {
            return false;
        }
        covers(&win::monitor_rect(mon), &r)
    }
}

/// Start, search or a shell flyout is open or opening: Explorer shows its taskbar with them,
/// so it must not be forced hidden. The taskbar being the active window doesn't count; it
/// becomes that when Windows reveals it, e.g. as a screenshot overlay closes.
fn taskbar_may_show() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LWIN, VK_RWIN};
    let held = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe { GetAsyncKeyState(k.0 as i32) as u16 & 0x8000 != 0 };
    if held(VK_LWIN) || held(VK_RWIN) || win::start_open() {
        return true;
    }
    let fg = unsafe { GetForegroundWindow() };
    win::is_shell_ui(fg) && !matches!(win::class_name(fg).as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

static WATCH_APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Explorer just showed a taskbar window: hide it again right away instead of at the next
/// tick of the watcher, so it doesn't flash.
pub fn tray_shown() {
    let Some(app) = WATCH_APP.get() else { return };
    let st = app.state::<AppState>();
    if !st.taskbar_hidden.load(Ordering::SeqCst) {
        return;
    }
    if !taskbar_may_show() {
        taskbar::reassert_hidden();
        return;
    }
    // Start opened over a fullscreen app: bring the bar back now rather than at the next
    // watcher tick, then keep it in front of the taskbar Explorer is showing.
    if st.fullscreen.load(Ordering::SeqCst) {
        sync_fullscreen(app);
    }
    raise_over_taskbar(app);
}

/// Explorer shows its taskbar with Start and keeps re-raising it while Start opens (more so
/// coming out of a fullscreen app, where it had dropped the taskbar from topmost). Put the
/// bar back in front of it a few times over the next ~600 ms.
fn raise_over_taskbar(app: &AppHandle) {
    use std::sync::atomic::AtomicBool;
    static BUSY: AtomicBool = AtomicBool::new(false);
    let Some(h) = app.get_webview_window("dock").and_then(|d| win::tauri_hwnd(&d)) else { return };
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let hv = win::hval(h);
    let app = app.clone();
    std::thread::spawn(move || {
        for _ in 0..12 {
            // Back in the fullscreen app: the bar stays behind it.
            if app.state::<AppState>().fullscreen.load(Ordering::SeqCst) {
                break;
            }
            let h = win::hwnd(hv);
            if unsafe { IsWindowVisible(h) }.as_bool() {
                win::raise_topmost(h);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

/// Put the bar behind a fullscreen app on the primary monitor, or back on top once there is
/// none. Both the watcher and `tray_shown` (another thread) call this; the lock keeps one from
/// undoing the other half-way, which used to leave the bar stuck behind ordinary windows.
fn sync_fullscreen(app: &AppHandle) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let st = app.state::<AppState>();
    let Some(h) = app.get_webview_window("dock").and_then(|d| win::tauri_hwnd(&d)) else { return };
    let fs = fullscreen_on_primary();
    if fs != st.fullscreen.swap(fs, Ordering::SeqCst) {
        if fs {
            popups::close_card(app, None);
            popups::close_menu(app);
            popups::hide_preview(app);
            // Behind the app rather than hidden: a hidden webview drops what it drew, and
            // would come back see-through (showing the taskbar) when Start opens.
            win::send_to_back(h);
        } else {
            win::show_no_activate(h);
        }
    } else if !fs && unsafe { IsWindowVisible(h) }.as_bool() && !win::is_topmost(h) {
        // Safety net: never leave the bar behind other windows outside fullscreen.
        win::raise_topmost(h);
    }
}

/// Background loop: fullscreen hiding, keeping the taskbar hidden, and recovering from
/// Explorer restarts or display changes.
pub fn start_watcher(app: AppHandle) {
    let _ = WATCH_APP.set(app.clone());
    std::thread::spawn(move || {
        let mut last_tray = taskbar::primary_tray().map(win::hval).unwrap_or(0);
        let mut last_mon = win::monitor_rect(win::primary_monitor());
        let mut last_scale = win::monitor_scale(win::primary_monitor());
        loop {
            std::thread::sleep(Duration::from_millis(400));
            let st = app.state::<AppState>();
            let hide_tb = st.taskbar_hidden.load(Ordering::SeqCst);

            // Explorer restarted: the taskbar and our AppBar registration are new/lost.
            let tray = taskbar::primary_tray().map(win::hval).unwrap_or(0);
            if tray != 0 && tray != last_tray {
                last_tray = tray;
                std::thread::sleep(Duration::from_millis(1500));
                let a = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if a.state::<AppState>().taskbar_hidden.load(Ordering::SeqCst) {
                        taskbar::hide(&a.state::<AppState>().marker_path);
                    }
                    if let Some(h) = a.get_webview_window("dock").and_then(|d| win::tauri_hwnd(&d)) {
                        appbar::register(h);
                    }
                    place(&a);
                });
                continue;
            }

            if hide_tb {
                if taskbar_may_show() {
                    raise_over_taskbar(&app);
                } else {
                    taskbar::reassert_hidden();
                }
            }

            // Resolution or scaling changed.
            let mon = win::monitor_rect(win::primary_monitor());
            let scale = win::monitor_scale(win::primary_monitor());
            if mon != last_mon || scale != last_scale {
                last_mon = mon;
                last_scale = scale;
                let a = app.clone();
                let _ = app.run_on_main_thread(move || place(&a));
            }

            sync_fullscreen(&app);
        }
    });
}
