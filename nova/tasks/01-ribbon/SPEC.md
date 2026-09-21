# Task 1: one row of ribbon panels, like Fusion

## The problem

The Solid tab draws every button of every group in one wrapped block.
At 1366 points wide that is three rows, and the last group still runs
off the right edge. The user comes from Fusion 360 and wants the layout
Fusion uses.

## What Fusion does

One row per tab. The row holds panels. A panel shows a few icons with
no text under them. Under the icons sits the panel name with a small
triangle. That name is a menu, and the menu lists **every** command of
the panel, with its keyboard shortcut on the right. Fusion's Design
workspace has the tabs SOLID, SURFACE, MESH, SHEET METAL, PLASTIC,
MANAGE, UTILITIES, and the SOLID tab has the panels CREATE, MODIFY,
CONFIGURE, CONSTRUCT, INSPECT, INSERT, ASSEMBLE, SELECT.

Anvil copies the shape, not the command list.

## What to build

### 1. Ribbon data

In `crates/anvil-ui/src/ribbon.rs`:

* `RibbonButton` gains two fields:
  * `pub pinned: bool` - true when the command shows as an icon in the row.
  * `pub shortcut: Option<&'static str>` - for example `Some("E")`.
* Two tables in `ribbon.rs` decide both. A command not named in them is
  in the panel menu only, with no shortcut:

```rust
/// Commands pinned as icons, panel by panel, in the order shown.
const PINNED: &[(&str, &str, &[&str])] = &[
    // (tab, panel, ids in order)
    ("Solid", "Create", &["sketch", "extrude", "revolve", "hole"]),
    ...
];
/// Keyboard shortcuts, as the menus show them.
const SHORTCUTS: &[(&str, &str)] = &[("extrude", "E"), ("hole", "H"), ...];
```

* `build_ribbon()` fills `pinned` and `shortcut` from those tables.
* A feature that no table names still reaches the ribbon, in its panel's
  menu. Adding a feature must still need no edit to the ribbon, which is
  a rule of this repository (`CLAUDE.md`).
* At most **four** pinned commands per panel.
* `pub fn shortcut_of(id: &str) -> Option<&'static str>` is public, so
  the menus and the status bar can show the same letter.
* `mod ribbon` becomes `pub mod ribbon` in `crates/anvil-ui/src/lib.rs`,
  and `RibbonTab`, `RibbonGroup`, `RibbonButton`, `ButtonKind`,
  `RibbonAction` and `build_ribbon` are public. This is what the test
  file reads.

### 2. Tabs and panels

The tab order becomes:

    File, Solid, Field, Inspect, View, Examples

* **Solid** keeps the panels Create, Modify, Construct, Insert, Select.
* **Field** takes the implicit commands (Lattice fill, Aircraft, Density
  body) that are in the Solid tab's Field group today.
* **Inspect** takes Measure, Interference, Section analysis, Center of
  Mass and Bill of Materials, which are the Solid tab's Inspect and
  Manage groups today.
* **Select** is a new panel on the Solid tab. It holds the selection
  filter that lives in the status bar today, as four commands labelled
  All, Body, Face and Edge. Add one action for them:
  `RibbonAction::SetSelectFilter(u8)`, where 0 is All, 1 Body, 2 Face
  and 3 Edge, and `run_action` sets `AnvilApp::filter`. The status bar
  keeps showing which filter is on. None of the four is pinned.
* Nothing is lost. Every command that is on the ribbon today is on the
  ribbon afterwards, in exactly one panel.

Move a feature by changing the `tab` and `group` fields of its
`FeatureDescriptor`, where the feature declares itself. Do not build a
second table of feature names.

### 3. Drawing

In `crates/anvil-ui/src/app.rs`, `ribbon_ui`:

* One `ui.horizontal` row, not `horizontal_wrapped`.
* Each panel draws its pinned commands with `icons::icon_only`, then,
  under them, a `menu_button` whose text is the panel name.
* The menu lists every command of the panel: icon, label, and the
  shortcut letter right aligned and weak.
* A separator between panels, as now.
* The whole row is inside `egui::ScrollArea::horizontal()`, so a narrow
  window scrolls instead of clipping.

### 4. Shortcuts

The letters in `SHORTCUTS` are the ones the application already obeys:
E extrude, H hole, Q press pull, F fillet, D dimension, L line, R
rectangle, C circle. Show them. Do not invent new key handling in this
task.

## The test file

`crates/anvil-ui/tests/ribbon_panels.rs` is copied in before you start.
Do not edit it. Do not add `#[ignore]`. Make it pass.

## Done means

* `cargo fmt --all --check`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace`
* `cargo test -p anvil-ui --features devtools`
* `scripts/ui_shots.sh` runs and the pictures it writes show one row.

## Do not

* Do not change the sketch ribbon (`sketch_ribbon`). It is contextual
  and already fits.
* Do not remove a command to make the row fit.
* Do not use an em dash anywhere, in code, comments or documents.
