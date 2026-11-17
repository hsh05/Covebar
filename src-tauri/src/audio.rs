//! Volume, mute and default-device switching through Core Audio.
#![allow(non_snake_case)]

use crate::worker::{com_mta, Worker};
use serde::Serialize;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::core::{interface, Interface, GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eMultimedia, eRender, EDataFlow, IAudioSessionControl2, IAudioSessionManager2,
    IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    /// 0..=1
    pub volume: f32,
    pub muted: bool,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub output: Option<Endpoint>,
    pub input: Option<Endpoint>,
    pub outputs: Vec<DeviceInfo>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Option<AudioState>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(com_mta, Duration::from_millis(500), refresh));
}

pub fn current() -> AudioState {
    CURRENT.lock().unwrap().clone().unwrap_or_default()
}

/// Re-read the state and emit `audio-changed` if it changed. Runs on the worker.
fn refresh() {
    let st = read().unwrap_or_default();
    let mut cur = CURRENT.lock().unwrap();
    // Mic switched on or off (by us, a hotkey or another app): flash the big indicator.
    if let (Some(prev), Some(now)) = (cur.as_ref().and_then(|c| c.input.as_ref()), st.input.as_ref()) {
        if prev.id == now.id && prev.muted != now.muted {
            if let Some(app) = APP.get() {
                crate::popups::show_osd(app, "mic", !now.muted);
            }
        }
    }
    if cur.as_ref() != Some(&st) {
        *cur = Some(st.clone());
        if let Some(app) = APP.get() {
            let _ = app.emit("audio-changed", st);
        }
    }
}

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

unsafe fn device_id(d: &IMMDevice) -> String {
    match d.GetId() {
        Ok(p) => {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        }
        Err(_) => String::new(),
    }
}

unsafe fn device_name(d: &IMMDevice) -> String {
    d.OpenPropertyStore(STGM_READ)
        .and_then(|ps| ps.GetValue(&PKEY_Device_FriendlyName))
        .map(|v| v.to_string())
        .unwrap_or_default()
}

unsafe fn endpoint(e: &IMMDeviceEnumerator, flow: EDataFlow) -> Option<(IMMDevice, IAudioEndpointVolume)> {
    let d = e.GetDefaultAudioEndpoint(flow, eMultimedia).ok()?;
    let v = d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()?;
    Some((d, v))
}

fn read() -> windows::core::Result<AudioState> {
    unsafe {
        let e = enumerator()?;
        let info = |flow| {
            endpoint(&e, flow).map(|(d, v)| Endpoint {
                id: device_id(&d),
                name: device_name(&d),
                volume: v.GetMasterVolumeLevelScalar().unwrap_or(0.0),
                muted: v.GetMute().map(|b| b.as_bool()).unwrap_or(false),
            })
        };
        let mut outputs = Vec::new();
        if let Ok(list) = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
            for i in 0..list.GetCount().unwrap_or(0) {
                if let Ok(d) = list.Item(i) {
                    outputs.push(DeviceInfo { id: device_id(&d), name: device_name(&d) });
                }
            }
        }
        Ok(AudioState { output: info(eRender), input: info(eCapture), outputs })
    }
}

fn flow_of(which: &str) -> EDataFlow {
    if which == "input" { eCapture } else { eRender }
}

fn with_volume(which: String, f: impl FnOnce(&IAudioEndpointVolume) + Send + 'static) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || {
        unsafe {
            if let Some((_, v)) = enumerator().ok().and_then(|e| endpoint(&e, flow_of(&which))) {
                f(&v);
            }
        }
        refresh();
    });
}

pub fn set_volume(which: String, volume: f32) {
    let v = volume.clamp(0.0, 1.0);
    with_volume(which, move |ep| unsafe {
        let _ = ep.SetMasterVolumeLevelScalar(v, std::ptr::null());
        // Turning the volume up unmutes, like the Windows flyout.
        if v > 0.0 {
            let _ = ep.SetMute(false, std::ptr::null());
        }
    });
}

pub fn set_mute(which: String, muted: bool) {
    with_volume(which, move |ep| unsafe {
        let _ = ep.SetMute(muted, std::ptr::null());
    });
}

/// Toggle mute; returns nothing, the new state arrives as `audio-changed`.
pub fn toggle_mute(which: &str) {
    with_volume(which.to_string(), |ep| unsafe {
        let m = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
        let _ = ep.SetMute(!m, std::ptr::null());
    });
}

// ---------------------------------------------------------------- default device

