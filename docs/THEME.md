# Theme: Material 3

Anvil's desktop UI follows Material Design 3 (Material You): one seed
colour, expanded into a tonal colour system, applied through egui's
`Style`/`Visuals`. All of it lives in `crates/anvil-ui/src/theme.rs`.

## Seed colour

`theme::SEED` is `#3B6EA5`, a calm steel blue. Every role below is a tone
of this hue (an HSL approximation of M3's tonal palettes), except the
error role, which uses a fixed red hue, and the warning role, a fixed
amber, so errors and warnings read the same regardless of the brand
colour.

## Roles

`theme::Scheme` holds the M3 roles used in the UI:

* `surface` and five container levels (`surface_container_lowest` through
  `surface_container_highest`), plus `surface_dim`/`surface_bright`:
  the backgrounds for panels, windows, and menus. Higher container
  levels sit visually "above" lower ones. This is how elevation reads,
  instead of a drop shadow: a window uses `surface_container_high`, a
  panel uses `surface` itself.
* `primary` / `on_primary`, `primary_container` / `on_primary_container`:
  the accent colour (hyperlinks, selection, the hover/press tint on
  buttons) and its container form (selected tabs and buttons).
* `secondary_container` / `on_secondary_container`: a quieter accent,
  used for monospace/code-styled backgrounds (expression values).
* `outline` / `outline_variant`: borders (window frames, dividers).
* `on_surface` / `on_surface_variant`: body text and secondary text.
* `error` / `on_error`: error messages and the error border/fill role.

Two extra tones exist because egui needs them and M3 does not assign a
role to either: `warn` (a fixed amber, distinct from `error`) and
`model_body()`, a mid-lightness tone of the seed hue used as the 3D
view's default part colour (it does not follow surface lightness,
because the model is lit, not a UI surface).

## Shape, elevation, and state

* Corner radius: 16px for windows/dialogs, 12px for panels and
  non-interactive frames, 10px for buttons, combo boxes, and menus.
* Elevation: a surface tint (a higher container tone), plus a small
  contact shadow on windows and popups; never a heavy drop shadow.
* Hover and press: the M3 state layer opacities, 8% and 12%, mixing the
  interaction colour (`primary`) over the resting surface colour
  (`theme::mix`).

## Type scale

`theme.rs` sets egui's five text styles as a ratio of the user's base
text size (so the existing UI-scale setting keeps working): `Small`
0.79x, `Body` 1.0x, `Button` 0.93x (M3's label-large sits a touch under
body text), `Heading` 1.43x, `Monospace` 0.95x.

## Light, dark, and system

`UiSettings.theme` (`crates/anvil-ui/src/settings.rs`) is an
`egui::ThemePreference`: `Light`, `Dark`, or `System` (the default).
Change it in View > Settings > Settings > Theme. `theme::install` runs
once per frame (from `AnvilApp::frame_ui_inner`): it builds both the
light and the dark `Style` and hands them to the context, then sets the
active preference, so egui itself resolves which one is showing,
including tracking a live OS theme change under System. The setting
persists the same way UI scale and text size do, through eframe's
storage.

## Changing the seed colour

Edit `theme::SEED` in `crates/anvil-ui/src/theme.rs`. Every role is
derived from it at `Scheme::build`, so nothing else needs to change. Run
`cargo test -p anvil-ui theme::` to check the new seed still gives
light and dark surfaces that differ and body text that clears 4.5:1
contrast against its surface (see the `tests` module in `theme.rs`).
