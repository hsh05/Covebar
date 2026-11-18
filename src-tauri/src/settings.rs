//! User settings, stored as JSON in the app config folder and edited from the settings window.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Widgets that exist in this build, in their default order. The list lives in
/// `src/lib/widget-ids.json`, shared with the frontend so the two can't drift apart.
pub fn known_widgets() -> &'static [String] {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/lib/widget-ids.json")).expect("widget-ids.json is a list of ids")
    })
}

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
    /// Clear blur behind a glassy, highlighted surface.
    Glass,
}

/// A floating pill around the widgets, or a strip across the whole screen.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Pill,
    Full,
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
    pub shape: Shape,
    pub clock24h: bool,
    pub clock_seconds: bool,
    pub week_starts_monday: bool,
    pub widgets: Vec<WidgetEntry>,
    pub autostart: bool,
    pub hide_taskbar: bool,
    /// Slide the bar away until the cursor touches its screen edge.
    pub auto_hide: bool,
    pub hotkeys: Hotkeys,
    /// Timer presets in seconds.
    pub timer_presets: Vec<u32>,
    /// Currency widget: converting from `base` to `quote`, plus favourites listed in its card.
    pub currency_base: String,
    pub currency_quote: String,
    pub currency_favorites: Vec<String>,
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
            shape: Shape::Pill,
            clock24h: true,
            clock_seconds: true,
            week_starts_monday: true,
            widgets: known_widgets()
                .iter()
                .map(|id| WidgetEntry { id: id.clone(), enabled: true })
                .collect(),
            autostart: false,
            hide_taskbar: true,
            auto_hide: false,
            hotkeys: Hotkeys::default(),
            timer_presets: vec![60, 300, 600, 1500, 3600],
            currency_base: "USD".into(),
            currency_quote: "AED".into(),
            currency_favorites: vec!["GBP".into()],
        }
    }
}

impl Settings {
    /// Drop unknown or duplicate widgets and append any new ones.
    pub fn normalize(&mut self) {
        // The stopwatch now lives in the timer widget: keep it on the bar if either was.
        if self.widgets.iter().any(|w| w.id == "stopwatch" && w.enabled) {
            for w in self.widgets.iter_mut().filter(|w| w.id == "timer") {
                w.enabled = true;
            }
        }
        let known = known_widgets();
        let mut seen = std::collections::HashSet::new();
        self.widgets.retain(|w| known.contains(&w.id) && seen.insert(w.id.clone()));
        // New widgets go after their nearest known predecessor in the default order.
        for (i, id) in known.iter().enumerate() {
            if self.widgets.iter().any(|w| w.id == *id) {
                continue;
            }
            let at = known[..i]
                .iter()
                .rev()
                .find_map(|prev| self.widgets.iter().position(|w| w.id == *prev))
                .map(|p| p + 1)
                .unwrap_or(0);
            self.widgets.insert(at, WidgetEntry { id: id.clone(), enabled: true });
        }
        self.timer_presets.retain(|s| (1..=86_400).contains(s));
        self.timer_presets.dedup();
        self.timer_presets.truncate(8);
        let code = |c: &mut String, fallback: &str| {
            *c = c.trim().to_ascii_uppercase();
            if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
                *c = fallback.into();
            }
        };
        code(&mut self.currency_base, "USD");
        code(&mut self.currency_quote, "AED");
        for f in self.currency_favorites.iter_mut() {
            *f = f.trim().to_ascii_uppercase();
        }
        self.currency_favorites.retain(|c| c.len() == 3 && c.chars().all(|ch| ch.is_ascii_alphabetic()));
        self.currency_favorites.dedup();
        self.currency_favorites.truncate(8);
    }

    /// The widget is on the bar.
    pub fn has_widget(&self, id: &str) -> bool {
        self.widgets.iter().any(|w| w.id == id && w.enabled)
    }

    /// Height of the visible bar in device-independent pixels.
    pub fn bar_dip(&self) -> f64 {
        match self.size {
            BarSize::Small => 48.0,
            BarSize::Large => 64.0,
        }
    }

    /// The dock window's thickness: the bar plus room for the pill to float, the same in both
    /// shapes so switching between them can animate. The extra strip is click-through.
    pub fn window_dip(&self) -> f64 {
        self.bar_dip() + 8.0
    }

    /// Screen space kept free of maximized windows: the full bar, or nothing for the
    /// pill (it floats over windows) or an auto-hiding bar.
    pub fn reserve_dip(&self) -> f64 {
        if self.shape == Shape::Pill || self.auto_hide { 0.0 } else { self.bar_dip() }
    }

    /// Window inner side to the visible bar.
    pub fn inset_dip(&self) -> f64 {
        if self.shape == Shape::Pill { 4.0 } else { 8.0 }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &Settings) -> Vec<&str> {
        s.widgets.iter().map(|w| w.id.as_str()).collect()
    }

    #[test]
    fn widget_list_is_valid() {
        let k = known_widgets();
        assert!(!k.is_empty());
        let unique: std::collections::HashSet<_> = k.iter().collect();
        assert_eq!(unique.len(), k.len(), "duplicate ids in widget-ids.json");
    }

    #[test]
    fn normalize_drops_unknown_and_duplicate_widgets() {
        let mut s = Settings::default();
        s.widgets.insert(0, WidgetEntry { id: "nope".into(), enabled: true });
        s.widgets.push(WidgetEntry { id: "clock".into(), enabled: false });
        s.normalize();
        assert!(!ids(&s).contains(&"nope"));
        assert_eq!(ids(&s).iter().filter(|i| **i == "clock").count(), 1);
        assert_eq!(s.widgets.len(), known_widgets().len());
    }

    #[test]
    fn normalize_inserts_new_widgets_after_their_predecessor() {
        let mut s = Settings::default();
        // A user who reordered things and whose saved list predates "currency".
        s.widgets.retain(|w| w.id != "currency");
        s.widgets.swap(0, 1);
        s.normalize();
        let order = ids(&s);
        let at = order.iter().position(|i| *i == "currency").unwrap();
        assert_eq!(order[at - 1], "recycle");
        assert_eq!(&order[..2], &["media", "apps"]);
    }

    #[test]
    fn stopwatch_on_the_bar_keeps_the_merged_timer_on() {
        let mut s = Settings::default();
        for w in s.widgets.iter_mut().filter(|w| w.id == "timer") {
            w.enabled = false;
        }
        s.widgets.push(WidgetEntry { id: "stopwatch".into(), enabled: true });
        s.normalize();
        assert!(s.has_widget("timer"));
        assert!(!ids(&s).contains(&"stopwatch"));
    }

    #[test]
    fn normalize_cleans_timer_presets_and_currencies() {
        let mut s = Settings::default();
        s.timer_presets = vec![0, 60, 60, 100_000, 300];
        s.currency_base = " eur ".into();
        s.currency_quote = "dollars".into();
        s.currency_favorites = vec!["gbp".into(), "GBP".into(), "x1".into()];
        s.normalize();
        assert_eq!(s.timer_presets, vec![60, 300]);
        assert_eq!(s.currency_base, "EUR");
        assert_eq!(s.currency_quote, "AED");
        assert_eq!(s.currency_favorites, vec!["GBP".to_string()]);
    }
}
