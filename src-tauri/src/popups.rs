//! The card popover (opened by clicking a tile) and the window preview with live DWM thumbnails.

use crate::settings::Edge;
use crate::state::AppState;
use crate::win;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::{POINT, RECT, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect, IsWindowVisible};
use windows::Win32::Graphics::Dwm::{
    DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
    DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};

/// Transparent margin (CSS px) around each popup's content, room for its CSS shadow.
/// Popups draw their own surface so they can animate in and out smoothly.
pub const PAD: f64 = 16.0;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    for (label, w, h) in [("card", 320.0, 360.0), ("preview", 240.0, 180.0), ("menu", 240.0, 200.0), ("osd", OSD_SIZE, OSD_SIZE)] {
        let win = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .title(app.package_info().name.clone())
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .focused(false)
            .inner_size(w, h)
            .disable_drag_drop_handler()
            .build()?;
        if let Some(h) = win::tauri_hwnd(&win) {
            win::make_tool_window(h);
        }
        if label == "osd" {
            // Purely informational: clicks go straight through to whatever is underneath.
            let _ = win.set_ignore_cursor_events(true);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- on-screen display

/// Size (CSS px) of the on-screen display's content, without the shadow margin.
const OSD_SIZE: f64 = 200.0;
static OSD_SESSION: AtomicU64 = AtomicU64::new(0);

/// Flash a big indicator in the middle of the primary monitor (e.g. "Microphone muted"),
/// without taking focus. Repeated calls restart its timer.
pub fn show_osd(app: &AppHandle, kind: &str, on: bool) {
    let Some(osd) = app.get_webview_window("osd") else { return };
    let Some(h) = win::tauri_hwnd(&osd) else { return };
    let me = OSD_SESSION.fetch_add(1, Ordering::SeqCst) + 1;
    let mon = win::monitor_rect(win::primary_monitor());
    let scale = win::monitor_scale(win::primary_monitor());
    let full = ((OSD_SIZE + 2.0 * PAD) * scale).round() as i32;
    let x = (mon.left + mon.right - full) / 2;
    let y = (mon.top + mon.bottom - full) / 2;
    let _ = osd.set_size(PhysicalSize::new(full as u32, full as u32));
    let _ = osd.set_position(PhysicalPosition::new(x, y));
    let _ = app.emit_to("osd", "osd-show", serde_json::json!({ "kind": kind, "on": on, "session": me }));
    win::show_no_activate(h);
    let app = app.clone();
    let hw = win::hval(h);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1300));
        if OSD_SESSION.load(Ordering::SeqCst) != me {
            return;
        }
        let _ = app.emit_to("osd", "osd-out", me);
        std::thread::sleep(Duration::from_millis(260));
        if OSD_SESSION.load(Ordering::SeqCst) == me {
            win::hide(win::hwnd(hw));
        }
    });
}

/// Top-left of a `w`x`h` (physical px) content box centred on screen-x `center`, next to the dock.
fn content_position(app: &AppHandle, center: i32, w: i32, h: i32) -> (i32, i32) {
    let st = app.state::<AppState>();
    let d = *st.dock.lock().unwrap();
    let mon = win::monitor_rect(win::primary_monitor());
    let gap = (8.0 * d.scale).round() as i32;
    let x = (center - w / 2).clamp(mon.left + gap, (mon.right - w - gap).max(mon.left + gap));
    let y = match d.edge {
        Edge::Bottom => d.y - h - gap,
        Edge::Top => d.y + d.h + gap,
    };
    (x, y)
}

/// Size and move a popup window so its content (`width`x`height` CSS px) sits centred on
/// screen-x `center` next to the dock, with the transparent shadow margin around it.
fn place_popup(app: &AppHandle, win: &tauri::WebviewWindow, center: i32, width: f64, height: f64) {
    let scale = app.state::<AppState>().dock.lock().unwrap().scale;
    let (w, h) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let pad = (PAD * scale).round() as i32;
    let (x, y) = content_position(app, center, w, h);
    let _ = win.set_size(PhysicalSize::new((w + 2 * pad) as u32, (h + 2 * pad) as u32));
    let _ = win.set_position(PhysicalPosition::new(x - pad, y - pad));
}

/// Screen-x (physical) of the centre of a dock span given in the dock's CSS px.
fn anchor_center(app: &AppHandle, anchor_x: f64, anchor_w: f64) -> i32 {
    let d = *app.state::<AppState>().dock.lock().unwrap();
    d.x + ((anchor_x + anchor_w / 2.0) * d.scale).round() as i32
}

