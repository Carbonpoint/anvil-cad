//! Material Design 3 (Material You) colour system for the desktop UI.
//!
//! One seed colour drives a light and a dark [`Scheme`]: tonal surfaces,
//! a primary role, a secondary container, outlines, and an error role,
//! following the M3 tone tables (surface at tone 98/6, primary at tone
//! 40/80, and so on). [`install`] turns a scheme into a full `egui::Style`
//! for both themes (colours, 8-16 px corner radii, and an M3 type scale),
//! hands them to the context with `set_style_of`, and sets the active
//! [`egui::ThemePreference`] so egui itself resolves Light, Dark, or
//! System every frame.
//!
//! Elevation is a surface tint (a higher "container" tone), not a heavy
//! drop shadow: window and menu shadows stay small. Hover and press use
//! M3's state layer opacities (8% and 12%) mixed over the resting colour.

use egui::style::{Selection, WidgetVisuals};
use egui::{Color32, CornerRadius, Shadow, Stroke, Style, Visuals};

/// Anvil's brand seed: a calm steel blue, `#3B6EA5`. Every tonal role
/// below is a tone of this hue (or, for the error role, a fixed red).
pub const SEED: Color32 = Color32::from_rgb(0x3b, 0x6e, 0xa5);

/// M3 hover state layer opacity.
const HOVER_LAYER: f32 = 0.08;
/// M3 pressed state layer opacity.
const PRESS_LAYER: f32 = 0.12;

/// Corner radius used by cards, panels, and most buttons.
const RADIUS_MD: u8 = 12;
/// Corner radius used by windows and dialogs, the most prominent shapes.
const RADIUS_LG: u8 = 16;
/// Corner radius used by menus and small chips.
const RADIUS_SM: u8 = 10;

/// A resolved M3 colour scheme: one set of role colours, already tuned
/// for either light or dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scheme {
    pub dark: bool,

    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,

    pub secondary_container: Color32,
    pub on_secondary_container: Color32,

    pub surface: Color32,
    pub surface_dim: Color32,
    pub surface_bright: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,

    pub outline: Color32,
    pub outline_variant: Color32,

    pub error: Color32,
    pub on_error: Color32,

    /// Not one of the eight roles the theme was asked for, but egui needs
    /// a warning colour distinct from `error`. A warm amber, independent
    /// of the seed hue so it reads the same in both schemes.
    pub warn: Color32,
}

impl Scheme {
    pub fn light() -> Self {
        Self::build(false)
    }

    pub fn dark() -> Self {
        Self::build(true)
    }

