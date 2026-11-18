//! Claude plan usage, read with the Claude Code sign-in already on this PC
//! (`%USERPROFILE%\.claude\.credentials.json`) from the usage endpoint community tools use.
//!
//! The endpoint is undocumented, so every failure becomes a friendly status instead of an
//! error. The credentials file is only ever read, the token is read fresh for each request
//! and dropped right after, and it is never logged or sent anywhere but api.anthropic.com.

use crate::worker::Worker;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    /// "session", "week", "opus", "sonnet"
    pub key: String,
    pub label: String,
    /// 0-100
    pub percent: f64,
    /// ISO 8601, or None if the window hasn't started.
    pub resets_at: Option<String>,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// "ok", "loading", "signedOut", "expired", "offline", "error"
    pub status: String,
    pub plan: Option<String>,
    pub windows: Vec<Window>,
    /// When `windows` was fetched (unix ms); kept from the last success through errors.
    pub fetched_at: Option<f64>,
    pub message: Option<String>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static STATE: Mutex<Option<Usage>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();
static NEXT: Mutex<Option<Instant>> = Mutex::new(None);
/// The Claude Code command-line tool couldn't be started, so expired sign-ins can't be renewed.
static NO_CLI: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What to tell the user when the saved sign-in has expired.
fn expired_message() -> &'static str {
    if NO_CLI.load(std::sync::atomic::Ordering::Relaxed) {
        "Your Claude Code sign-in expired. Install the Claude Code command-line tool so Covebar can renew it, or run claude once in a terminal."
    } else {
        "Your Claude Code sign-in expired. Run claude once in a terminal to renew it."
    }
}

const EVERY: Duration = Duration::from_secs(5 * 60);
const BACKOFF: Duration = Duration::from_secs(15 * 60);
const URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(|| {}, Duration::from_secs(20), tick));
}

pub fn current() -> Usage {
    STATE.lock().unwrap().clone().unwrap_or_else(|| Usage { status: "loading".into(), ..Default::default() })
}

pub fn refresh() {
    if let Some(w) = WORKER.get() {
        w.run(fetch);
    }
}

fn enabled() -> bool {
    APP.get().map(|a| a.state::<crate::state::AppState>().settings.lock().unwrap().has_widget("claude")).unwrap_or(false)
}

fn tick() {
    if enabled() && NEXT.lock().unwrap().is_none_or(|t| Instant::now() >= t) {
        fetch();
    }
}

fn publish(u: Usage) {
    let mut st = STATE.lock().unwrap();
    if st.as_ref() != Some(&u) {
        *st = Some(u.clone());
        if let Some(app) = APP.get() {
            let _ = app.emit("claude-changed", u);
        }
    }
}

struct Creds {
    token: String,
    expires_at: Option<f64>,
    plan: Option<String>,
}

