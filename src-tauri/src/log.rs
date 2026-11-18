//! A small log file (`logs\covebar.log` in the config folder) that release builds write too,
//! so problems can be diagnosed after the fact. It is capped at 512 KB; the previous file is
//! kept as `covebar.old.log`. Never log tokens, credentials or window contents here.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 512 * 1024;

struct Sink {
    path: PathBuf,
    file: File,
    written: u64,
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);
static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Start logging into `dir`. Logs a panic's message and location before the app goes down.
pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = DIR.set(dir.to_path_buf());
    let path = dir.join("covebar.log");
    if let Some(f) = open(&path) {
        let written = f.metadata().map(|m| m.len()).unwrap_or(0);
        *SINK.lock().unwrap() = Some(Sink { path, file: f, written });
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".into());
        write(&format!("PANIC{at}: {msg}"));
        default(info);
    }));
    write(&format!("--- Covebar {} started (Windows build {})", env!("CARGO_PKG_VERSION"), crate::win::build_number()));
}

pub fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

fn open(path: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Append one line, prefixed with the local time.
pub fn write(line: &str) {
    let mut g = match SINK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let Some(sink) = g.as_mut() else { return };
    if sink.written > MAX_BYTES {
        let old = sink.path.with_file_name("covebar.old.log");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&sink.path, &old);
        match open(&sink.path) {
            Some(f) => {
                sink.file = f;
                sink.written = 0;
            }
            None => return,
        }
    }
    let text = format!("{} {}\n", timestamp(), line);
    if sink.file.write_all(text.as_bytes()).is_ok() {
        sink.written += text.len() as u64;
    }
}

fn timestamp() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}