    fn build(dark: bool) -> Self {
        let (hue, chroma) = rgb_to_hue_sat(SEED);
        // M3's neutral and neutral-variant palettes share the primary hue
        // but drop almost all of its saturation, so surfaces read as
        // "grey with a hint of the brand colour" rather than as tinted.
        let neutral_s = (chroma * 0.12).clamp(0.03, 0.08);
        let neutral_variant_s = (chroma * 0.24).clamp(0.06, 0.14);
        const ERROR_HUE: f32 = 25.0;
        const ERROR_SAT: f32 = 0.65;
        const WARN_HUE: f32 = 45.0;
        const WARN_SAT: f32 = 0.65;

        let t = |hue: f32, sat: f32, tone: f32| tone_color(hue, sat, tone);

        let scheme = if dark {
            Scheme {
                dark: true,
                primary: t(hue, chroma, 80.0),
                on_primary: t(hue, chroma, 20.0),
                primary_container: t(hue, chroma, 30.0),
                on_primary_container: t(hue, chroma, 90.0),
                secondary_container: t(hue, neutral_variant_s, 30.0),
                on_secondary_container: t(hue, neutral_variant_s, 90.0),
                surface: t(hue, neutral_s, 6.0),
                surface_dim: t(hue, neutral_s, 6.0),
                surface_bright: t(hue, neutral_s, 24.0),
                surface_container_lowest: t(hue, neutral_s, 4.0),
                surface_container_low: t(hue, neutral_s, 10.0),
                surface_container: t(hue, neutral_s, 12.0),
                surface_container_high: t(hue, neutral_s, 17.0),
                surface_container_highest: t(hue, neutral_s, 22.0),
                on_surface: t(hue, neutral_variant_s, 90.0),
                on_surface_variant: t(hue, neutral_variant_s, 80.0),
                outline: t(hue, neutral_variant_s, 60.0),
                outline_variant: t(hue, neutral_variant_s, 30.0),
                error: t(ERROR_HUE, ERROR_SAT, 80.0),
                on_error: t(ERROR_HUE, ERROR_SAT, 20.0),
                warn: t(WARN_HUE, WARN_SAT, 75.0),
            }
        } else {
            Scheme {
                dark: false,
                primary: t(hue, chroma, 40.0),
                on_primary: t(hue, chroma, 100.0),
                primary_container: t(hue, chroma, 90.0),
                on_primary_container: t(hue, chroma, 10.0),
                secondary_container: t(hue, neutral_variant_s, 90.0),
                on_secondary_container: t(hue, neutral_variant_s, 10.0),
                surface: t(hue, neutral_s, 98.0),
                surface_dim: t(hue, neutral_s, 87.0),
                surface_bright: t(hue, neutral_s, 98.0),
                surface_container_lowest: t(hue, neutral_s, 100.0),
                surface_container_low: t(hue, neutral_s, 96.0),
                surface_container: t(hue, neutral_s, 94.0),
                surface_container_high: t(hue, neutral_s, 92.0),
                surface_container_highest: t(hue, neutral_s, 90.0),
                on_surface: t(hue, neutral_variant_s, 10.0),
                on_surface_variant: t(hue, neutral_variant_s, 30.0),
                outline: t(hue, neutral_variant_s, 50.0),
                outline_variant: t(hue, neutral_variant_s, 80.0),
                error: t(ERROR_HUE, ERROR_SAT, 40.0),
                on_error: t(ERROR_HUE, ERROR_SAT, 100.0),
                warn: t(WARN_HUE, WARN_SAT, 38.0),
            }
        };

        // A change to the seed colour or the tone table above must keep
        // body text readable: check the WCAG contrast right here rather
        // than trusting the tone table alone.
        debug_assert!(contrast_ratio(scheme.on_surface, scheme.surface) >= 4.5);
        debug_assert!(contrast_ratio(scheme.on_primary, scheme.primary) >= 4.5);
        scheme
    }

    /// A calm mid-tone version of the brand hue for the 3D view's default
    /// body colour: it does not follow the surface lightness (the model
    /// is lit, not a UI surface), so it stays legible in both schemes.
    pub fn model_body(&self) -> Color32 {
        let (hue, chroma) = rgb_to_hue_sat(SEED);
        tone_color(hue, (chroma * 0.45).clamp(0.12, 0.3), if self.dark { 58.0 } else { 63.0 })
    }
}

/// Blend `overlay` over `base` by `t` (0..1), in sRGB space. Used for M3
/// state layers: hover and press are the resting colour tinted toward
/// the interaction colour, not a separate flat fill.
pub fn mix(base: Color32, overlay: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| ((a as f32) * (1.0 - t) + (b as f32) * t).round() as u8;
    Color32::from_rgb(lerp(base.r(), overlay.r()), lerp(base.g(), overlay.g()), lerp(base.b(), overlay.b()))
}

/// A colour at a given hue, saturation, and M3 "tone" (0..100, HSL
/// lightness percent). This is an HSL approximation of M3's HCT tonal
/// palettes: close enough for a calm, high-contrast desktop tool, without
/// pulling in a full colour-appearance model.
fn tone_color(hue: f32, sat: f32, tone: f32) -> Color32 {
    hsl_to_rgb(hue, sat, tone / 100.0)
}

fn hsl_to_rgb(hue: f32, sat: f32, light: f32) -> Color32 {
    let s = sat.clamp(0.0, 1.0);
    let l = light.clamp(0.0, 1.0);
    if s <= 0.0001 {
        let v = (l * 255.0).round() as u8;
        return Color32::from_rgb(v, v, v);
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = hue.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to_u8 = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(to_u8(r1), to_u8(g1), to_u8(b1))
}

/// Hue (degrees) and an HSL-style saturation for a colour, used to pull
/// the seed's hue and chroma out once.
fn rgb_to_hue_sat(c: Color32) -> (f32, f32) {
    let r = c.r() as f32 / 255.0;
    let g = c.g() as f32 / 255.0;
    let b = c.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s)
}

