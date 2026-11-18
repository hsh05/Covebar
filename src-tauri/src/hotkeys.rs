//! Global shortcuts (mute mic / mute speakers / focus the bar) through RegisterHotKey on a
//! dedicated thread. Win+number is handled separately, in `winkeys`.

use crate::settings::Hotkeys;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG, PM_NOREMOVE, WM_APP,
    WM_HOTKEY,
};

const ACTIONS: [&str; 3] = ["micMute", "speakerMute", "focusBar"];
const WM_RELOAD: u32 = WM_APP + 1;

/// Per action: "ok", "off" (no shortcut), "invalid" or "taken" (another app owns it).
#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub mic_mute: String,
    pub speaker_mute: String,
    pub focus_bar: String,
}

static THREAD: OnceLock<u32> = OnceLock::new();
static WANTED: Mutex<Option<Hotkeys>> = Mutex::new(None);
static STATUS: Mutex<Option<HotkeyStatus>> = Mutex::new(None);
/// Set while the settings window records a new shortcut, so the old one doesn't fire.
static PAUSED: AtomicBool = AtomicBool::new(false);

pub fn start(app: AppHandle, keys: Hotkeys) {
    *WANTED.lock().unwrap() = Some(keys);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        // Make sure the thread has a message queue before anyone posts to it.
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let _ = tx.send(GetCurrentThreadId());
        apply(&app);
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_HOTKEY => match ACTIONS.get(msg.wParam.0) {
                    Some(&"micMute") => crate::audio::toggle_mute("input"),
                    Some(&"speakerMute") => crate::audio::toggle_mute("output"),
                    Some(&"focusBar") => crate::dock::focus_bar(&app),
                    _ => {}
                },
                WM_RELOAD => apply(&app),
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    });
    if let Ok(id) = rx.recv() {
        let _ = THREAD.set(id);
    }
}

/// Re-register after the settings changed.
pub fn update(keys: Hotkeys) {
    *WANTED.lock().unwrap() = Some(keys);
    if let Some(&id) = THREAD.get() {
        unsafe {
            let _ = PostThreadMessageW(id, WM_RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    if let Some(&id) = THREAD.get() {
        unsafe {
            let _ = PostThreadMessageW(id, WM_RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn status() -> HotkeyStatus {
    STATUS.lock().unwrap().clone().unwrap_or_default()
}

/// Runs on the hotkey thread (hotkeys belong to the thread that registered them).
fn apply(app: &AppHandle) {
    if PAUSED.load(Ordering::SeqCst) {
        for i in 0..ACTIONS.len() {
            unsafe {
                let _ = UnregisterHotKey(None, i as i32);
            }
        }
        return;
    }
    let keys = WANTED.lock().unwrap().clone().unwrap_or_default();
    let mut result = Vec::new();
    for (i, combo) in [&keys.mic_mute, &keys.speaker_mute, &keys.focus_bar].into_iter().enumerate() {
        unsafe {
            let _ = UnregisterHotKey(None, i as i32);
        }
        let st = if combo.trim().is_empty() {
            "off"
        } else if let Some((mods, vk)) = parse(combo) {
            match unsafe { RegisterHotKey(None, i as i32, mods | MOD_NOREPEAT, vk) } {
                Ok(()) => "ok",
                Err(e) => {
                    crate::dlog!("RegisterHotKey {combo}: {e}");
                    "taken"
                }
            }
        } else {
            "invalid"
        };
        result.push(st.to_string());
    }
    let st = HotkeyStatus {
        mic_mute: result[0].clone(),
        speaker_mute: result[1].clone(),
        focus_bar: result[2].clone(),
    };
    *STATUS.lock().unwrap() = Some(st.clone());
    let _ = app.emit("hotkeys-status", st);
}

/// "Win+Alt+M" -> (modifiers, virtual key). Needs at least one modifier.
pub fn parse(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let mut mods = HOT_KEY_MODIFIERS(0);
    let mut key = None;
    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "win" | "meta" | "super" => mods |= MOD_WIN,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "alt" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            k => {
                if key.is_some() {
                    return None;
                }
                key = Some(vk(k)?);
            }
        }
    }
    if mods.0 == 0 {
        return None;
    }
    Some((mods, key?))
}

fn vk(k: &str) -> Option<u32> {
    let b = k.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_alphanumeric()) {
        return Some(b[0].to_ascii_uppercase() as u32);
    }
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=24).contains(&n) {
            return Some(0x70 + n - 1);
        }
    }
    Some(match k {
        "space" => 0x20,
        "enter" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "delete" => 0x2E,
        "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "up" | "arrowup" => 0x26,
        "down" | "arrowdown" => 0x28,
        "left" | "arrowleft" => 0x25,
        "right" | "arrowright" => 0x27,
        "pause" => 0x13,
        "," | "comma" => 0xBC,
        "." | "period" => 0xBE,
        "-" | "minus" => 0xBD,
        "=" | "equal" => 0xBB,
        ";" | "semicolon" => 0xBA,
        "/" | "slash" => 0xBF,
        "`" | "backquote" => 0xC0,
        "[" | "bracketleft" => 0xDB,
        "]" | "bracketright" => 0xDD,
        "'" | "quote" => 0xDE,
        "\\" | "backslash" => 0xDC,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        assert_eq!(parse("Win+Alt+M"), Some((MOD_WIN | MOD_ALT, 'M' as u32)));
        assert_eq!(parse("ctrl+shift+f5"), Some((MOD_CONTROL | MOD_SHIFT, 0x74)));
        assert_eq!(parse("M"), None);
        assert_eq!(parse("Alt+M+K"), None);
        assert_eq!(parse("Alt+Nope"), None);
    }
}