// ---------------------------------------------------------------- card

#[derive(Serialize, Clone)]
struct CardOpen {
    widget: String,
}

pub fn open_card(app: &AppHandle, widget: String, anchor_x: f64, anchor_w: f64, width: f64, height: f64) {
    let st = app.state::<AppState>();
    let mut c = st.card.lock().unwrap();
    // A click on the tile first blurs the open card, closing it; don't reopen it right away.
    if let Some((w, t)) = &c.last_closed {
        if *w == widget && t.elapsed() < Duration::from_millis(350) {
            c.last_closed = None;
            return;
        }
    }
    let Some(card) = app.get_webview_window("card") else { return };
    if c.open.as_deref() == Some(widget.as_str()) {
        let _ = card.hide();
        c.open = None;
        return;
    }
    hide_preview(app);
    close_menu(app);
    place_popup(app, &card, anchor_center(app, anchor_x, anchor_w), width, height);
    let _ = app.emit_to("card", "card-open", CardOpen { widget: widget.clone() });
    let _ = card.show();
    let _ = card.set_focus();
    c.open = Some(widget);
}

/// Close the card. With `only`, close it only if that widget's card is the one open
/// (the card's own delayed close after its exit animation must not close a newer card).
pub fn close_card(app: &AppHandle, only: Option<&str>) {
    let st = app.state::<AppState>();
    let mut c = st.card.lock().unwrap();
    if only.is_some() && c.open.as_deref() != only {
        return;
    }
    if let Some(w) = c.open.take() {
        c.last_closed = Some((w, Instant::now()));
    }
    if let Some(card) = app.get_webview_window("card") {
        let _ = card.hide();
    }
}

// ---------------------------------------------------------------- menu

static MENU_SESSION: AtomicU64 = AtomicU64::new(0);

/// Show the context menu (`items` are rendered by the menu view) next to the dock, at the
/// cursor. Returns the menu's session id, which the menu passes back when it closes.
pub fn open_menu(app: &AppHandle, items: serde_json::Value, width: f64, height: f64) -> u64 {
    let Some(menu) = app.get_webview_window("menu") else { return 0 };
    close_card(app, None);
    hide_preview(app);
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    let session = MENU_SESSION.fetch_add(1, Ordering::SeqCst) + 1;
    let scale = app.state::<AppState>().dock.lock().unwrap().scale;
    // Open to the right of the cursor, like a native menu.
    let center = p.x + (width * scale / 2.0).round() as i32;
    place_popup(app, &menu, center, width, height);
    let _ = app.emit_to("menu", "menu-open", serde_json::json!({ "session": session, "items": items }));
    let _ = menu.show();
    let _ = menu.set_focus();
    session
}

/// Hide the menu; with `session`, only if it is still that menu.
pub fn close_menu_session(app: &AppHandle, session: Option<u64>) {
    if session.is_some_and(|s| s != MENU_SESSION.load(Ordering::SeqCst)) {
        return;
    }
    if let Some(menu) = app.get_webview_window("menu") {
        let _ = menu.hide();
    }
}

pub fn close_menu(app: &AppHandle) {
    close_menu_session(app, None);
}

// ---------------------------------------------------------------- preview

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWin {
    pub hwnd: isize,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewData {
    pub app_key: String,
    pub name: String,
    pub icon: String,
    pub windows: Vec<PreviewWin>,
}

#[derive(Deserialize)]
pub struct ThumbRect {
    pub hwnd: isize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn show_preview(app: &AppHandle, data: PreviewData, anchor_x: f64, anchor_w: f64, width: f64, height: f64) {
    let st = app.state::<AppState>();
    if st.card.lock().unwrap().open.is_some() {
        return;
    }
    st.preview_gen.fetch_add(1, Ordering::SeqCst);
    let Some(pv) = app.get_webview_window("preview") else { return };
    clear_thumbnails(app);
    place_popup(app, &pv, anchor_center(app, anchor_x, anchor_w), width, height);
    let _ = app.emit_to("preview", "preview-data", data);
    if let Some(hw) = win::tauri_hwnd(&pv) {
        win::show_no_activate(hw);
        watch_cursor(app.clone(), win::hval(hw));
    }
}

static PREVIEW_SESSION: AtomicU64 = AtomicU64::new(0);

/// Pointer-leave events are unreliable when the cursor leaves a WebView window quickly, so
/// also poll the cursor: once it has been off both the dock and the preview for a moment,
/// hide the preview.
fn watch_cursor(app: AppHandle, preview: isize) {
    let me = PREVIEW_SESSION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let inside = |r: &RECT, p: &POINT| p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom;
        let mut away_since: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(60));
            if PREVIEW_SESSION.load(Ordering::SeqCst) != me {
                return;
            }
            let pv = win::hwnd(preview);
            if unsafe { !IsWindowVisible(pv).as_bool() } {
                return;
            }
            let mut p = POINT::default();
            let mut pr = RECT::default();
            unsafe {
                let _ = GetCursorPos(&mut p);
                let _ = GetWindowRect(pv, &mut pr);
            }
            let d = *app.state::<AppState>().dock.lock().unwrap();
            let dock = RECT { left: d.x, top: d.y, right: d.x + d.w, bottom: d.y + d.h };
            if inside(&pr, &p) || inside(&dock, &p) {
                away_since = None;
            } else if away_since.get_or_insert_with(Instant::now).elapsed() > Duration::from_millis(300) {
                fade_out_preview(&app);
                return;
            }
        }
    });
}

