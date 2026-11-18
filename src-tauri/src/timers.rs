//! Countdown timer and stopwatch. State is kept in unix-ms timestamps so it survives
//! restarts, and saved to `timers.json` in the config folder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_LOOP, SND_NODEFAULT};

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Timer {
    /// Total length of the current countdown (0 = idle).
    pub duration_ms: u64,
    /// When it ends, while running.
    pub ends_at: Option<u64>,
    /// Time left, while paused.
    pub remaining_ms: Option<u64>,
    /// It finished and the alarm is sounding.
    pub ringing: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Stopwatch {
    /// When the current run started, while running.
    pub started_at: Option<u64>,
    /// Time accumulated by earlier runs.
    pub elapsed_ms: u64,
    /// Total elapsed time at each lap.
    pub laps: Vec<u64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct TimersState {
    pub timer: Timer,
    pub stopwatch: Stopwatch,
}

static STATE: Mutex<Option<TimersState>> = Mutex::new(None);
static PATH: OnceLock<PathBuf> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();

const MAX_LAPS: usize = 99;
/// Stop the alarm on its own after this long.
const RING_FOR: u64 = 2 * 60 * 1000;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn start(app: AppHandle, path: PathBuf) {
    let mut st: TimersState = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // A timer that ended long ago while we were not running: don't ring for it now.
    if st.timer.ringing || st.timer.ends_at.is_some_and(|e| e + 60_000 < now()) {
        st.timer = Timer::default();
    }
    *STATE.lock().unwrap() = Some(st);
    let _ = PATH.set(path);
    let _ = APP.set(app);

    std::thread::spawn(|| {
        let mut rang_at = 0u64;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let t = now();
            let mut fire = false;
            let mut silence = false;
            update_quiet(|s| {
                if let Some(end) = s.timer.ends_at {
                    if end <= t {
                        s.timer.ends_at = None;
                        s.timer.remaining_ms = Some(0);
                        s.timer.ringing = true;
                        fire = true;
                    }
                }
                if s.timer.ringing && rang_at > 0 && t > rang_at + RING_FOR {
                    s.timer.ringing = false;
                    silence = true;
                }
            });
            if fire {
                rang_at = t;
                ring();
                changed();
            } else if silence {
                rang_at = 0;
                stop_sound();
                if let Some(app) = APP.get() {
                    crate::popups::hide_alarm(app);
                }
                changed();
            }
        }
    });
}

pub fn get() -> TimersState {
    STATE.lock().unwrap().clone().unwrap_or_default()
}

/// Change the state without notifying anyone; returns whether anything changed.
fn update_quiet(f: impl FnOnce(&mut TimersState)) -> bool {
    let mut g = STATE.lock().unwrap();
    let st = g.get_or_insert_with(Default::default);
    let before = st.clone();
    f(st);
    *st != before
}

fn changed() {
    let st = get();
    if let Some(p) = PATH.get() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&st).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }
    if let Some(app) = APP.get() {
        let _ = app.emit("timers-changed", st);
    }
}

