# Anvil CAD

Anvil is an open-source parametric CAD/CAM application written in Rust.
The long-term target is the working style of Siemens NX: a feature history,
named expressions that drive every dimension, constrained sketches, robust
solids, and an integrated CAM module. The short-term target is a small,
honest core that builds and runs on Linux, Windows, and macOS with no GPU.

Status: **early draft**, 2026-09-09. Read `docs/ROADMAP.md` before planning
work. Read `docs/ADDING_A_FEATURE.md` before adding a feature.

## What works today

* Workspace of nine crates. All build with stable Rust. 21 tests pass.
* Expressions: `h = w / 2 + 5`, dependency ordering, cycle detection.
* Sketch: points, lines, circles, arcs, 14 constraint types, a damped
  Newton (Levenberg-Marquardt) solver, closed-profile extraction.
* Kernel: planar-facet B-rep solid, parallel tessellation, extrude, revolve.
  Booleans and fillets return `Unsupported` (see ADR 0001).
* Features: Sketch, Extrude, Revolve, Fillet (stub). Ordered history with
  regeneration, per-feature errors, suppression, undo/redo, JSON save/load.
* CAM: neutral toolpath, 2.5D contour operation, generic RS-274 G-code post.
* IO: native `.anvil` JSON, binary STL export.
* GUI: ribbon built from the feature registry, part navigator, generic
  property panel, expression table, CPU-only software 3D viewport.

## Build and run

```
cargo run --release --bin anvil
```

Linux needs the usual GUI development packages (on Debian/Ubuntu:
`libgtk-3-dev libxkbcommon-dev libwayland-dev`). Windows and macOS need
nothing beyond a Rust toolchain.

Viewport controls: left drag orbits, middle or right drag pans, scroll zooms.
The status bar holds the file path used by Save, Open, STL, and G-code export.

## Layout

```
crates/anvil-math      shared math, tolerances, Plane, Axis, Aabb
crates/anvil-expr      expression parser and evaluation table
crates/anvil-sketch    2D entities, constraints, solver, profiles
crates/anvil-kernel    B-rep topology, tessellation, extrude, revolve
crates/anvil-feature   Feature trait, Document, registry, built-in features
crates/anvil-cam       tools, toolpaths, operations, post-processors
crates/anvil-io        .anvil documents and mesh export
crates/anvil-ui        ribbon, panels, viewport (eframe/egui)
crates/anvil-app       the `anvil` binary
docs/research          four literature reviews that shaped the design
docs/adr               architecture decision records
```

## Vocabulary

One word, one meaning, everywhere in code and docs:

* **Feature**: one step in the part history.
* **Document**: the ordered feature list plus the expression table.
* **Regenerate**: run every feature in order and rebuild all bodies.
* **Body**: one solid produced by the history.
* **Profile**: a closed loop taken from a solved sketch.
* **Descriptor**: static metadata that puts a feature on the ribbon.
* **Post**: a post-processor that turns a toolpath into controller text.

## License

MIT or Apache-2.0, at your option.