fn read_creds() -> Option<Creds> {
    let home = std::env::var_os("USERPROFILE")?;
    let path = std::path::Path::new(&home).join(".claude").join(".credentials.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let o = &v["claudeAiOauth"];
    Some(Creds {
        token: o["accessToken"].as_str().filter(|t| !t.is_empty())?.to_string(),
        expires_at: o["expiresAt"].as_f64(),
        plan: o["subscriptionType"].as_str().map(plan_name),
    })
}

fn plan_name(s: &str) -> String {
    match s.to_ascii_lowercase().as_str() {
        "pro" => "Pro".into(),
        "max" => "Max".into(),
        "team" => "Team".into(),
        "enterprise" => "Enterprise".into(),
        "free" => "Free".into(),
        other => {
            let mut c = other.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// Keep the last good numbers, with a new status on top.
fn with_status(status: &str, message: &str, plan: Option<String>) -> Usage {
    let last = STATE.lock().unwrap().clone().unwrap_or_default();
    Usage { status: status.into(), message: Some(message.into()), plan: plan.or(last.plan), ..last }
}

/// Claude Code renews its sign-in whenever it runs, but the desktop app keeps its own copy and
/// leaves this file to expire. Run `claude auth status` (no prompt, no usage) quietly so the
/// CLI renews the file itself; Covebar still only ever reads it. At most every 10 minutes.
fn renew_signin() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap();
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(10 * 60)) {
            return false;
        }
        *last = Some(Instant::now());
    }
    let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
    let native = home.join(".local").join("bin").join("claude.exe");
    let mut cmd = if native.is_file() {
        Command::new(native)
    } else {
        // npm installs a .cmd shim, which needs cmd to run.
        let mut c = Command::new("cmd");
        c.args(["/d", "/c", "claude"]);
        c
    };
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let child = cmd
        .args(["auth", "status"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let Ok(mut child) = child else {
        crate::dlog!("claude usage: Claude Code CLI not found");
        NO_CLI.store(true, std::sync::atomic::Ordering::Relaxed);
        return false;
    };
    NO_CLI.store(false, std::sync::atomic::Ordering::Relaxed);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            // `cmd /c claude` runs even when there's no claude; it then exits with 9009.
            Ok(Some(status)) if status.code() == Some(9009) => {
                NO_CLI.store(true, std::sync::atomic::Ordering::Relaxed);
                return false;
            }
            Ok(Some(status)) => {
                crate::dlog!("claude usage: asked Claude Code to renew its sign-in ({status})");
                return status.success();
            }
            Ok(None) if started.elapsed() < Duration::from_secs(30) => std::thread::sleep(Duration::from_millis(200)),
            _ => {
                let _ = child.kill();
                return false;
            }
        }
    }
}

fn fetch() {
    fetch_once(true);
}

fn fetch_once(may_renew: bool) {
    let Some(creds) = read_creds() else {
        *NEXT.lock().unwrap() = Some(Instant::now() + EVERY);
        return publish(Usage {
            status: "signedOut".into(),
            message: Some("Sign in to Claude Code on this PC to see your usage".into()),
            ..Default::default()
        });
    };
    let plan = creds.plan.clone();
    // Claude Code refreshes its own sign-in; we never do (that would mean writing the file).
    if creds.expires_at.is_some_and(|e| e < now_ms()) {
        drop(creds);
        if may_renew && renew_signin() {
            return fetch_once(false);
        }
        *NEXT.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        return publish(with_status("expired", expired_message(), plan));
    }

    let auth = format!("Bearer {}", creds.token);
    drop(creds);
    let r = crate::net::get_json(URL, &[("Authorization", &auth), ("anthropic-beta", "oauth-2025-04-20"), ("Accept", "application/json")]);
    drop(auth);

    let (next, usage) = match r {
        Ok((200, v)) => match parse(&v) {
            Some(windows) => (
                EVERY,
                Usage { status: "ok".into(), plan, windows, fetched_at: Some(now_ms()), message: None },
            ),
            None => (EVERY, with_status("error", "The usage service answered in an unexpected way", plan)),
        },
        Ok((401 | 403, _)) if may_renew && renew_signin() => return fetch_once(false),
        Ok((401 | 403, _)) => (
            Duration::from_secs(60),
            with_status("expired", expired_message(), plan),
        ),
        Ok((429, _)) => (BACKOFF, with_status("error", "Usage service is busy; trying again later", plan)),
        Ok((status, _)) => {
            crate::dlog!("claude usage: HTTP {status}");
            (BACKOFF, with_status("error", "Usage service unavailable right now", plan))
        }
        Err(e) => {
            crate::dlog!("claude usage: {e}");
            (EVERY, with_status("offline", "Can't reach Claude right now", plan))
        }
    };
    *NEXT.lock().unwrap() = Some(Instant::now() + next);
    publish(usage);
}

fn parse(v: &serde_json::Value) -> Option<Vec<Window>> {
    let fields = [
        ("five_hour", "session", "Current session"),
        ("seven_day", "week", "This week, all models"),
        ("seven_day_opus", "opus", "This week, Opus"),
        ("seven_day_sonnet", "sonnet", "This week, Sonnet"),
    ];
    let out: Vec<Window> = fields
        .iter()
        .filter_map(|(field, key, label)| {
            let o = v.get(*field)?;
            let percent = o["utilization"].as_f64()?;
            Some(Window {
                key: (*key).into(),
                label: (*label).into(),
                percent: percent.clamp(0.0, 100.0),
                resets_at: o["resets_at"].as_str().map(str::to_string),
            })
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_windows_and_skips_missing_ones() {
        let v = serde_json::json!({
            "five_hour": { "utilization": 42.5, "resets_at": "2026-11-18T20:00:00Z" },
            "seven_day": { "utilization": 130.0, "resets_at": null },
            "seven_day_opus": null,
            "extra": { "utilization": 1 }
        });
        let w = parse(&v).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].key, "session");
        assert_eq!(w[0].percent, 42.5);
        assert_eq!(w[0].resets_at.as_deref(), Some("2026-11-18T20:00:00Z"));
        assert_eq!(w[1].percent, 100.0, "clamped");
        assert_eq!(w[1].resets_at, None);
    }

    #[test]
    fn unexpected_answers_are_not_usage() {
        assert!(parse(&serde_json::json!({})).is_none());
        assert!(parse(&serde_json::json!({ "five_hour": { "utilization": "lots" } })).is_none());
    }

    #[test]
    fn plan_names() {
        assert_eq!(plan_name("pro"), "Pro");
        assert_eq!(plan_name("MAX"), "Max");
        assert_eq!(plan_name("student"), "Student");
        assert_eq!(plan_name(""), "");
    }
}
