//! CPU/GPU temperature and load. Temperatures come from LibreHardwareMonitor (its WMI
//! namespace, or its web server on :8085); CPU and memory load fall back to Windows itself,
//! so the widget still shows something when LibreHardwareMonitor isn't running.

use crate::worker::{com_mta, Worker};
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{BSTR, PCWSTR};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Com::{
    CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::Win32::System::Variant::{VariantClear, VariantToDouble, VARIANT, VT_BSTR};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
    WBEM_INFINITE,
};

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Temps {
    /// "wmi", "web" or "" when LibreHardwareMonitor isn't reachable.
    pub source: String,
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    /// 0-100.
    pub cpu_load: Option<f64>,
    pub gpu_load: Option<f64>,
    pub mem_load: Option<f64>,
    pub cpu_name: Option<String>,
    pub gpu_name: Option<String>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Option<Temps>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(com_mta, Duration::from_secs(2), refresh));
}

pub fn current() -> Temps {
    CURRENT.lock().unwrap().clone().unwrap_or_default()
}

fn enabled() -> bool {
    APP.get().map(|a| a.state::<crate::state::AppState>().settings.lock().unwrap().has_widget("temperature")).unwrap_or(false)
}

thread_local! {
    static WMI: std::cell::RefCell<Option<IWbemServices>> = const { std::cell::RefCell::new(None) };
    static PREV_TIMES: std::cell::Cell<Option<(u64, u64)>> = const { std::cell::Cell::new(None) };
    /// Skip the web server for a while after it failed, so a missing LHM costs nothing.
    static WEB_BACKOFF: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn refresh() {
    if !enabled() {
        return;
    }
    let mut t = from_wmi().or_else(from_web).unwrap_or_default();
    // Windows' own numbers when LibreHardwareMonitor doesn't give them.
    if t.cpu_load.is_none() {
        t.cpu_load = system_cpu_load();
    } else {
        let _ = system_cpu_load(); // keep the delta baseline current
    }
    if t.mem_load.is_none() {
        t.mem_load = system_mem_load();
    }
    let mut cur = CURRENT.lock().unwrap();
    if cur.as_ref() != Some(&t) {
        *cur = Some(t.clone());
        if let Some(app) = APP.get() {
            let _ = app.emit("temps-changed", t);
        }
    }
}

// ---------------------------------------------------------------- picking sensors

struct Sensor {
    id: String,
    name: String,
    kind: String,
    value: f64,
}

#[derive(PartialEq)]
enum Hw {
    Cpu,
    Gpu,
    Ram,
    Other,
}

fn hw(id: &str) -> Hw {
    let first = id.trim_start_matches('/').split('/').next().unwrap_or("");
    if first.contains("cpu") {
        Hw::Cpu
    } else if first.starts_with("gpu") {
        Hw::Gpu
    } else if first == "ram" {
        Hw::Ram
    } else {
        Hw::Other
    }
}

/// The best sensor of `kind` on `h`, preferring names in `prefer` (in order), else the highest.
fn pick(sensors: &[Sensor], h: Hw, kind: &str, prefer: &[&str]) -> Option<f64> {
    let list: Vec<&Sensor> = sensors.iter().filter(|s| hw(&s.id) == h && s.kind.eq_ignore_ascii_case(kind)).collect();
    for p in prefer {
        if let Some(s) = list.iter().find(|s| s.name.eq_ignore_ascii_case(p)) {
            return Some(s.value);
        }
    }
    list.iter().map(|s| s.value).filter(|v| v.is_finite()).reduce(f64::max)
}

fn summarize(sensors: Vec<Sensor>, names: Vec<(String, String)>, source: &str) -> Option<Temps> {
    if sensors.is_empty() {
        return None;
    }
    let round = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0).map(|v| (v * 10.0).round() / 10.0);
    let name = |h: Hw| names.iter().find(|(id, _)| hw(id) == h).map(|(_, n)| n.clone());
    Some(Temps {
        source: source.into(),
        cpu_temp: round(pick(&sensors, Hw::Cpu, "Temperature", &["CPU Package", "Core (Tctl/Tdie)", "Tctl/Tdie", "Tdie", "Tctl", "Core Average", "Core Max"])),
        gpu_temp: round(pick(&sensors, Hw::Gpu, "Temperature", &["GPU Core", "GPU Temperature"])),
        cpu_load: round(pick(&sensors, Hw::Cpu, "Load", &["CPU Total"])),
        gpu_load: round(pick(&sensors, Hw::Gpu, "Load", &["GPU Core", "D3D 3D"])),
        mem_load: round(pick(&sensors, Hw::Ram, "Load", &["Memory"])),
        cpu_name: name(Hw::Cpu),
        gpu_name: name(Hw::Gpu),
    })
}

// ---------------------------------------------------------------- WMI

