//! Exchange rates, USD-based: open.er-api.com, with the fawazahmed0 currency API on jsDelivr as
//! a fallback. The last good set is cached on disk so the widget works offline.

use crate::worker::Worker;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rates {
    /// Units of each currency per 1 USD, keyed by upper-case ISO code.
    pub rates: BTreeMap<String, f64>,
    /// When the provider last updated the rates (unix ms).
    pub updated: f64,
    /// "er-api" or "jsdelivr".
    pub source: String,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyState {
    pub rates: Option<Rates>,
    /// Set while the last fetch failed; the cached rates (if any) are still shown.
    pub error: Option<String>,
    pub loading: bool,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static STATE: Mutex<CurrencyState> = Mutex::new(CurrencyState { rates: None, error: None, loading: false });
static APP: OnceLock<AppHandle> = OnceLock::new();
static CACHE: OnceLock<PathBuf> = OnceLock::new();
/// When the next automatic fetch is due.
static NEXT: Mutex<Option<Instant>> = Mutex::new(None);

const EVERY: Duration = Duration::from_secs(3 * 3600);
const RETRY: Duration = Duration::from_secs(10 * 60);

pub fn start(app: AppHandle, cache: PathBuf) {
    if let Some(r) = std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice::<Rates>(&b).ok()) {
        STATE.lock().unwrap().rates = Some(r);
    }
    let _ = CACHE.set(cache);
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(|| {}, Duration::from_secs(30), tick));
}

pub fn current() -> CurrencyState {
    STATE.lock().unwrap().clone()
}

fn publish(f: impl FnOnce(&mut CurrencyState)) {
    let mut st = STATE.lock().unwrap();
    let before = st.clone();
    f(&mut st);
    if *st != before {
        if let Some(app) = APP.get() {
            let _ = app.emit("currency-changed", st.clone());
        }
    }
}

fn enabled() -> bool {
    APP.get().map(|a| a.state::<crate::state::AppState>().settings.lock().unwrap().has_widget("currency")).unwrap_or(false)
}

fn tick() {
    if !enabled() {
        return;
    }
    let due = NEXT.lock().unwrap().is_none_or(|t| Instant::now() >= t);
    // Fresh cached rates count: no need to fetch at every start.
    let fresh = STATE.lock().unwrap().rates.as_ref().is_some_and(|r| now_ms() - r.updated < 6.0 * 3600.0 * 1000.0);
    if due && NEXT.lock().unwrap().is_none() && fresh {
        *NEXT.lock().unwrap() = Some(Instant::now() + EVERY);
        return;
    }
    if due {
        fetch();
    }
}

/// Fetch now (from the card's refresh button).
pub fn refresh() {
    if let Some(w) = WORKER.get() {
        w.run(fetch);
    }
}

fn fetch() {
    publish(|s| s.loading = true);
    let r = from_er_api().or_else(|e| {
        crate::dlog!("currency: er-api: {e}; trying jsDelivr");
        from_jsdelivr()
    });
    match r {
        Ok(rates) => {
            if let Some(p) = CACHE.get() {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(p, serde_json::to_vec(&rates).unwrap_or_default());
            }
            *NEXT.lock().unwrap() = Some(Instant::now() + EVERY);
            publish(|s| {
                s.rates = Some(rates);
                s.error = None;
                s.loading = false;
            });
        }
        Err(e) => {
            crate::dlog!("currency: {e}");
            *NEXT.lock().unwrap() = Some(Instant::now() + RETRY);
            publish(|s| {
                s.error = Some("Couldn't update the rates".into());
                s.loading = false;
            });
        }
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

fn from_er_api() -> Result<Rates, String> {
    let (status, v) = crate::net::get_json("https://open.er-api.com/v6/latest/USD", &[])?;
    if status != 200 || v["result"] != "success" {
        return Err(format!("HTTP {status}"));
    }
    let rates: BTreeMap<String, f64> = v["rates"]
        .as_object()
        .ok_or("no rates")?
        .iter()
        .filter_map(|(k, v)| Some((k.to_ascii_uppercase(), v.as_f64()?)))
        .collect();
    let updated = v["time_last_update_unix"].as_f64().map(|s| s * 1000.0).unwrap_or_else(now_ms);
    check(Rates { rates, updated, source: "er-api".into() })
}

fn from_jsdelivr() -> Result<Rates, String> {
    let url = "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json";
    let (status, v) = crate::net::get_json(url, &[])?;
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    // It also lists crypto and metals; keep the three-letter currency codes.
    let rates: BTreeMap<String, f64> = v["usd"]
        .as_object()
        .ok_or("no rates")?
        .iter()
        .filter(|(k, _)| k.len() == 3 && k.chars().all(|c| c.is_ascii_alphabetic()))
        .filter_map(|(k, v)| Some((k.to_ascii_uppercase(), v.as_f64()?)))
        .collect();
    let updated = v["date"]
        .as_str()
        .and_then(date_ms)
        .unwrap_or_else(now_ms);
    check(Rates { rates, updated, source: "jsdelivr".into() })
}

fn check(mut r: Rates) -> Result<Rates, String> {
    r.rates.retain(|_, v| v.is_finite() && *v > 0.0);
    r.rates.insert("USD".into(), 1.0);
    if r.rates.len() < 20 {
        return Err("too few rates".into());
    }
    Ok(r)
}

/// "YYYY-MM-DD" to unix ms (midnight UTC).
fn date_ms(s: &str) -> Option<f64> {
    let mut it = s.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (it.next()??, it.next()??, it.next()??);
    // Days from civil, Howard Hinnant's algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(((era * 146_097 + doe - 719_468) * 86_400_000) as f64)
}
