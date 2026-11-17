//! User settings, stored as JSON in the app config folder and edited from the settings window.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Widgets that exist in this build, in their default order.
pub const KNOWN_WIDGETS: &[&str] =
    &["apps", "media", "timer", "stopwatch", "recycle", "brightness", "mic", "volume", "clock"];

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Bottom,
    Top,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum BarSize {
    Small,
    Large,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    System,
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    Acrylic,
    Mica,
}

/// Where the widgets sit along the bar.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Center,
    Left,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WidgetEntry {
    pub id: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub edge: Edge,
    pub size: BarSize,
    pub theme: ThemePref,
    pub material: Material,
    pub align: Align,
    pub clock24h: bool,
    pub clock_seconds: bool,
    pub week_starts_monday: bool,
    pub widgets: Vec<WidgetEntry>,
    pub autostart: bool,
    pub hide_taskbar: bool,
    pub hotkeys: Hotkeys,
    /// Timer presets in seconds.
    pub timer_presets: Vec<u32>,
}

/// Global shortcuts, as strings like "Win+Alt+M". Empty disables one.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub mic_mute: String,
    pub speaker_mute: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self { mic_mute: "Win+Alt+M".into(), speaker_mute: "Win+Alt+S".into() }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: Edge::Bottom,
            size: BarSize::Small,
            theme: ThemePref::System,
            material: Material::Acrylic,
            align: Align::Center,
            clock24h: true,
            clock_seconds: true,
            week_starts_monday: true,
            widgets: KNOWN_WIDGETS
                .iter()
                .map(|id| WidgetEntry { id: id.to_string(), enabled: true })
                .collect(),
            autostart: false,
            hide_taskbar: true,
            hotkeys: Hotkeys::default(),
            timer_presets: vec![60, 300, 600, 1500, 3600],
        }
    }
}

impl Settings {
    /// Drop unknown or duplicate widgets and append any new ones.
    pub fn normalize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.widgets
            .retain(|w| KNOWN_WIDGETS.contains(&w.id.as_str()) && seen.insert(w.id.clone()));
        // New widgets go after their nearest known predecessor in the default order.
        for (i, id) in KNOWN_WIDGETS.iter().enumerate() {
            if self.widgets.iter().any(|w| w.id == *id) {
                continue;
            }
            let at = KNOWN_WIDGETS[..i]
                .iter()
                .rev()
                .find_map(|prev| self.widgets.iter().position(|w| w.id == *prev))
                .map(|p| p + 1)
                .unwrap_or(0);
            self.widgets.insert(at, WidgetEntry { id: id.to_string(), enabled: true });
        }
        self.timer_presets.retain(|s| (1..=86_400).contains(s));
        self.timer_presets.dedup();
        self.timer_presets.truncate(8);
    }

    /// Bar thickness in device-independent pixels.
    pub fn thickness_dip(&self) -> f64 {
        match self.size {
            BarSize::Small => 48.0,
            BarSize::Large => 64.0,
        }
    }
}

pub fn load(path: &Path) -> Settings {
    let mut s: Settings = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    s.normalize();
    s
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(&tmp, path)
}
