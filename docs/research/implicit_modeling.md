# Field driven design: an open nTop beside Anvil

Written 2026-09-17. This is the plan for the second product in this
repository: an implicit modeller for lattices, shells, and field driven
geometry, of the kind nTop sells, built open source on the same crates
as Anvil CAD.

## What nTop does that a B-rep CAD cannot

nTop represents a part as an implicit function: for every point in
space, a signed distance to the surface. Booleans become min and max,
offsets become subtraction, a fillet is a smooth minimum, and a lattice
is a periodic function. None of these operations can fail, none produce
sliver faces, and the cost does not grow with the number of features.
The price: the shape has no faces or edges to pick, no exact surfaces,
and every export is a mesh, so a field driven part is meshed at the end
at a chosen resolution.

Its strengths, in the order users pay for them:

1. Lattices: beam lattices on a unit cell, sheet lattices on a TPMS
   (gyroid, Schwarz, diamond, and others), conformal lattices that follow
   a surface, and graded cells whose size or wall follows a field.
2. Shells and infill: a skin of one thickness with a lattice core.
3. Field driven parameters: a wall thickness, a cell size, or a blend
   radius that varies with a scalar field from a simulation (stress,
   temperature) or from distance to a feature.
4. Topology optimization import: a density field from a solver becomes a
   smooth implicit body.
5. Robust booleans and blends between imported meshes and CAD bodies.
6. Meshing and export for printing and for FE: STL, 3MF, and slices
   straight from the field.

## What exists in the open

| Project | Language | Notes |
| --- | --- | --- |
| libfive | C++ with Scheme and Python bindings | Implicit kernel with an expression tree, meshing by dual contouring, an editor. The closest open cousin of nTop's kernel. |
| OpenVDB | C++ | Sparse voxel grids and level sets, used by every film studio. Booleans, offsets, meshing by marching cubes. Heavy build. |
| ImplicitCAD | Haskell | Implicit modeller with a script language. |
| Curv | C++ | Signed distance modelling language with a GPU renderer. |
| Fidget | Rust | JIT compiled implicit expressions with meshing. Active in 2026, from the libfive author. |
| OpenSCAD | C++ | CSG on meshes, not implicit. |

None of them has a feature history with editable parameters, a body
picked from a B-rep, or a print oriented workflow. That gap is where an
open field tool next to Anvil fits.

## The plan

The product is a crate, `anvil-implicit`, and the features that wrap it.
It shares the math, the expression parser, the feature history, the
document format, and the viewport with Anvil, so a lattice fill is a
feature in the same tree as the revolve it fills. A separate desktop
front end is a later decision; the ribbon gets a Field tab first.

Milestones:

* F1 (done 2026-09-17): the `Field` trait, sphere and box, union,
  intersection, subtraction, smooth union, offset, shell, skin, TPMS
  sheet lattices (gyroid, Schwarz P, diamond), a sampled field from any
  closed mesh (ray parity plus an exact distance transform), and surface
  nets meshing. The Lattice fill feature: skin plus lattice core on any
  body.
* F2 (done 2026-09-17): beam lattices on a unit cell (cubic, bcc, octet,
  kelvin) as the distance to a fixed list of segments; `Graded` thickens
  any centreline or mid sheet by a scalar field (`Ramp`, `Radial`, or
  any `Field`); `Warp` evaluates a field in mapped coordinates, with a
  cylindrical map so cells follow a round wall. Lattice fill exposes
  all of it, and `anvil-cli lattice` runs it on any closed STL.
* Aircraft demo (done 2026-09-17): `wing.rs` (NACA four digit airfoils
  with a blunt trailing edge, a lofted wing with taper, sweep, dihedral,
  twist, and section change, a Sears-Haack fuselage, rigid placement)
  and `aero.rs` (Prandtl lifting line, flat plate profile drag, Nelder
  Mead on taper and twist). The Aircraft feature builds the body and
  reports lift to drag, with an Optimise switch. Surface nets skip
  blocks far from the surface, a first step of F4.
* Research (2026-09-17): docs/research/ntop_capabilities.md,
  implicit_math.md, wing_aero_loop.md. Their priority list for the next
  steps: field from point map (CSV, VTK), headless parametric run with
  JSON in and out, boundary tagged mesh export, adaptive meshing with
  sharp features, topology optimisation density import, cell size
  grading and general warps, ribs from a projected graph, slices from
  the field, an OpenFOAM bridge, the 3MF beam extension, Fidget.
* F3 (first half done 2026-09-17): `PointMap` reads `x, y, z, value`
  CSV samples into a bucket grid with inverse distance weighting, and
  `Remap` turns any scalar into a thickness; Lattice fill's grade "map"
  uses them. `anvil-cli run` sets expressions from `--set` or a JSON
  file, rebuilds, exports, and writes a JSON report, the hook for a
  design loop. The TPMS fields now divide by their analytic gradient,
  so wall thickness is even across the cell. A vortex lattice solver
  sits beside the lifting line, `anvil-cli run --openfoam` writes a
  simpleFoam case for the visible bodies, and docs/examples/wing_loop.py
  is a scripted sweep and taper loop. Later the same day: `vtk.rs` reads
  legacy VTK grids and point scalars, `GridField` interpolates a grid,
  `Threshold` makes a body from a density, and the Density body feature
  wraps them (topology optimisation import). `CellRamp` grades a sheet
  lattice's cell size along an axis by the phase integral, and `Blend`
  mixes two fields by a weight field; Lattice fill exposes the ramp as
  Cell size at far end. Faces of an Aircraft body carry a component tag
  (`from_tagged_triangles`), and the OpenFOAM export writes one named
  STL solid and one patch per component (boundary tagged export).
  `Honeycomb` is the ribs pattern (hexagonal cell walls extruded along
  Z, the simplest of nTop's ribbing blocks), and Lattice fill's grade
  "distance" thickens by the sampled distance to a picked body. `slice.rs` cuts closed contours from any field by marching squares
  with bisected crossings; `anvil-cli slice` writes an SVG per layer and
  the layer areas and perimeters. `BeamLattice::beams_in` lists the beams inside a body clipped to its
  surface and `anvil-cli beams` writes them under the 3MF beam lattice
  extension. Still open: cell size grading for beam lattices, a better
  than first order sheet distance (a Newton step jumps between sheets),
  ribs projected onto a curved surface, a Fidget back end.
* F4 (first half done 2026-09-17): `Field::grad` (central differences by
  default; analytic for sphere, box, TPMS, beam lattices, and forwarded
  through the combinators), and surface nets place each cell vertex by
  the dual contouring quadratic error function of its crossings and
  their normals, with the crossings bisected where the field has a
  crease. Box corners land within 0.02 mm at a 0.5 mm step. Blocks far
  from the surface are skipped. Still open: an octree with interval
  pruning, the manifold clustering rule, smoothing and decimation.
* F5: Fidget as an optional back end for the expression tree, so a field
  built from primitives is compiled and evaluated on all cores, and a GPU
  path for the viewport.
* F6: print preparation from the field: slices at layer height directly
  from the field, overhang maps, and a 3MF with beam lattice extensions.

## What is exact and what is not

A sampled field is a distance accurate to one voxel; the lattice
functions are scaled level sets, close to a distance near the sheet.
Surface nets meshes are closed and stay within a voxel of the surface.
For a printed part at 0.4 mm resolution that is below the nozzle width.
For analysis or machining, Anvil's B-rep is still the reference.