fn wmi() -> windows::core::Result<IWbemServices> {
    WMI.with(|c| {
        if let Some(s) = c.borrow().as_ref() {
            return Ok(s.clone());
        }
        let svc = unsafe {
            let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
            let svc = loc.ConnectServer(
                &BSTR::from("root\\LibreHardwareMonitor"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )?;
            CoSetProxyBlanket(&svc, RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE, None, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE)?;
            svc
        };
        *c.borrow_mut() = Some(svc.clone());
        Ok(svc)
    })
}

fn query(q: &str) -> windows::core::Result<Vec<IWbemClassObject>> {
    let svc = wmi()?;
    let mut out = Vec::new();
    unsafe {
        let en = svc.ExecQuery(&BSTR::from("WQL"), &BSTR::from(q), WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, None)?;
        loop {
            let mut row = [None; 1];
            let mut got = 0u32;
            let _ = en.Next(WBEM_INFINITE, &mut row, &mut got);
            match row[0].take() {
                Some(o) if got > 0 => out.push(o),
                _ => break,
            }
        }
    }
    Ok(out)
}

fn prop_str(o: &IWbemClassObject, name: &str) -> String {
    let n: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut v = VARIANT::default();
        if o.Get(PCWSTR(n.as_ptr()), 0, &mut v, None, None).is_err() {
            return String::new();
        }
        let inner = &v.Anonymous.Anonymous;
        let s = if inner.vt == VT_BSTR { (*inner.Anonymous.bstrVal).to_string() } else { String::new() };
        let _ = VariantClear(&mut v);
        s
    }
}

fn prop_f64(o: &IWbemClassObject, name: &str) -> Option<f64> {
    let n: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut v = VARIANT::default();
        o.Get(PCWSTR(n.as_ptr()), 0, &mut v, None, None).ok()?;
        let r = VariantToDouble(&v).ok();
        let _ = VariantClear(&mut v);
        r
    }
}

fn from_wmi() -> Option<Temps> {
    let rows = match query("SELECT Identifier, Name, SensorType, Value FROM Sensor") {
        Ok(r) => r,
        Err(_) => {
            // LHM may have started since; connect afresh next time.
            WMI.with(|c| *c.borrow_mut() = None);
            return None;
        }
    };
    let sensors = rows
        .iter()
        .filter_map(|o| {
            Some(Sensor {
                id: prop_str(o, "Identifier"),
                name: prop_str(o, "Name"),
                kind: prop_str(o, "SensorType"),
                value: prop_f64(o, "Value")?,
            })
        })
        .collect();
    let names = query("SELECT Identifier, Name FROM Hardware")
        .unwrap_or_default()
        .iter()
        .map(|o| (prop_str(o, "Identifier"), prop_str(o, "Name")))
        .collect();
    summarize(sensors, names, "wmi")
}

// ---------------------------------------------------------------- web server

fn from_web() -> Option<Temps> {
    let skip = WEB_BACKOFF.with(|b| b.get());
    if skip > 0 {
        WEB_BACKOFF.with(|b| b.set(skip - 1));
        return None;
    }
    let r = crate::net::get("http://localhost:8085/data.json", &[], 800).ok().filter(|r| r.status == 200);
    let Some(v) = r.and_then(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok()) else {
        WEB_BACKOFF.with(|b| b.set(15)); // ~30 s
        return None;
    };
    let mut sensors = Vec::new();
    let mut names = Vec::new();
    walk(&v, &mut sensors, &mut names);
    summarize(sensors, names, "web")
}

/// The web server's tree: hardware nodes carry `HardwareId`, sensors `SensorId`, `Type` and a
/// `Value` such as "45.0 °C".
fn walk(n: &serde_json::Value, sensors: &mut Vec<Sensor>, names: &mut Vec<(String, String)>) {
    let text = n["Text"].as_str().unwrap_or_default().to_string();
    if let Some(id) = n["HardwareId"].as_str() {
        names.push((id.to_string(), text.clone()));
    }
    if let (Some(id), Some(val)) = (n["SensorId"].as_str(), n["Value"].as_str()) {
        let num: String = val.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-').collect();
        if let Ok(value) = num.replace(',', ".").parse::<f64>() {
            let kind = n["Type"].as_str().map(str::to_string).unwrap_or_else(|| {
                if val.contains('°') { "Temperature".into() } else if val.contains('%') { "Load".into() } else { String::new() }
            });
            sensors.push(Sensor { id: id.to_string(), name: text, kind, value });
        }
    }
    if let Some(ch) = n["Children"].as_array() {
        for c in ch {
            walk(c, sensors, names);
        }
    }
}

// ---------------------------------------------------------------- Windows fallbacks

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

fn system_cpu_load() -> Option<f64> {
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    let (idle, total) = (ft(idle), ft(kernel) + ft(user)); // kernel time includes idle
    let prev = PREV_TIMES.with(|p| p.replace(Some((idle, total))))?;
    let (di, dt) = (idle.saturating_sub(prev.0), total.saturating_sub(prev.1));
    (dt > 0).then(|| ((1.0 - di as f64 / dt as f64) * 1000.0).round() / 10.0)
}

fn system_mem_load() -> Option<f64> {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    (m.ullTotalPhys > 0).then(|| ((1.0 - m.ullAvailPhys as f64 / m.ullTotalPhys as f64) * 1000.0).round() / 10.0)
}
