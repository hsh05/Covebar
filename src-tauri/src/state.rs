use crate::settings::{Edge, Settings};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use std::time::Instant;

/// Where the dock currently is, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct DockGeom {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub scale: f64,
    pub edge: Edge,
    /// Gap (physical px) between the window's inner side and the visible bar: the window is a
    /// little taller than the bar so the pill can float and morph.
    pub inset: i32,
}

#[derive(Default)]
pub struct CardState {
    pub open: Option<String>,
    pub last_closed: Option<(String, Instant)>,
    /// Where the open card is anchored (screen-x) and its width, for `resize_card`.
    pub center: i32,
    pub width: f64,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub marker_path: PathBuf,
    pub dock: Mutex<DockGeom>,
    pub card: Mutex<CardState>,
    pub thumbs: Mutex<Vec<isize>>,
    pub preview_gen: AtomicU64,
    pub fullscreen: AtomicBool,
    pub taskbar_hidden: AtomicBool,
}
