//! Updates from the project's GitHub releases. Release builds look once shortly after
//! starting and then twice a day; a found update is announced to the bar and the settings
//! window, and installed only when the user asks. Every update is signature-checked against
//! the public key in tauri.conf.json before it runs.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

static FOUND: Mutex<Option<UpdateInfo>> = Mutex::new(None);

pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio_sleep(Duration::from_secs(45)).await;
        loop {
            if let Err(e) = check(&app).await {
                crate::dlog!("update check failed: {e}");
            }
            tokio_sleep(Duration::from_secs(12 * 3600)).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

/// The update found by the last check, if any.
pub fn found() -> Option<UpdateInfo> {
    FOUND.lock().unwrap().clone()
}

pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let info = update.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() });
    if let Some(i) = &info {
        crate::dlog!("update available: {}", i.version);
    }
    *FOUND.lock().unwrap() = info.clone();
    let _ = app.emit("update-available", &info);
    Ok(info)
}

/// Download and run the installer, then restart into the new version.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Covebar is already up to date.")?;
    crate::dlog!("installing update {}", update.version);
    let progress = app.clone();
    let mut got = 0u64;
    let bytes = update
        .download(
            move |chunk, total| {
                got += chunk as u64;
                let _ = progress.emit("update-progress", (got, total));
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    // The installer closes this process: put the Windows taskbar back first.
    crate::cleanup(app);
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