/// Undocumented but long-stable interface the Sound control panel uses to change the
/// default device (used by SoundSwitch, AudioDeviceCmdlets, EarTrumpet and others).
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, id: PCWSTR, fmt: *mut *mut c_void) -> HRESULT;
    fn GetDeviceFormat(&self, id: PCWSTR, default: i32, fmt: *mut *mut c_void) -> HRESULT;
    fn ResetDeviceFormat(&self, id: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, id: PCWSTR, a: *mut c_void, b: *mut c_void) -> HRESULT;
    fn GetProcessingPeriod(&self, id: PCWSTR, default: i32, a: *mut i64, b: *mut i64) -> HRESULT;
    fn SetProcessingPeriod(&self, id: PCWSTR, a: *mut i64) -> HRESULT;
    fn GetShareMode(&self, id: PCWSTR, mode: *mut c_void) -> HRESULT;
    fn SetShareMode(&self, id: PCWSTR, mode: *mut c_void) -> HRESULT;
    fn GetPropertyValue(&self, id: PCWSTR, key: *const c_void, v: *mut c_void) -> HRESULT;
    fn SetPropertyValue(&self, id: PCWSTR, key: *const c_void, v: *const c_void) -> HRESULT;
    fn SetDefaultEndpoint(&self, id: PCWSTR, role: i32) -> HRESULT;
    fn SetEndpointVisibility(&self, id: PCWSTR, visible: i32) -> HRESULT;
}

const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

pub fn set_default_output(id: String) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || {
        unsafe {
            match CoCreateInstance::<_, IPolicyConfig>(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL) {
                Ok(pc) => {
                    let wid = crate::win::wide(&id);
                    for role in [eConsole, eMultimedia, eCommunications] {
                        let hr = pc.SetDefaultEndpoint(PCWSTR(wid.as_ptr()), role.0);
                        if hr.is_err() {
                            crate::dlog!("SetDefaultEndpoint: {hr:?}");
                        }
                    }
                }
                Err(e) => crate::dlog!("IPolicyConfig: {e}"),
            }
        }
        refresh();
    });
}

// ---------------------------------------------------------------- per-app volume

/// Volume of one app's audio sessions (the mixer's per-app slider).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppVolume {
    pub volume: f32,
    pub muted: bool,
}

/// Names an app's processes might go by, from its media AppUserModelID:
/// "Spotify.exe" -> spotify, "MSEdge" -> msedge, "SpotifyAB.SpotifyMusic_zpd...!Spotify" -> spotifyab.spotifymusic.
fn app_tokens(aumid: &str) -> Vec<String> {
    let lower = aumid.to_lowercase();
    let base = lower.split('!').next().unwrap_or(&lower);
    let exe = base.rsplit(['\\', '/']).next().unwrap_or(base).trim_end_matches(".exe");
    let mut t = vec![exe.to_string()];
    if let Some((pkg, _)) = base.split_once('_') {
        t.push(pkg.to_string());
    }
    t.retain(|s| s.len() >= 3);
    t
}

/// The audio sessions on the default speakers that belong to the app.
unsafe fn app_sessions(aumid: &str) -> Vec<ISimpleAudioVolume> {
    let tokens = app_tokens(aumid);
    let mut out = Vec::new();
    let Some(dev) = enumerator().ok().and_then(|e| e.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()) else {
        return out;
    };
    let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return out };
    let Ok(list) = mgr.GetSessionEnumerator() else { return out };
    for i in 0..list.GetCount().unwrap_or(0) {
        let Ok(ctl) = list.GetSession(i) else { continue };
        let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else { continue };
        let pid = ctl2.GetProcessId().unwrap_or(0);
        if pid == 0 {
            continue;
        }
        let Some(path) = crate::win::process_path(pid) else { continue };
        let path = path.to_lowercase();
        let stem = crate::win::file_stem_lower(&path);
        if tokens.iter().any(|t| *t == stem || path.contains(t.as_str())) {
            if let Ok(v) = ctl.cast::<ISimpleAudioVolume>() {
                out.push(v);
            }
        }
    }
    out
}

/// The app's volume, or None if it has no audio session we can match.
pub fn app_volume(aumid: String) -> Option<AppVolume> {
    WORKER.get()?.call(move || unsafe {
        let s = app_sessions(&aumid);
        let v = s.first()?;
        Some(AppVolume {
            volume: v.GetMasterVolume().unwrap_or(1.0),
            muted: s.iter().all(|v| v.GetMute().map(|b| b.as_bool()).unwrap_or(false)),
        })
    })?
}

/// Set the app's volume (all its sessions); turning it up unmutes it.
pub fn set_app_volume(aumid: String, volume: Option<f32>, muted: Option<bool>) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || unsafe {
        for v in app_sessions(&aumid) {
            if let Some(vol) = volume {
                let vol = vol.clamp(0.0, 1.0);
                let _ = v.SetMasterVolume(vol, std::ptr::null());
                if vol > 0.0 {
                    let _ = v.SetMute(false, std::ptr::null());
                }
            }
            if let Some(m) = muted {
                let _ = v.SetMute(m, std::ptr::null());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::app_tokens;
    #[test]
    fn tokens() {
        assert_eq!(app_tokens("Spotify.exe"), vec!["spotify"]);
        assert_eq!(app_tokens("MSEdge"), vec!["msedge"]);
        assert_eq!(app_tokens("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), vec!["spotifyab.spotifymusic_zpdnekdrzrea0", "spotifyab.spotifymusic"]);
    }
}
