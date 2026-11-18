//! Display brightness: WMI for built-in panels, DDC/CI (dxva2) for external monitors.

use crate::worker::{com_mta, Worker};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::core::{w, BOOL, BSTR, PCWSTR};
use windows::Win32::System::Variant::{VariantClear, VariantToUInt32, VARIANT, VT_BSTR, VT_I4, VT_UI1};
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, GetMonitorBrightness,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, QueryDisplayConfig,
    SetMonitorBrightness, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, PHYSICAL_MONITOR, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
use windows::Win32::System::Com::{
    CoCreateInstance, CoSetProxyBlanket, CLSCTX_INPROC_SERVER, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Wmi::{
    IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
    WBEM_INFINITE,
};

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// GDI device name, e.g. `\\.\DISPLAY1`.
    pub id: String,
    pub name: String,
    /// 0..=100
    pub brightness: u32,
    /// "wmi" or "ddc"
    pub method: String,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Vec<Display>> = Mutex::new(Vec::new());
static PENDING: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| {
        let mut n = 0u32;
        Worker::spawn(com_mta, Duration::from_secs(5), move || {
            // DDC/CI reads are slow and some monitors dislike being polled, so read them less often.
            refresh(n.is_multiple_of(3));
            n = n.wrapping_add(1);
        })
    });
}

pub fn current() -> Vec<Display> {
    CURRENT.lock().unwrap().clone()
}

fn publish(list: Vec<Display>) {
    let mut cur = CURRENT.lock().unwrap();
    if *cur != list {
        *cur = list.clone();
        if let Some(app) = APP.get() {
            let _ = app.emit("brightness-changed", list);
        }
    }
}

/// Set brightness. Rapid calls (mouse wheel, slider drags) coalesce to the latest value.
pub fn set(id: String, value: u32) {
    let value = value.min(100);
    // Show the new value right away.
    let mut list = current();
    if let Some(d) = list.iter_mut().find(|d| d.id == id) {
        d.brightness = value;
    }
    publish(list);

    PENDING.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, value);
    if let Some(w) = WORKER.get() {
        w.run(|| {
            let jobs = PENDING.lock().unwrap().take().unwrap_or_default();
            if jobs.is_empty() {
                return;
            }
            let list = current();
            for (id, v) in jobs {
                let Some(d) = list.iter().find(|d| d.id == id) else { continue };
                let r = if d.method == "wmi" { wmi_set(v) } else { ddc_set(&id, v) };
                if let Err(e) = r {
                    crate::dlog!("brightness {id}: {e}");
                }
            }
        });
    }
}

// ---------------------------------------------------------------- enumeration

struct Mon {
    handle: HMONITOR,
    device: String,
}

fn monitors() -> Vec<Mon> {
    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
        let out = &mut *(lp.0 as *mut Vec<Mon>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            let n = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
            out.push(Mon { handle: h, device: String::from_utf16_lossy(&mi.szDevice[..n]) });
        }
        true.into()
    }
    let mut out: Vec<Mon> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

struct Target {
    name: String,
    internal: bool,
}

/// GDI device name -> friendly monitor name and whether it is a built-in panel.
fn targets() -> HashMap<String, Target> {
    let mut map = HashMap::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm).is_err() {
            return map;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None)
            .is_err()
        {
            return map;
        }
        for p in &paths[..np as usize] {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            src.header.size = std::mem::size_of_val(&src) as u32;
            src.header.adapterId = p.sourceInfo.adapterId;
            src.header.id = p.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let mut tgt = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            tgt.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            tgt.header.size = std::mem::size_of_val(&tgt) as u32;
            tgt.header.adapterId = p.targetInfo.adapterId;
            tgt.header.id = p.targetInfo.id;
            let name = if DisplayConfigGetDeviceInfo(&mut tgt.header) == 0 {
                utf16(&tgt.monitorFriendlyDeviceName)
            } else {
                String::new()
            };
            let tech = p.targetInfo.outputTechnology;
            let internal = tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
                || tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
                || tech == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED;
            map.insert(utf16(&src.viewGdiDeviceName), Target { name, internal });
        }
    }
    map
}

fn utf16(s: &[u16]) -> String {
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..n])
}

fn refresh(read_ddc: bool) {
    let names = targets();
    let old = current();
    let wmi = wmi_get();
    let mut list = Vec::new();
    for (i, m) in monitors().into_iter().enumerate() {
        let t = names.get(&m.device);
        let internal = t.is_some_and(|t| t.internal);
        let name = t
            .map(|t| t.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| if internal { "Built-in display".into() } else { format!("Display {}", i + 1) });
        let (brightness, method) = if internal && wmi.is_some() {
            (wmi, "wmi")
        } else if read_ddc {
            (ddc_get(m.handle), "ddc")
        } else {
            // Between DDC reads keep what we knew, but only for monitors that answered before.
            match old.iter().find(|d| d.id == m.device && d.method == "ddc") {
                Some(d) => (Some(d.brightness), "ddc"),
                None => (None, "ddc"),
            }
        };
        if let Some(b) = brightness {
            list.push(Display { id: m.device, name, brightness: b, method: method.into() });
        }
    }
    publish(list);
}

// ---------------------------------------------------------------- DDC/CI