/// Play the preview's exit animation, then hide it (unless it was reopened meanwhile).
pub fn fade_out_preview(app: &AppHandle) {
    let visible = app.get_webview_window("preview").and_then(|w| w.is_visible().ok());
    if visible != Some(true) {
        return;
    }
    let me = PREVIEW_SESSION.load(Ordering::SeqCst);
    // Live thumbnails can't fade, so drop them first; the frame then fades out.
    clear_thumbnails(app);
    let _ = app.emit_to("preview", "preview-out", ());
    let _ = app.emit_to("dock", "preview-hidden", ());
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(140));
        if PREVIEW_SESSION.load(Ordering::SeqCst) == me {
            hide_preview(&app);
        }
    });
}

pub fn hide_preview(app: &AppHandle) {
    PREVIEW_SESSION.fetch_add(1, Ordering::SeqCst);
    clear_thumbnails(app);
    if let Some(pv) = app.get_webview_window("preview") {
        if pv.is_visible().unwrap_or(false) {
            if let Some(h) = win::tauri_hwnd(&pv) {
                win::hide(h);
            }
            let _ = app.emit_to("dock", "preview-hidden", ());
        }
    }
}

pub fn preview_hold(app: &AppHandle) {
    app.state::<AppState>().preview_gen.fetch_add(1, Ordering::SeqCst);
}

/// Hide the preview shortly, unless the pointer moves onto the dock tile or the preview first.
pub fn preview_release(app: &AppHandle) {
    let g = app.state::<AppState>().preview_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(320));
        if app.state::<AppState>().preview_gen.load(Ordering::SeqCst) == g {
            fade_out_preview(&app);
        }
    });
}

fn clear_thumbnails(app: &AppHandle) {
    let st = app.state::<AppState>();
    for t in st.thumbs.lock().unwrap().drain(..) {
        unsafe {
            let _ = DwmUnregisterThumbnail(t);
        }
    }
}

/// Register live thumbnails of `items` into the preview window at the given client rects (physical px).
pub fn set_thumbnails(app: &AppHandle, items: Vec<ThumbRect>) {
    clear_thumbnails(app);
    let Some(pv) = app.get_webview_window("preview") else { return };
    let Some(dest) = win::tauri_hwnd(&pv) else { return };
    let st = app.state::<AppState>();
    let mut thumbs = st.thumbs.lock().unwrap();
    for it in items {
        unsafe {
            let Ok(id) = DwmRegisterThumbnail(dest, win::hwnd(it.hwnd)) else { continue };
            let src = DwmQueryThumbnailSourceSize(id).unwrap_or_default();
            let (sw, sh) = (src.cx.max(1) as f64, src.cy.max(1) as f64);
            let k = (it.w / sw).min(it.h / sh).min(1.0);
            let (tw, th) = (sw * k, sh * k);
            let left = it.x + (it.w - tw) / 2.0;
            let top = it.y + (it.h - th) / 2.0;
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination: RECT {
                    left: left.round() as i32,
                    top: top.round() as i32,
                    right: (left + tw).round() as i32,
                    bottom: (top + th).round() as i32,
                },
                opacity: 255,
                fVisible: TRUE,
                fSourceClientAreaOnly: false.into(),
                ..Default::default()
            };
            let _ = DwmUpdateThumbnailProperties(id, &props);
            thumbs.push(id);
        }
    }
}
