// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Debug-build logging to the console.
#[macro_export]
macro_rules! dlog {
    ($($t:tt)*) => {{
        #[cfg(debug_assertions)]
        eprintln!("[covebar] {}", format!($($t)*));
    }};
}

mod apps;
mod appbar;
mod dock;
mod icons;
mod popups;
mod settings;
mod state;
mod taskbar;
mod watchdog;
mod win;

use settings::Settings;
use state::{AppState, CardState, DockGeom};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

// ---------------------------------------------------------------- commands

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppState>, mut settings: Settings) -> Result<Settings, String> {
    settings.normalize();
    let old = state.settings.lock().unwrap().clone();
    settings::save(&state.settings_path, &settings).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings.clone();

    if old.edge != settings.edge || old.size != settings.size {
        popups::close_card(&app, None);
        popups::close_menu(&app);
        popups::hide_preview(&app);
        dock::place(&app);
    }
    if old.theme != settings.theme || old.material != settings.material {
        dock::apply_look(&app, &settings);
    }
    if old.hide_taskbar != settings.hide_taskbar {
        dock::set_taskbar_hidden(&app, settings.hide_taskbar);
        dock::place(&app);
    }
    if old.autostart != settings.autostart {
        let al = app.autolaunch();
        let r = if settings.autostart { al.enable() } else { al.disable() };
        r.map_err(|e| e.to_string())?;
    }
    let _ = app.emit("settings-changed", &settings);
    Ok(settings)
}

#[tauri::command]
fn get_apps() -> apps::AppsSnapshot {
    apps::current()
}

#[tauri::command]
async fn get_icon(key: String, size: i32) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || icons::icon_on_worker(key, size.clamp(16, 256)))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
fn click_app(hwnds: Vec<isize>) {
    apps::click_app(&hwnds);
}

#[tauri::command]
fn activate_window(app: AppHandle, hwnd: isize) {
    popups::hide_preview(&app);
    apps::activate(win::hwnd(hwnd));
}

#[tauri::command]
fn close_window(hwnd: isize) {
    apps::close_window(hwnd);
}

#[tauri::command]
fn launch_app(target: String) {
    apps::launch(&target);
}

#[tauri::command]
fn open_start() {
    apps::open_start();
}

#[tauri::command]
fn open_card(app: AppHandle, widget: String, anchor_x: f64, anchor_width: f64, width: f64, height: f64) {
    popups::open_card(&app, widget, anchor_x, anchor_width, width, height);
}

#[tauri::command]
fn close_card(app: AppHandle, widget: Option<String>) {
    popups::close_card(&app, widget.as_deref());
}

#[tauri::command]
fn open_menu(app: AppHandle, items: serde_json::Value, width: f64, height: f64) -> u64 {
    popups::open_menu(&app, items, width, height)
}

#[tauri::command]
fn close_menu(app: AppHandle, session: Option<u64>) {
    popups::close_menu_session(&app, session);
}

#[tauri::command]
fn show_preview(app: AppHandle, data: popups::PreviewData, anchor_x: f64, anchor_width: f64, width: f64, height: f64) {
    popups::show_preview(&app, data, anchor_x, anchor_width, width, height);
}

#[tauri::command]
fn set_thumbnails(app: AppHandle, items: Vec<popups::ThumbRect>) {
    popups::set_thumbnails(&app, items);
}

#[tauri::command]
fn preview_hold(app: AppHandle) {
    popups::preview_hold(&app);
}

#[tauri::command]
fn preview_release(app: AppHandle) {
    popups::preview_release(&app);
}

#[tauri::command]
fn hide_preview(app: AppHandle) {
    popups::hide_preview(&app);
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    show_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn show_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("settings") {
        w.unminimize()?;
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title(format!("{} settings", app.package_info().name))
        .inner_size(780.0, 580.0)
        .min_inner_size(620.0, 460.0)
        .transparent(true)
        .disable_drag_drop_handler()
        .center()
        .build()?;
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    dock::apply_look(app, &s);
    Ok(())
}

// ---------------------------------------------------------------- startup

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let settings_path = app.path().app_config_dir()?.join("settings.json");
    let settings = settings::load(&settings_path);
    let marker_path = taskbar::default_marker_path();

    // A previous run that crashed may have left the taskbar hidden: put it back first.
    taskbar::restore(&marker_path);

    app.manage(AppState {
        settings: Mutex::new(settings.clone()),
        settings_path,
        marker_path,
        dock: Mutex::new(DockGeom { x: 0, y: 0, w: 0, h: 0, scale: 1.0, edge: settings.edge }),
        card: Mutex::new(CardState::default()),
        thumbs: Mutex::new(Vec::new()),
        preview_gen: AtomicU64::new(0),
        fullscreen: AtomicBool::new(false),
        taskbar_hidden: AtomicBool::new(false),
    });

    // Keep the autostart entry in sync with the setting.
    let al = handle.autolaunch();
    if al.is_enabled().unwrap_or(false) != settings.autostart {
        let _ = if settings.autostart { al.enable() } else { al.disable() };
    }

    if settings.hide_taskbar {
        dock::set_taskbar_hidden(&handle, true);
    }

    let dock_win = app.get_webview_window("dock").ok_or("dock window missing")?;
    if let Some(h) = win::tauri_hwnd(&dock_win) {
        win::make_tool_window(h);
        appbar::register(h);
    }
    dock::place(&handle);
    popups::create(&handle)?;
    dock::apply_look(&handle, &settings);
    if let Some(h) = win::tauri_hwnd(&dock_win) {
        win::show_no_activate(h);
    }

    apps::start(handle.clone());
    dock::start_watcher(handle);
    Ok(())
}

fn cleanup(app: &AppHandle) {
    if let Some(h) = app.get_webview_window("dock").and_then(|d| win::tauri_hwnd(&d)) {
        appbar::remove(h);
    }
    let st = app.state::<AppState>();
    if st.taskbar_hidden.swap(false, Ordering::SeqCst) {
        taskbar::restore(&st.marker_path);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--watchdog") => return watchdog::run(&args[2..]),
        Some("--restore-taskbar") => {
            if !taskbar::restore(&taskbar::default_marker_path()) {
                taskbar::force_restore();
            }
            return;
        }
        Some("--dump-icons") => return apps::dump_icons(args.get(2).map(String::as_str).unwrap_or("."), args.get(3).and_then(|s| s.parse().ok()).unwrap_or(64)),
        _ => {}
    }

    // Never leave the user without a taskbar, even if we panic.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        taskbar::restore(&taskbar::default_marker_path());
        prev(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            get_apps,
            get_icon,
            click_app,
            activate_window,
            close_window,
            launch_app,
            open_start,
            open_card,
            close_card,
            open_menu,
            close_menu,
            show_preview,
            set_thumbnails,
            preview_hold,
            preview_release,
            hide_preview,
            open_settings,
            quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Covebar")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                cleanup(app);
            }
        });
}
