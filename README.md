# Anvil CAD

Anvil is an open-source parametric CAD/CAM application written in Rust.
The long-term target is the working style of Siemens NX: a feature history,
named expressions that drive every dimension, constrained sketches, robust
solids, and an integrated CAM module. The short-term target is a small,
honest core that builds and runs on Linux, Windows, and macOS with no GPU.

Status: **early draft**, updated 2026-09-09. Read `docs/ROADMAP.md` before
planning work. Read `docs/ADDING_A_FEATURE.md` before adding a feature.
Read `docs/USING_ANVIL.md` for the sketch workflow.

## What works today

* Workspace of nine crates. All build with stable Rust. 35 tests pass.
* Expressions: `h = w / 2 + 5`, dependency ordering, cycle detection.
* Sketch: points, lines, circles, arcs, 20 constraint types, a damped
  Newton (Levenberg-Marquardt) solver, closed-profile and open-path
  extraction. Builders for rectangles (2-point, centre), circles (centre,
  2-point, 3-point), arcs (3-point, centre), polygons, slots.
* Sketch editor: pick a datum plane or click a face to start; draw with the
  tools above; select entities and apply constraints or dimensions; drag
  points with live solving; delete; construction lines; grid snap.
* Kernel: planar-facet B-rep solid, parallel tessellation, extrude,
  revolve, sweep, loft, box, cylinder, sphere, torus, mirror, transforms,
  feature-edge extraction. Booleans, fillet, chamfer, shell return
  `Unsupported` (see ADR 0001).
* Features on the Solid tab. Create: Sketch, Extrude, Revolve, Sweep, Loft,
  Hole (pending), Box, Cylinder, Sphere, Torus, Mirror, Rectangular pattern,
  Circular pattern. Modify: Fillet, Chamfer, Shell, Combine (all pending
  kernel support), Move/Copy, Scale. Construct: Offset plane, Plane at
  angle. Inspect: Measure.
* History: regeneration, per-feature errors, suppression, body consumption
  (a moved body replaces its source), undo/redo, JSON save/load.
* CAM: neutral toolpath, 2.5D contour operation, generic RS-274 G-code post.
* IO: native `.anvil` JSON, binary STL export.
* GUI: ribbon built from the feature registry, contextual Sketch tab, part
  navigator, generic property panel, expression table, CPU-only viewport
  with a depth buffer, model edges, hover and click selection of bodies.

## Build and run

```
cargo run --release --bin anvil
```

Linux needs the usual GUI development packages (on Debian/Ubuntu:
`libgtk-3-dev libxkbcommon-dev libwayland-dev`). Windows and macOS need
nothing beyond a Rust toolchain.

Viewport controls: left drag orbits, middle or right drag pans, scroll zooms,
click selects a body. In sketch mode left click draws, right drag pans.
The status bar holds the file path used by Save, Open, STL, and G-code export.

On WSL the app uses X11 by default. If no window appears, WSLg's X11 socket
link is missing. Run once:

```
sudo rm -rf /tmp/.X11-unix && sudo ln -s /mnt/wslg/.X11-unix /tmp/.X11-unix
```

## Run in Docker

The container holds the headless command-line tool, `anvil-cli`. It needs
no Rust install and no display. The desktop app is not in the image.

```
docker build -t anvil-cad .
docker run --rm -v "$PWD/out:/out" anvil-cad card --name "Your Name" --url "https://www.linkedin.com/in/your-handle"
```

On Windows PowerShell, use `-v "${PWD}\out:/out"`. The card files appear in
the `out` folder. `docker run --rm anvil-cad fonts` lists the fonts, and
`docker build --target test .` runs the tests that need no display.
`docker run --rm -v "$PWD/out:/out" anvil-cad kettle --variant gated --out /out`
writes the kettle sample with its gating, one STL per part, and a Truchas
case; `--variant mold` writes the pattern halves, core, and core box.

## Layout

```
crates/anvil-math      shared math, tolerances, Plane, Axis, Aabb
crates/anvil-expr      expression parser and evaluation table
crates/anvil-sketch    2D entities, constraints, solver, profiles
crates/anvil-kernel    B-rep topology, tessellation, extrude, revolve
crates/anvil-feature   Feature trait, Document, registry, built-in features
crates/anvil-implicit  implicit modelling: fields, TPMS lattices, sampled fields, surface nets
crates/anvil-cam       tools, toolpaths, operations, post-processors
crates/anvil-io        .anvil documents and mesh export
crates/anvil-ui        ribbon, panels, viewport (eframe/egui)
crates/anvil-app       the `anvil` binary
crates/anvil-cli       headless command-line tools (used by the Docker image)
docs/KETTLE.md         cast iron kettle sample: Pattern on face, mold split, casting check
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
* **Region**: an outer profile with the profiles directly inside it as holes.
  Extrude and Revolve use the regions you click in the view.
* **Descriptor**: static metadata that puts a feature on the ribbon.
* **Post**: a post-processor that turns a toolpath into controller text.

## License

MIT or Apache-2.0, at your option.
