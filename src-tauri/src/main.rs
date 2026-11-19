// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Log a line to the log file (and, in debug builds, the console).
#[macro_export]
macro_rules! dlog {
    ($($t:tt)*) => {{
        let line = format!($($t)*);
        #[cfg(debug_assertions)]
        eprintln!("[covebar] {line}");
        $crate::log::write(&line);
    }};
}

mod log;
mod apps;
mod audio;
mod media;
mod worker;
mod appbar;
mod autohide;
mod brightness;
mod dock;
mod hotkeys;
mod update;
mod winkeys;
mod icons;
mod popups;
mod recycle;
mod settings;
mod state;
mod taskbar;
mod timers;
mod tray;
mod net;
mod notify;
mod currency;
mod temps;
mod claude;
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

    if old.edge != settings.edge || old.size != settings.size || old.shape != settings.shape {
        popups::close_card(&app, None);
        popups::close_menu(&app);
        popups::hide_preview(&app);
        dock::place(&app);
    }
    if old.shape != settings.shape && settings.shape == settings::Shape::Full {
        // Let the pill finish growing into the full bar before the backdrop appears behind it.
        let (app, s) = (app.clone(), settings.clone());
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(460));
            dock::apply_look(&app, &s);
        });
    } else if old.theme != settings.theme || old.material != settings.material || old.shape != settings.shape {
        dock::apply_look(&app, &settings);
    }
    if old.auto_hide != settings.auto_hide {
        dock::place(&app);
    }
    if old.hide_taskbar != settings.hide_taskbar {
        dock::set_taskbar_hidden(&app, settings.hide_taskbar);
        dock::place(&app);
    }
    winkeys::set_enabled(settings.hotkeys.win_numbers && settings.has_widget("apps"));
    if old.hotkeys != settings.hotkeys {
        hotkeys::update(settings.hotkeys.clone());
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
fn run_as_admin(target: String) {
    apps::run_as_admin(&target);
}

#[tauri::command]
fn open_location(path: String) {
    apps::open_location(&path);
}

#[tauri::command]
fn show_windows(hwnds: Vec<isize>, minimize: bool) {
    apps::show_windows(&hwnds, minimize);
}

#[tauri::command]
fn pin_app(launch: String, name: String, location: Option<String>) -> Result<(), String> {
    apps::pin(&launch, &name, location)
}

#[tauri::command]
fn unpin_app(lnk: String) -> Result<(), String> {
    apps::unpin(&lnk)
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
fn open_media_source(source: String) {
    std::thread::spawn(move || apps::open_media_source(&source));
}

#[tauri::command]
fn resize_card(app: AppHandle, widget: String, height: f64) {
    popups::resize_card(&app, &widget, height);
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

// ---------------------------------------------------------------- phase 2 widgets

#[tauri::command]
fn get_media() -> Option<media::MediaInfo> {
    media::current()
}

#[tauri::command]
fn media_command(action: String) {
    media::command(action);
}

#[tauri::command]
fn get_app_volume(source: String) -> Option<audio::AppVolume> {
    audio::app_volume(source)
}

#[tauri::command]
fn set_app_volume(source: String, volume: Option<f32>, muted: Option<bool>) {
    audio::set_app_volume(source, volume, muted);
}

/// The dock's visible bar, in the dock's CSS px; clicks elsewhere in its window pass through.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // the frontend passes the rect as named fields
fn set_dock_region(app: AppHandle, state: State<AppState>, x: f64, y: f64, w: f64, h: f64, radius: Option<f64>, full: bool) {
    let Some(dock) = app.get_webview_window("dock") else { return };
    let Some(hw) = win::tauri_hwnd(&dock) else { return };
    let d = *state.dock.lock().unwrap();
    let k = d.scale;
    let r = windows::Win32::Foundation::RECT {
        left: (x * k).floor() as i32,
        top: (y * k).floor() as i32,
        right: ((x + w) * k).ceil() as i32,
        bottom: ((y + h) * k).ceil() as i32,
    };
    autohide::set_region(hw, (!full).then_some(r), (radius.unwrap_or(0.0) * k).round() as i32, &d);
}

#[tauri::command]
fn media_seek(seconds: f64) {
    media::seek(seconds);
}

#[tauri::command]
fn get_audio() -> audio::AudioState {
    audio::current()
}

#[tauri::command]
fn set_volume(which: String, volume: f32) {
    audio::set_volume(which, volume);
}

#[tauri::command]
fn set_mute(which: String, muted: bool) {
    audio::set_mute(which, muted);
}

#[tauri::command]
fn set_default_output(id: String) {
    audio::set_default_output(id);
}

#[tauri::command]
fn get_brightness() -> Vec<brightness::Display> {
    brightness::current()
}

#[tauri::command]
fn set_brightness(id: String, value: u32) {
    brightness::set(id, value);
}

#[tauri::command]
fn get_recycle() -> recycle::BinInfo {
    recycle::current()
}

#[tauri::command]
async fn empty_recycle() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(recycle::empty).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_recycle() {
    recycle::open();
}

#[tauri::command]
fn show_alarm(app: AppHandle, anchor_x: Option<f64>, anchor_width: Option<f64>) {
    if timers::get().timer.ringing {
        let d = timers::get().timer.duration_ms;
        popups::show_alarm(&app, anchor_x.zip(anchor_width), d);
    }
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<update::UpdateInfo>, String> {
    update::check(&app).await
}

#[tauri::command]
fn found_update() -> Option<update::UpdateInfo> {
    update::found()
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

#[tauri::command]
fn leave_bar(restore: bool) {
    dock::leave_bar(restore);
}

#[tauri::command]
fn show_tour(app: AppHandle) {
    popups::show_tour(&app);
}

#[tauri::command]
fn hide_tour(app: AppHandle) {
    popups::hide_tour(&app);
}

#[tauri::command]
fn popup_ready(app: AppHandle, window: tauri::WebviewWindow) {
    popups::popup_ready(&app, window.label());
}

#[tauri::command]
fn open_log_folder() {
    if let Some(d) = log::dir() {
        apps::launch(&d.to_string_lossy());
    }
}

#[tauri::command]
fn get_timers() -> timers::TimersState {
    timers::get()
}

#[tauri::command]
fn timers_action(action: String, value: Option<u64>) -> timers::TimersState {
    timers::action(&action, value)
}

// ---------------------------------------------------------------- phase 3 widgets

#[tauri::command]
fn get_tray() -> tray::TrayState {
    tray::current()
}

#[tauri::command]
fn tray_click(app: AppHandle, id: String, button: String, anchor_x: Option<f64>, anchor_width: Option<f64>) {
    let center = anchor_x.map(|x| popups::anchor_center(&app, x, anchor_width.unwrap_or(0.0)));
    tray::click(id, button, center);
}

#[tauri::command]
fn get_temps() -> temps::Temps {
    temps::current()
}

#[tauri::command]
fn get_currency() -> currency::CurrencyState {
    currency::current()
}

#[tauri::command]
fn refresh_currency() {
    currency::refresh();
}

#[tauri::command]
fn get_claude() -> claude::Usage {
    claude::current()
}

#[tauri::command]
fn refresh_claude() {
    claude::refresh();
}

#[tauri::command]
fn pause_hotkeys(paused: bool) {
    hotkeys::set_paused(paused);
}

#[tauri::command]
fn get_hotkey_status() -> hotkeys::HotkeyStatus {
    hotkeys::status()
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    show_settings(&app).map_err(|e| e.to_string())
}

/// The section the settings window should show when it next opens or is brought forward.
static SETTINGS_SECTION: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
async fn open_settings_at(app: AppHandle, section: String) -> Result<(), String> {
    *SETTINGS_SECTION.lock().unwrap() = Some(section.clone());
    show_settings(&app).map_err(|e| e.to_string())?;
    let _ = app.emit_to("settings", "settings-section", section);
    Ok(())
}

#[tauri::command]
fn take_settings_section() -> Option<String> {
    SETTINGS_SECTION.lock().unwrap().take()
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
    let config_dir = app.path().app_config_dir()?;
    log::init(&config_dir.join("logs"));
    let settings_path = config_dir.join("settings.json");
    let settings = settings::load(&settings_path);
    let marker_path = taskbar::default_marker_path();

    // A previous run that crashed may have left the taskbar hidden: put it back first.
    taskbar::restore(&marker_path);

    app.manage(AppState {
        settings: Mutex::new(settings.clone()),
        settings_path,
        marker_path,
        dock: Mutex::new(DockGeom { x: 0, y: 0, w: 0, h: 0, scale: 1.0, edge: settings.edge, inset: 0 }),
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
    media::start(handle.clone());
    audio::start(handle.clone());
    brightness::start(handle.clone());
    recycle::start(handle.clone());
    timers::start(handle.clone(), config_dir.join("timers.json"));
    hotkeys::start(handle.clone(), settings.hotkeys.clone());
    winkeys::start(settings.hotkeys.win_numbers && settings.has_widget("apps"));
    tray::start(handle.clone());
    temps::start(handle.clone());
    currency::start(handle.clone(), config_dir.join("currency.json"));
    claude::start(handle.clone());
    autohide::start(handle.clone());
    update::start(handle.clone());
    if !settings.tour_done {
        // Give the bar a moment to settle before the tour points at it.
        let h = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let a = h.clone();
            let _ = h.run_on_main_thread(move || popups::show_tour(&a));
        });
    }
    dock::start_watcher(handle);
    Ok(())
}

pub(crate) fn cleanup(app: &AppHandle) {
    timers::shutdown();
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

    // WebView2 stops drawing a window it thinks is covered (here: the bar behind a fullscreen
    // app) and only starts again on input, so the bar came back see-through when Start opened.
    // Chromium reads one --disable-features list, so Tauri's own defaults are repeated here.
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion",
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            resize_card,
            open_media_source,
            get_tray,
            tray_click,
            get_temps,
            get_currency,
            refresh_currency,
            get_claude,
            refresh_claude,
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
            get_media,
            media_command,
            media_seek,
            set_dock_region,
            run_as_admin,
            open_location,
            show_windows,
            pin_app,
            unpin_app,
            get_app_volume,
            set_app_volume,
            get_audio,
            set_volume,
            set_mute,
            set_default_output,
            get_brightness,
            set_brightness,
            get_recycle,
            empty_recycle,
            open_recycle,
            get_timers,
            timers_action,
            show_alarm,
            open_log_folder,
            show_tour,
            leave_bar,
            check_update,
            found_update,
            install_update,
            hide_tour,
            popup_ready,
            get_hotkey_status,
            pause_hotkeys,
            open_settings,
            open_settings_at,
            take_settings_section,
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
