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

/// UI zoom and base text size, round-tripped through serde for persistence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiSettings {
    pub scale: f32,
    pub base_text: f32,
    /// GPU viewport: `None` means "follow the machine", which is on when
    /// an OpenGL context is there and off when it is not.
    #[serde(default)]
    /// Draw the model with the GPU. `None` means off: the GPU path has
    /// not been seen running on a real driver yet, so it is opt in.
    pub gpu_viewport: Option<bool>,
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings { scale: DEFAULT_SCALE, base_text: DEFAULT_TEXT, gpu_viewport: None }
    }
}

impl UiSettings {
    /// Key under which `AnvilApp` stores these in eframe's persistence file.
    pub const STORAGE_KEY: &'static str = "anvil_ui_settings";

    /// Applies the zoom factor and rebuilds the text styles from
    /// `base_text`, so a change takes effect the same frame.
    pub fn apply(&self, ctx: &egui::Context) {
        ctx.set_zoom_factor(self.scale.clamp(MIN_SCALE, MAX_SCALE));
        let base = self.base_text.clamp(MIN_TEXT, MAX_TEXT);
        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (egui::TextStyle::Small, egui::FontId::proportional(base * 0.78)),
            (egui::TextStyle::Body, egui::FontId::proportional(base)),
            (egui::TextStyle::Button, egui::FontId::proportional(base)),
            (egui::TextStyle::Heading, egui::FontId::proportional(base * 1.45)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(base * 0.95)),
        ]
        .into_iter()
        .collect();
        ctx.set_style(style);
    }
}

/// Draws the Settings window. Returns true when a value changed and the
/// caller should call `UiSettings::apply` again.
pub fn window(ctx: &egui::Context, open: &mut bool, settings: &mut UiSettings) -> bool {
    let mut changed = false;
    egui::Window::new("Settings").collapsible(false).resizable(false).open(open).show(ctx, |ui| {
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
        let s = UiSettings { scale: 1.6, base_text: 18.0, gpu_viewport: Some(false) };
        let json = serde_json::to_string(&s).unwrap();
        let back: UiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    /// Settings written before the GPU viewport existed must still load.
    #[test]
    fn older_settings_without_the_gpu_field_load() {
        let s: UiSettings = serde_json::from_str(r#"{"scale":1.2,"base_text":15.0}"#).unwrap();
        assert_eq!(s.gpu_viewport, None);
        assert_eq!(s.scale, 1.2);
    }

    #[test]
    fn default_round_trips() {
        let s = UiSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: UiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
