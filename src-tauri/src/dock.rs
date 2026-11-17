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
    GetForegroundWindow, GetWindowLongW, GetWindowRect, IsZoomed, GWL_STYLE, WS_CAPTION,
};

const WINDOWS: [&str; 5] = ["dock", "card", "preview", "menu", "settings"];

/// Reserve space with the AppBar and move the dock into it.
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
            // The pill draws its own rounded surface; a system backdrop would fill the whole strip.
            ("dock", _) if s.shape == Shape::Pill => {
                let _ = w.set_effects(None::<tauri::utils::config::WindowEffectsConfig>);
                continue;
            }
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

/// Shell surfaces (Start, search, flyouts) during which the taskbar must not be forced hidden.
fn shell_ui_in_front() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    fg.is_invalid() || win::is_shell_ui(fg)
}

/// Background loop: fullscreen hiding, keeping the taskbar hidden, and recovering from
/// Explorer restarts or display changes.
pub fn start_watcher(app: AppHandle) {
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

            if hide_tb && !shell_ui_in_front() {
                taskbar::reassert_hidden();
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

            let fs = fullscreen_on_primary();
            if fs != st.fullscreen.swap(fs, Ordering::SeqCst) {
                let Some(dock) = app.get_webview_window("dock") else { continue };
                if fs {
                    popups::close_card(&app, None);
                    popups::close_menu(&app);
                    popups::hide_preview(&app);
                    if let Some(h) = win::tauri_hwnd(&dock) {
                        win::hide(h);
                    }
                } else if let Some(h) = win::tauri_hwnd(&dock) {
                    win::show_no_activate(h);
                }
            }
        }
    });
}
