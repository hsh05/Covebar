//! The card popover (opened by clicking a tile) and the window preview with live DWM thumbnails.

use crate::settings::Edge;
use crate::state::AppState;
use crate::win;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::{RECT, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
    DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    for (label, w, h) in [("card", 320.0, 360.0), ("preview", 240.0, 180.0)] {
        let win = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .title(app.package_info().name.clone())
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(true)
            .visible(false)
            .focused(false)
            .inner_size(w, h)
            .disable_drag_drop_handler()
            .build()?;
        if let Some(h) = win::tauri_hwnd(&win) {
            win::make_tool_window(h);
        }
    }
    Ok(())
}

/// Where a popup of `size` (physical px) should go, anchored to a span of the dock.
fn popup_position(app: &AppHandle, anchor_x: f64, anchor_w: f64, w: i32, h: i32) -> (i32, i32) {
    let st = app.state::<AppState>();
    let d = *st.dock.lock().unwrap();
    let mon = win::monitor_rect(win::primary_monitor());
    let gap = (8.0 * d.scale).round() as i32;
    let center = d.x + ((anchor_x + anchor_w / 2.0) * d.scale).round() as i32;
    let x = (center - w / 2).clamp(mon.left + gap, (mon.right - w - gap).max(mon.left + gap));
    let y = match d.edge {
        Edge::Bottom => d.y - h - gap,
        Edge::Top => d.y + d.h + gap,
    };
    (x, y)
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
    let scale = st.dock.lock().unwrap().scale;
    let (w, h) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let (x, y) = popup_position(app, anchor_x, anchor_w, w, h);
    let _ = card.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = card.set_position(PhysicalPosition::new(x, y));
    let _ = app.emit_to("card", "card-open", CardOpen { widget: widget.clone() });
    let _ = card.show();
    let _ = card.set_focus();
    c.open = Some(widget);
}

pub fn close_card(app: &AppHandle) {
    let st = app.state::<AppState>();
    let mut c = st.card.lock().unwrap();
    if let Some(w) = c.open.take() {
        c.last_closed = Some((w, Instant::now()));
    }
    if let Some(card) = app.get_webview_window("card") {
        let _ = card.hide();
    }
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
    let scale = st.dock.lock().unwrap().scale;
    let (w, h) = ((width * scale).round() as i32, (height * scale).round() as i32);
    let (x, y) = popup_position(app, anchor_x, anchor_w, w, h);
    let _ = pv.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = pv.set_position(PhysicalPosition::new(x, y));
    let _ = app.emit_to("preview", "preview-data", data);
    if let Some(hw) = win::tauri_hwnd(&pv) {
        win::show_no_activate(hw);
    }
}

pub fn hide_preview(app: &AppHandle) {
    clear_thumbnails(app);
    if let Some(pv) = app.get_webview_window("preview") {
        let _ = pv.hide();
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
            hide_preview(&app);
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