fn srgb_channel(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of a colour.
pub fn relative_luminance(c: Color32) -> f64 {
    0.2126 * srgb_channel(c.r()) + 0.7152 * srgb_channel(c.g()) + 0.0722 * srgb_channel(c.b())
}

/// WCAG contrast ratio between two colours (1.0..21.0).
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// M3 type scale, expressed as a ratio of the user's base text size, so
/// the existing UI-scale settings keep working. `Button` (M3 label-large)
/// sits a touch under `Body` (M3 body-medium); `Heading` (M3 title-large)
/// is the biggest jump, for section titles such as "Part Navigator".
fn m3_text_styles(base_text: f32) -> std::collections::BTreeMap<egui::TextStyle, egui::FontId> {
    let base = base_text.clamp(crate::settings::MIN_TEXT, crate::settings::MAX_TEXT);
    [
        (egui::TextStyle::Small, egui::FontId::proportional(base * 0.72)),
        (egui::TextStyle::Body, egui::FontId::proportional(base * 0.93)),
        (egui::TextStyle::Button, egui::FontId::proportional(base * 0.88)),
        (egui::TextStyle::Heading, egui::FontId::proportional(base * 1.2)),
        (egui::TextStyle::Monospace, egui::FontId::monospace(base * 0.88)),
    ]
    .into_iter()
    .collect()
}

/// Build a full `egui::Style` (colours, shape, elevation, type scale) for
/// one scheme.
fn style_for(scheme: &Scheme, base_text: f32) -> Style {
    let mut visuals = if scheme.dark { Visuals::dark() } else { Visuals::light() };

    visuals.override_text_color = None;
    visuals.weak_text_color = Some(scheme.on_surface_variant);
    visuals.hyperlink_color = scheme.primary;
    visuals.faint_bg_color = scheme.surface_container;
    visuals.extreme_bg_color = scheme.surface_container_lowest;
    visuals.text_edit_bg_color = Some(scheme.surface_container_lowest);
    // A quiet tint for monospace/code-styled labels (expression values),
    // using the secondary container role so it reads as "data", not
    // chrome.
    visuals.code_bg_color = scheme.secondary_container;
    visuals.warn_fg_color = scheme.warn;
    visuals.error_fg_color = scheme.error;

    visuals.window_corner_radius = CornerRadius::same(RADIUS_LG);
    visuals.window_fill = scheme.surface_container_high;
    visuals.window_stroke = Stroke::new(1.0f32, scheme.outline_variant);
    // Elevation reads as a tint (the container tone above), so the shadow
    // stays small: a soft contact shadow, not a heavy drop shadow.
    visuals.window_shadow = Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(40) };
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_MD);
    visuals.panel_fill = scheme.surface;
    visuals.popup_shadow = Shadow { offset: [0, 1], blur: 6, spread: 0, color: Color32::from_black_alpha(35) };

    visuals.selection =
        Selection { bg_fill: scheme.primary_container, stroke: Stroke::new(1.0f32, scheme.on_primary_container) };

    visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: scheme.surface_container_low,
        weak_bg_fill: scheme.surface_container_low,
        bg_stroke: Stroke::new(1.0f32, scheme.outline_variant),
        corner_radius: CornerRadius::same(RADIUS_MD),
        fg_stroke: Stroke::new(1.0f32, scheme.on_surface),
        expansion: 0.0,
    };
    visuals.widgets.inactive = WidgetVisuals {
        bg_fill: scheme.surface_container_high,
        weak_bg_fill: scheme.surface_container_high,
        bg_stroke: Stroke::new(1.0f32, Color32::TRANSPARENT),
        corner_radius: CornerRadius::same(RADIUS_SM),
        fg_stroke: Stroke::new(1.0f32, scheme.on_surface_variant),
        expansion: 0.0,
    };
    visuals.widgets.hovered = WidgetVisuals {
        bg_fill: mix(scheme.surface_container_high, scheme.primary, HOVER_LAYER),
        weak_bg_fill: mix(scheme.surface_container_high, scheme.primary, HOVER_LAYER),
        bg_stroke: Stroke::new(1.0f32, scheme.primary),
        corner_radius: CornerRadius::same(RADIUS_SM),
        fg_stroke: Stroke::new(1.5f32, scheme.on_surface),
        expansion: 1.0,
    };
    visuals.widgets.active = WidgetVisuals {
        bg_fill: mix(scheme.surface_container_high, scheme.primary, PRESS_LAYER),
        weak_bg_fill: mix(scheme.surface_container_high, scheme.primary, PRESS_LAYER),
        bg_stroke: Stroke::new(1.0f32, scheme.primary),
        corner_radius: CornerRadius::same(RADIUS_SM),
        fg_stroke: Stroke::new(2.0f32, scheme.primary),
        expansion: 1.0,
    };
    visuals.widgets.open = WidgetVisuals {
        bg_fill: mix(scheme.surface_container_high, scheme.primary, HOVER_LAYER),
        weak_bg_fill: mix(scheme.surface_container_high, scheme.primary, HOVER_LAYER),
        bg_stroke: Stroke::new(1.0f32, scheme.outline),
        corner_radius: CornerRadius::same(RADIUS_SM),
        fg_stroke: Stroke::new(1.0f32, scheme.on_surface),
        expansion: 0.0,
    };
    visuals.text_cursor.stroke = Stroke::new(2.0f32, scheme.primary);

    let mut style = Style { visuals, ..Style::default() };
    style.text_styles = m3_text_styles(base_text);
    style
}