fn with_physical<R>(h: HMONITOR, f: impl FnOnce(&[PHYSICAL_MONITOR]) -> Option<R>) -> Option<R> {
    unsafe {
        let mut n = 0u32;
        GetNumberOfPhysicalMonitorsFromHMONITOR(h, &mut n).ok()?;
        if n == 0 {
            return None;
        }
        let mut mons = vec![PHYSICAL_MONITOR::default(); n as usize];
        GetPhysicalMonitorsFromHMONITOR(h, &mut mons).ok()?;
        let r = f(&mons);
        let _ = DestroyPhysicalMonitors(&mons);
        r
    }
}

fn ddc_get(h: HMONITOR) -> Option<u32> {
    with_physical(h, |mons| unsafe {
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        if GetMonitorBrightness(mons[0].hPhysicalMonitor, &mut min, &mut cur, &mut max) == 0 || max <= min {
            return None;
        }
        Some(((cur.saturating_sub(min)) * 100 + (max - min) / 2) / (max - min))
    })
}

fn ddc_set(device: &str, v: u32) -> Result<(), String> {
    let m = monitors().into_iter().find(|m| m.device == device).ok_or("monitor gone")?;
    with_physical(m.handle, |mons| unsafe {
        for pm in mons {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 100u32);
            if GetMonitorBrightness(pm.hPhysicalMonitor, &mut min, &mut cur, &mut max) == 0 || max <= min {
                (min, max) = (0, 100);
            }
            SetMonitorBrightness(pm.hPhysicalMonitor, min + (max - min) * v / 100);
        }
        Some(())
    })
    .ok_or_else(|| "DDC/CI not supported".into())
}

// ---------------------------------------------------------------- WMI

thread_local! {
    static WMI: std::cell::RefCell<Option<IWbemServices>> = const { std::cell::RefCell::new(None) };
}

fn wmi() -> windows::core::Result<IWbemServices> {
    WMI.with(|c| {
        if let Some(s) = c.borrow().as_ref() {
            return Ok(s.clone());
        }
        let svc = unsafe {
            let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
            let svc = loc.ConnectServer(&BSTR::from("root\\WMI"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)?;
            CoSetProxyBlanket(
                &svc,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )?;
            svc
        };
        *c.borrow_mut() = Some(svc.clone());
        Ok(svc)
    })
}

fn wmi_query(class: &str) -> windows::core::Result<Vec<IWbemClassObject>> {
    let svc = wmi()?;
    let mut out = Vec::new();
    unsafe {
        let en = svc.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(format!("SELECT * FROM {class}")),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?;
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

/// Read a numeric property.
fn prop_u32(o: &IWbemClassObject, name: PCWSTR) -> windows::core::Result<u32> {
    unsafe {
        let mut v = VARIANT::default();
        o.Get(name, 0, &mut v, None, None)?;
        let r = VariantToUInt32(&v);
        let _ = VariantClear(&mut v);
        r
    }
}

/// Read a string property.
fn prop_bstr(o: &IWbemClassObject, name: PCWSTR) -> windows::core::Result<BSTR> {
    unsafe {
        let mut v = VARIANT::default();
        o.Get(name, 0, &mut v, None, None)?;
        let inner = &v.Anonymous.Anonymous;
        let r = if inner.vt == VT_BSTR { Ok((*inner.Anonymous.bstrVal).clone()) } else { Err(windows::core::Error::empty()) };
        let _ = VariantClear(&mut v);
        r
    }
}

fn variant_i4(n: i32) -> VARIANT {
    let mut v = VARIANT::default();
    unsafe {
        (*v.Anonymous.Anonymous).vt = VT_I4;
        (*v.Anonymous.Anonymous).Anonymous.lVal = n;
    }
    v
}

fn variant_u8(n: u8) -> VARIANT {
    let mut v = VARIANT::default();
    unsafe {
        (*v.Anonymous.Anonymous).vt = VT_UI1;
        (*v.Anonymous.Anonymous).Anonymous.bVal = n;
    }
    v
}

/// Brightness of the built-in panel, if the machine has one WMI can drive.
fn wmi_get() -> Option<u32> {
    let rows = wmi_query("WmiMonitorBrightness").ok()?;
    let o = rows.first()?;
    prop_u32(o, w!("CurrentBrightness")).ok()
}

fn wmi_set(v: u32) -> Result<(), String> {
    let e = |e: windows::core::Error| e.message();
    let svc = wmi().map_err(e)?;
    let rows = wmi_query("WmiMonitorBrightnessMethods").map_err(e)?;
    let inst = rows.first().ok_or("no WMI brightness control")?;
    let path = prop_bstr(inst, w!("__PATH")).map_err(e)?;
    unsafe {
        let mut class = None;
        svc.GetObject(&BSTR::from("WmiMonitorBrightnessMethods"), Default::default(), None, Some(&mut class), None)
            .map_err(e)?;
        let class = class.ok_or("no class")?;
        let mut def = None;
        class.GetMethod(w!("WmiSetBrightness"), 0, &mut def, std::ptr::null_mut()).map_err(e)?;
        let args = def.ok_or("no method")?.SpawnInstance(0).map_err(e)?;
        args.Put(w!("Timeout"), 0, &variant_i4(0), 0).map_err(e)?;
        args.Put(w!("Brightness"), 0, &variant_u8(v as u8), 0).map_err(e)?;
        svc.ExecMethod(&path, &BSTR::from("WmiSetBrightness"), Default::default(), None, &args, None, None)
            .map_err(e)?;
    }
    Ok(())
}
