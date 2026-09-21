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

/// Icon scale range offered by the slider: 1.0 is the size icons had
/// before the setting existed.
pub const MIN_ICONS: f32 = 0.8;
pub const MAX_ICONS: f32 = 2.0;

const DEFAULT_SCALE: f32 = 1.0;
/// Default icon scale. Bigger than 1.0 because the old 22 point icon was
/// hard to read on a high resolution laptop screen.
const DEFAULT_ICONS: f32 = 1.25;
/// Icon side in points at scale 1.0.
pub const BASE_ICON: f32 = 22.0;
/// Matches egui's own default `Body` size, so "Reset" looks like day one.
const DEFAULT_TEXT: f32 = 14.0;

/// UI zoom, base text size, and Material 3 theme choice, round-tripped
/// through serde for persistence.
/// Follow the operating system unless the user picks a side.
fn default_theme() -> egui::ThemePreference {
    egui::ThemePreference::System
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiSettings {
    pub scale: f32,
    pub base_text: f32,
    /// Draw the model with the GPU. `None` means off: the GPU path is
    /// opt in until it has been seen running on the machine.
    #[serde(default)]
    pub gpu_viewport: Option<bool>,
    /// Light, Dark, or System (the default: follows the OS theme).
    #[serde(default = "default_theme")]
    pub theme: egui::ThemePreference,
    /// Size of the ribbon icons, as a multiple of [`BASE_ICON`].
    #[serde(default = "default_icons")]
    pub icon_scale: f32,
}

/// Settings written before the icon size setting existed load with the
/// new default, so an old file still gets the bigger icons.
fn default_icons() -> f32 {
    DEFAULT_ICONS
}

/// Tell the context how much room the window has for the ribbon. A
/// short window (a laptop) shrinks the icon whatever the setting says,
/// because the 3D view comes first. Full size is reached at 900 points
/// of window height, and the shrink stops at 0.8.
pub fn set_icon_room(ctx: &egui::Context, window_height: f32) {
    let room = (window_height / 900.0).clamp(0.8, 1.0);
    ctx.data_mut(|d| d.insert_temp(icon_room_id(), room));
}

fn icon_room_id() -> egui::Id {
    egui::Id::new("anvil_icon_room")
}

/// Icon side in points for this frame, read back by `crate::icons`.
/// Falls back to [`BASE_ICON`] when no settings have been applied yet,
/// which keeps plain `egui::Context` tests working.
pub fn icon_size(ctx: &egui::Context) -> f32 {
    let scale: f32 = ctx.data(|d| d.get_temp(icon_scale_id())).unwrap_or(DEFAULT_ICONS);
    let room: f32 = ctx.data(|d| d.get_temp(icon_room_id())).unwrap_or(1.0);
    BASE_ICON * scale.clamp(MIN_ICONS, MAX_ICONS) * room
}

fn icon_scale_id() -> egui::Id {
    egui::Id::new("anvil_icon_scale")
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings {
            scale: DEFAULT_SCALE,
            base_text: DEFAULT_TEXT,
            gpu_viewport: None,
            theme: default_theme(),
            icon_scale: DEFAULT_ICONS,
        }
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
        ctx.data_mut(|d| d.insert_temp(icon_scale_id(), self.icon_scale.clamp(MIN_ICONS, MAX_ICONS)));
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
        ui.label("Icon size");
        changed |= ui
            .add(egui::Slider::new(&mut settings.icon_scale, MIN_ICONS..=MAX_ICONS).step_by(0.05).suffix("x"))
            .on_hover_text("Size of the ribbon icons")
            .changed();
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
        let s = UiSettings {
            scale: 1.6,
            base_text: 18.0,
            gpu_viewport: Some(false),
            theme: egui::ThemePreference::Dark,
            icon_scale: 1.5,
        };
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
        assert_eq!(s.icon_scale, DEFAULT_ICONS);
    }

    /// Icons are bigger than they used to be, and the setting reaches
    /// `icons::button_core` through the context.
    #[test]
    fn the_icon_size_follows_the_setting() {
        const { assert!(DEFAULT_ICONS > 1.0, "the default icon is meant to be bigger than the old 22 points") };
        let ctx = egui::Context::default();
        assert_eq!(icon_size(&ctx), BASE_ICON * DEFAULT_ICONS);
        UiSettings { icon_scale: 2.0, ..UiSettings::default() }.apply(&ctx);
        assert_eq!(icon_size(&ctx), BASE_ICON * 2.0);
        UiSettings { icon_scale: 9.0, ..UiSettings::default() }.apply(&ctx);
        assert_eq!(icon_size(&ctx), BASE_ICON * MAX_ICONS, "out of range values are clamped");
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