/// Install the M3 theme on `ctx` for this frame: builds a light and a
/// dark `Style` from [`Scheme::light`]/[`Scheme::dark`] and the settings'
/// base text size, hands both to the context, and sets the active
/// [`egui::ThemePreference`] from `settings.theme`. Returns the scheme
/// that is actually in effect this frame (resolving `System` against the
/// OS), so the caller can recolour the 3D viewport to match.
///
/// Cheap enough to call once per frame: no allocation beyond the two
/// small `Style` structs, and no image or texture work.
pub fn install(ctx: &egui::Context, settings: &crate::settings::UiSettings) -> Scheme {
    let light = Scheme::light();
    let dark = Scheme::dark();
    ctx.set_style_of(egui::Theme::Light, style_for(&light, settings.base_text));
    ctx.set_style_of(egui::Theme::Dark, style_for(&dark, settings.base_text));
    ctx.set_theme(settings.theme);
    match ctx.theme() {
        egui::Theme::Dark => dark,
        egui::Theme::Light => light,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_have_different_surfaces() {
        let light = Scheme::light();
        let dark = Scheme::dark();
        assert_ne!(light.surface, dark.surface);
        assert!(!light.dark);
        assert!(dark.dark);
        // The container tones should climb (light) or fall (dark) in
        // brightness through the ladder from lowest to highest.
        assert!(
            relative_luminance(light.surface_container_lowest) >= relative_luminance(light.surface_container_highest)
        );
        assert!(
            relative_luminance(dark.surface_container_lowest) <= relative_luminance(dark.surface_container_highest)
        );
    }

    #[test]
    fn body_text_meets_wcag_aa() {
        for scheme in [Scheme::light(), Scheme::dark()] {
            let pairs = [
                (scheme.on_surface, scheme.surface),
                (scheme.on_primary, scheme.primary),
                (scheme.on_primary_container, scheme.primary_container),
                (scheme.on_secondary_container, scheme.secondary_container),
                (scheme.on_error, scheme.error),
            ];
            for (on, base) in pairs {
                let ratio = contrast_ratio(on, base);
                assert!(ratio >= 4.5, "dark={} ratio={ratio:.2} on={on:?} base={base:?}", scheme.dark);
            }
        }
    }

    #[test]
    fn outline_is_visible_against_surface() {
        // Not body text, so not held to 4.5:1, but should still be
        // clearly distinguishable (M3's own target is 3:1 for UI parts).
        for scheme in [Scheme::light(), Scheme::dark()] {
            assert!(contrast_ratio(scheme.outline, scheme.surface) >= 3.0);
        }
    }

    #[test]
    fn mix_is_between_the_two_colours() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(200, 100, 50);
        let half = mix(a, b, 0.5);
        assert!(half.r() > a.r() && half.r() < b.r() || half.r() == 0);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
    }
}
