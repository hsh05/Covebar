//! Recycle Bin: item count and size, empty, open.

use crate::worker::{com_mta, Worker};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};

#[derive(Serialize, Clone, Copy, PartialEq, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BinInfo {
    pub items: u64,
    pub bytes: u64,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Option<BinInfo>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(com_mta, Duration::from_secs(3), refresh));
}

pub fn current() -> BinInfo {
    CURRENT.lock().unwrap().unwrap_or_default()
}

fn query() -> BinInfo {
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    match unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) } {
        Ok(()) => BinInfo { items: info.i64NumItems.max(0) as u64, bytes: info.i64Size.max(0) as u64 },
        Err(_) => BinInfo::default(),
    }
}

fn refresh() {
    let info = query();
    let mut cur = CURRENT.lock().unwrap();
    if *cur != Some(info) {
        *cur = Some(info);
        if let Some(app) = APP.get() {
            let _ = app.emit("recycle-changed", info);
        }
    }
}

/// Progress of an empty in flight, sent as `recycle-progress` every ~120 ms.
#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// 0..1, never goes backwards.
    fraction: f64,
    deleted: u64,
    total: u64,
}

/// Watch the bin shrink while the shell empties it and report how far along it is.
fn watch_progress(start: BinInfo, done: Arc<AtomicBool>) {
    let Some(app) = APP.get().cloned() else { return };
    std::thread::spawn(move || {
        let mut best = 0.0f64;
        while !done.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(120));
            let now = query();
            let by_bytes = if start.bytes > 0 { 1.0 - now.bytes as f64 / start.bytes as f64 } else { 0.0 };
            let by_items = if start.items > 0 { 1.0 - now.items as f64 / start.items as f64 } else { 0.0 };
            // Leave the last bit for the real finish.
            best = best.max(by_bytes.max(by_items).clamp(0.0, 0.97));
            let deleted = start.items.saturating_sub(now.items);
            let _ = app.emit("recycle-progress", Progress { fraction: best, deleted, total: start.items });
        }
        let _ = app.emit("recycle-progress", Progress { fraction: 1.0, deleted: start.items, total: start.items });
    });
}

/// Empty the bin. The frontend asks for confirmation first, so no shell dialog here.
pub fn empty() -> Result<(), String> {
    let w = WORKER.get().ok_or("not started")?;
    w.call(|| {
        let done = Arc::new(AtomicBool::new(false));
        watch_progress(query(), done.clone());
        let _guard = SetOnDrop(done);
        let r = unsafe {
            SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)
        };
        refresh();
        // Emptying an already-empty bin reports an error; that is fine.
        match r {
            Ok(()) => Ok(()),
            Err(_) if query().items == 0 => Ok(()),
            Err(e) => Err(e.message()),
        }
    })
    .unwrap_or_else(|| Err("worker stopped".into()))
}

pub fn open() {
    crate::apps::launch("shell:RecycleBinFolder");
}

struct SetOnDrop(Arc<AtomicBool>);
impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
