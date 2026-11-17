//! A tiny helper process that restores the taskbar if Covebar exits without cleaning up
//! (crash, kill from Task Manager, `tauri dev` restart).

use crate::taskbar;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE};

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

static SPAWNED: AtomicBool = AtomicBool::new(false);

pub fn spawn(marker: &Path) {
    if SPAWNED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Ok(mut exe) = std::env::current_exe() else { return };
    // In debug builds run a copy, so the watchdog never locks the exe cargo wants to rebuild.
    if cfg!(debug_assertions) {
        let copy = std::env::temp_dir().join("covebar-watchdog.exe");
        if std::fs::copy(&exe, &copy).is_ok() {
            exe = copy;
        }
    }
    let args = [
        "--watchdog".to_string(),
        std::process::id().to_string(),
        marker.to_string_lossy().to_string(),
    ];
    let ok = Command::new(&exe)
        .args(&args)
        .creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
        .is_ok();
    if !ok {
        let _ = Command::new(&exe).args(&args).creation_flags(DETACHED_PROCESS).spawn();
    }
}

/// Entry point for `covebar.exe --watchdog <pid> <marker>`.
pub fn run(args: &[String]) {
    let Some(pid) = args.first().and_then(|p| p.parse::<u32>().ok()) else { return };
    let marker = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(taskbar::default_marker_path);
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, INFINITE);
            let _ = CloseHandle(h);
        }
    }
    taskbar::restore(&marker);
}
