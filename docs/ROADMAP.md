# Roadmap

Ordered by dependency, not by date. Each milestone is usable on its own.

## M1: Draft (done 2026-09-09)

Workspace, expressions, sketch solver, planar kernel with extrude and
revolve, feature history with undo and save, CPU viewport, ribbon from
registry, STL export, contour G-code.

## M2: Interaction (mostly done 2026-09-09)

Done: z-buffer viewport with id-buffer picking of bodies and faces; sketch
on a datum plane or a face; sketch editor with line, rectangle (2-point,
centre), circle (centre, 2-point, 3-point), arc (3-point, centre), polygon,
slot, point; 12 constraint tools and 3 dimension tools; drag with live
solve; primitives, sweep, loft, mirror, patterns, move, scale, offset and
angled planes; measure.

Open:
* Edge picking and per-edge fillet selection.
* Dimension values as expressions, and dimension labels you can drag.
* Trim, extend, offset, and sketch fillet tools.
* Sketch text and spline entities.
* Project face edges into a sketch.

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

## F: Field driven design (started 2026-09-17)

An implicit modeller beside the B-rep, in the spirit of nTop, open
source. The plan and milestones F1 to F6 are in
docs/research/implicit_modeling.md. F1 and F2 are done: the
`anvil-implicit` crate (fields, TPMS and beam lattices, graded
thickness, cylindrical warp, sampled fields from meshes, surface nets),
the Lattice fill feature on the Solid tab, `anvil-cli lattice`, and the
Aircraft feature (lofted NACA wing, fuselage, tail, lifting line
estimate, taper and twist optimisation). Three research reports in
docs/research set the order of the next steps; as of 2026-09-17 every
item on their list has a first cut: sharp meshing, VTK and density
import, cell size grading, tagged export, honeycomb ribs, distance
grading, slices from the field, 3MF beam lattices, and an optional
Fidget back end (`--features fidget`).
