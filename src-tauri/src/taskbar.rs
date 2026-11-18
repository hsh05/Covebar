//! Hiding and restoring the Windows taskbar.
//!
//! Before hiding, the taskbar's original auto-hide state is written to a marker file.
//! The taskbar is restored from that file on a clean exit, by the panic hook, by the
//! watchdog process when Covebar dies, and at the next start if it was left stale.
//! `covebar.exe --restore-taskbar` restores it by hand.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, IsWindowVisible, ShowWindow, ShowWindowAsync, SW_HIDE, SW_SHOWNA,
};

const ABS_AUTOHIDE: u32 = 0x1;
const ABS_ALWAYSONTOP: u32 = 0x2;

#[derive(Serialize, Deserialize)]
struct Saved {
    state: u32,
}

/// `%LOCALAPPDATA%\<exe name>\taskbar-state.json`
pub fn default_marker_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let stem = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "covebar".into());
    base.join(stem).join("taskbar-state.json")
}

/// Explorer's taskbar. Skips the stand-in window of the same class that tray.rs keeps
/// (see notify.rs), which sits in front of it.
pub fn primary_tray() -> Option<HWND> {
    let me = std::process::id();
    let mut prev: Option<HWND> = None;
    loop {
        let h = unsafe { FindWindowExW(None, prev, w!("Shell_TrayWnd"), PCWSTR::null()) }.ok().filter(|h| !h.is_invalid())?;
        if crate::win::window_pid(h) != me {
            return Some(h);
        }
        prev = Some(h);
    }
}

pub fn tray_windows() -> Vec<HWND> {
    let mut v: Vec<HWND> = primary_tray().into_iter().collect();
    let mut prev: Option<HWND> = None;
    loop {
        match unsafe { FindWindowExW(None, prev, w!("Shell_SecondaryTrayWnd"), PCWSTR::null()) } {
            Ok(h) if !h.is_invalid() => {
                v.push(h);
                prev = Some(h);
            }
            _ => break,
        }
    }
    v
}

fn appbar_data() -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: primary_tray().unwrap_or_default(),
        ..Default::default()
    }
}

fn get_state() -> u32 {
    let mut abd = appbar_data();
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32 }
}

fn set_state(state: u32) {
    let mut abd = appbar_data();
    abd.lParam = LPARAM(state as isize);
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };
}

/// Save the original state (unless a saved one already exists) and hide the taskbar.
pub fn hide(marker: &PathBuf) {
    if !marker.exists() {
        let saved = Saved { state: get_state() };
        if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(marker, serde_json::to_vec(&saved).unwrap_or_default());
    }
    set_state(ABS_AUTOHIDE);
    reassert_hidden();
}

/// Hide any taskbar window that has become visible again. Doesn't wait on Explorer.
pub fn reassert_hidden() {
    for h in tray_windows() {
        unsafe {
            if IsWindowVisible(h).as_bool() {
                let _ = ShowWindowAsync(h, SW_HIDE);
            }
        }
    }
}

/// Restore from the marker file. Returns false if there was nothing to restore.
pub fn restore(marker: &PathBuf) -> bool {
    let Ok(text) = std::fs::read_to_string(marker) else {
        return false;
    };
    let state = serde_json::from_str::<Saved>(&text)
        .map(|s| s.state)
        .unwrap_or(ABS_ALWAYSONTOP);
    set_state(state);
    show_all();
    let _ = std::fs::remove_file(marker);
    true
}

/// Last-resort restore: taskbar visible and not auto-hidden.
pub fn force_restore() {
    set_state(ABS_ALWAYSONTOP);
    show_all();
}

fn show_all() {
    for h in tray_windows() {
        unsafe {
            let _ = ShowWindow(h, SW_SHOWNA);
        }
    }
}
