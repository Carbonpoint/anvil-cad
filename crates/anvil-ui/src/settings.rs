//! UI scale and text size settings, opened from View > Settings.
//!
//! Persisted through eframe's own storage (see `AnvilApp::new` and
//! `eframe::App::save`), so the choice survives a restart without a
//! separate config file.

use serde::{Deserialize, Serialize};

/// Zoom factor range offered by the slider.
pub const MIN_SCALE: f32 = 0.7;
pub const MAX_SCALE: f32 = 2.0;

/// Base text size range offered by the slider (points, before scaling).
pub const MIN_TEXT: f32 = 10.0;
pub const MAX_TEXT: f32 = 22.0;

const DEFAULT_SCALE: f32 = 1.0;
/// Matches egui's own default `Body` size, so "Reset" looks like day one.
const DEFAULT_TEXT: f32 = 14.0;

/// UI zoom, base text size, and Material 3 theme choice, round-tripped
/// through serde for persistence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiSettings {
    pub scale: f32,
    pub base_text: f32,
    /// Light, Dark, or System (the default: follows the OS theme).
    pub theme: egui::ThemePreference,
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings { scale: DEFAULT_SCALE, base_text: DEFAULT_TEXT, theme: egui::ThemePreference::System }
    }
}

impl UiSettings {
    /// Key under which `AnvilApp` stores these in eframe's persistence file.
    pub const STORAGE_KEY: &'static str = "anvil_ui_settings";

    /// Applies the zoom factor and installs the Material 3 theme (colours,
    /// shape, and the type scale built from `base_text`), so a change
    /// takes effect the same frame. `AnvilApp` also calls
    /// `crate::theme::install` directly once per frame, so the theme keeps
    /// following a "System" preference even between settings changes;
    /// this method exists so scale and text size changes take effect
    /// immediately too, and for callers (including tests) that only have
    /// a `UiSettings` and a context at hand.
    pub fn apply(&self, ctx: &egui::Context) {
        ctx.set_zoom_factor(self.scale.clamp(MIN_SCALE, MAX_SCALE));
        crate::theme::install(ctx, self);
    }
}

/// Draws the Settings window. Returns true when a value changed and the
/// caller should call `UiSettings::apply` again.
pub fn window(ctx: &egui::Context, open: &mut bool, settings: &mut UiSettings) -> bool {
    let mut changed = false;
    egui::Window::new("Settings").collapsible(false).resizable(false).open(open).show(ctx, |ui| {
        ui.label("Theme");
        let before = settings.theme;
        settings.theme.radio_buttons(ui);
        changed |= settings.theme != before;
        ui.add_space(6.0);
        ui.label("UI scale");
        changed |= ui.add(egui::Slider::new(&mut settings.scale, MIN_SCALE..=MAX_SCALE).step_by(0.1)).changed();
        ui.add_space(6.0);
        ui.label("Text size");
        changed |= ui.add(egui::Slider::new(&mut settings.base_text, MIN_TEXT..=MAX_TEXT).step_by(1.0)).changed();
        ui.add_space(6.0);
        if ui.button("Reset").clicked() {
            *settings = UiSettings::default();
            changed = true;
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_serde() {
        let s = UiSettings { scale: 1.6, base_text: 18.0, theme: egui::ThemePreference::Dark };
        let json = serde_json::to_string(&s).unwrap();
        let back: UiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn default_round_trips() {
        let s = UiSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: UiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    /// Every theme choice round trips, not just the default (System).
    #[test]
    fn every_theme_choice_round_trips() {
        for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark, egui::ThemePreference::System] {
            let s = UiSettings { theme, ..UiSettings::default() };
            let json = serde_json::to_string(&s).unwrap();
            let back: UiSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }
}