/// Apply a timer or stopwatch action from the UI.
pub fn action(action: &str, value: Option<u64>) -> TimersState {
    let t = now();
    let mut was_ringing = false;
    let did = update_quiet(|s| {
        let tm = &mut s.timer;
        let sw = &mut s.stopwatch;
        was_ringing = tm.ringing;
        match action {
            // Start a new countdown of `value` seconds.
            "timerStart" => {
                let ms = value.unwrap_or(0).clamp(1, 86_400) * 1000;
                *tm = Timer { duration_ms: ms, ends_at: Some(t + ms), remaining_ms: None, ringing: false };
            }
            "timerPause" => {
                if let Some(end) = tm.ends_at.take() {
                    tm.remaining_ms = Some(end.saturating_sub(t));
                }
            }
            "timerResume" => {
                if let Some(left) = tm.remaining_ms.filter(|&l| l > 0) {
                    tm.ends_at = Some(t + left);
                    tm.remaining_ms = None;
                }
            }
            // Add `value` seconds to a running or paused countdown.
            "timerAdd" => {
                let ms = value.unwrap_or(60) * 1000;
                if let Some(end) = tm.ends_at.as_mut() {
                    *end += ms;
                    tm.duration_ms += ms;
                } else if let Some(left) = tm.remaining_ms.as_mut().filter(|_| !tm.ringing) {
                    *left += ms;
                    tm.duration_ms += ms;
                }
            }
            "timerReset" | "timerDismiss" => *tm = Timer::default(),
            "swStart" => {
                if sw.started_at.is_none() {
                    sw.started_at = Some(t);
                }
            }
            "swPause" => {
                if let Some(st) = sw.started_at.take() {
                    sw.elapsed_ms += t.saturating_sub(st);
                }
            }
            "swLap" => {
                if let Some(st) = sw.started_at {
                    sw.laps.push(sw.elapsed_ms + t.saturating_sub(st));
                    if sw.laps.len() > MAX_LAPS {
                        sw.laps.remove(0);
                    }
                }
            }
            "swReset" => *sw = Stopwatch::default(),
            _ => {}
        }
    });
    if was_ringing && !get().timer.ringing {
        stop_sound();
        if let Some(app) = APP.get() {
            crate::popups::hide_alarm(app);
        }
    }
    if did {
        changed();
    }
    get()
}

fn ring() {
    let alarm = std::env::var("SystemRoot")
        .map(|r| format!("{r}\\Media\\Alarm01.wav"))
        .unwrap_or_else(|_| "C:\\Windows\\Media\\Alarm01.wav".into());
    let p = crate::win::wide(&alarm);
    unsafe {
        let _ = PlaySoundW(PCWSTR(p.as_ptr()), None, SND_FILENAME | SND_ASYNC | SND_LOOP | SND_NODEFAULT);
    }
    let Some(app) = APP.get() else { return };
    // The dock answers with the timer tile's position (show_alarm); if it doesn't (the tile
    // is hidden, or the dock is busy), show the popup over the middle of the dock anyway.
    let duration = get().timer.duration_ms;
    let before = crate::popups::alarm_session();
    let _ = app.emit("timer-done", duration);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        if crate::popups::alarm_session() == before && get().timer.ringing {
            crate::popups::show_alarm(&app, None, duration);
        }
    });
}

fn stop_sound() {
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, Default::default());
    }
}

/// Make sure the alarm doesn't outlive the app.
pub fn shutdown() {
    stop_sound();
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test, since the timer state is global.
    #[test]
    fn timer_and_stopwatch_actions() {
        let t0 = now();
        let s = action("timerStart", Some(60));
        assert_eq!(s.timer.duration_ms, 60_000);
        let end = s.timer.ends_at.unwrap();
        assert!(end >= t0 + 60_000 && end <= now() + 60_000);

        let s = action("timerPause", None);
        assert!(s.timer.ends_at.is_none());
        let left = s.timer.remaining_ms.unwrap();
        assert!(left <= 60_000 && left > 59_000);

        let s = action("timerAdd", Some(30));
        assert_eq!(s.timer.remaining_ms, Some(left + 30_000));
        assert_eq!(s.timer.duration_ms, 90_000);

        let s = action("timerResume", None);
        assert!(s.timer.ends_at.is_some() && s.timer.remaining_ms.is_none());

        // Zero is clamped to one second; an unknown action changes nothing.
        let s = action("timerStart", Some(0));
        assert_eq!(s.timer.duration_ms, 1000);
        assert_eq!(action("bogus", None), s);
        assert_eq!(action("timerReset", None).timer, Timer::default());

        let s = action("swLap", None);
        assert!(s.stopwatch.laps.is_empty(), "no laps while stopped");
        action("swStart", None);
        let s = action("swLap", None);
        assert_eq!(s.stopwatch.laps.len(), 1);
        let s = action("swPause", None);
        assert!(s.stopwatch.started_at.is_none());
        assert!(s.stopwatch.elapsed_ms >= s.stopwatch.laps[0]);
        assert_eq!(action("swReset", None).stopwatch, Stopwatch::default());
    }
}
