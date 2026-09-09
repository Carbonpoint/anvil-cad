# Roadmap

Ordered by dependency, not by date. Each milestone is usable on its own.

## M1: Draft (done 2026-09-09)

Workspace, expressions, sketch solver, planar kernel with extrude and
revolve, feature history with undo and save, CPU viewport, ribbon from
registry, STL export, contour G-code.

## M2: Interaction

* Viewport picking: CPU ray cast against the triangle mesh, then face and
  edge highlight. Face ids are already carried per triangle.
* Sketch editor: draw lines, rectangles, circles on a plane; add constraints
  and dimensions by clicking; live solve while dragging.
* Datum planes from faces. Sketch on a face.

## M3: Real solids

* Decide the kernel path per ADR 0001. The recommended path is an
  `OcctKernel` behind the `Kernel` trait, built with `opencascade-rs`,
  vendored so the build stays one `cargo build`.
* Booleans: unite, subtract, intersect. Extrude and Revolve gain a boolean
  mode against an existing body.
* Fillet and chamfer on selected edges.
* Persistent naming for faces and edges across regeneration. Do this in the
  same milestone as booleans. It is not a retrofit.
* STEP AP242 import and export through the same backend.

## M4: Workflow

* Patterns (linear, circular), mirror, shell, hole feature.
* Expressions panel with units, lists, and references to feature parameters.
* Part Navigator drag to reorder and rollback bar.
* Native format container per ADR 0002.
* Rhai scripting for journaling and design automation.

## M5: CAM

* Pocketing with contour-parallel and adaptive clearing (cavalier_contours
  for offsets).
* Drilling cycles with canned-cycle output on capable posts.
* 3-axis drop-cutter and waterline on tessellated bodies.
* Post-processor library: Fanuc, Haas, LinuxCNC, GRBL, Sinumerik.
* Stock simulation by voxel or per-layer 2D diff.

## M6: Performance and scale

* glow or wgpu viewport with the software path kept as the fallback.
* Incremental regeneration (only features downstream of the edit).
* Level of detail tessellation for assemblies.
* WASM plugin boundary with `wasmtime`.
