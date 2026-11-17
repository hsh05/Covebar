//! User settings, stored as JSON in the app config folder and edited from the settings window.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Widgets that exist in this build, in their default order.
pub const KNOWN_WIDGETS: &[&str] = &["apps", "clock"];

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
    pub clock24h: bool,
    pub clock_seconds: bool,
    pub week_starts_monday: bool,
    pub widgets: Vec<WidgetEntry>,
    pub autostart: bool,
    pub hide_taskbar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: Edge::Bottom,
            size: BarSize::Small,
            theme: ThemePref::System,
            material: Material::Acrylic,
            clock24h: true,
            clock_seconds: true,
            week_starts_monday: true,
            widgets: KNOWN_WIDGETS
                .iter()
                .map(|id| WidgetEntry { id: id.to_string(), enabled: true })
                .collect(),
            autostart: false,
            hide_taskbar: true,
        }
    }
}

impl Settings {
    /// Drop unknown or duplicate widgets and append any new ones.
    pub fn normalize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.widgets
            .retain(|w| KNOWN_WIDGETS.contains(&w.id.as_str()) && seen.insert(w.id.clone()));
        for id in KNOWN_WIDGETS {
            if !self.widgets.iter().any(|w| w.id == *id) {
                self.widgets.push(WidgetEntry { id: id.to_string(), enabled: true });
            }
        }
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
