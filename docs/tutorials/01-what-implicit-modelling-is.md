# 1. What implicit modelling is here, and when to use it

Anvil has two ways to describe a shape. This tutorial is about the
second one, and about knowing which to reach for.

## The two representations

**The history.** A Sketch, an Extrude, a Revolve. Each Feature hands the
next one a Body: planar facets with topology, faces you can pick, edges
you can measure. That is Anvil's B-rep. It is exact, it is what CAM and
inspection want, and it is what the part navigator shows.

**The field.** A function that answers one question at every point in
space: how far am I from the surface, and am I inside or outside?
Negative inside, positive outside, zero on the surface. That is the
`Field` trait in `crates/anvil-implicit/src/lib.rs`:

```
pub trait Field: Sync {
    fn at(&self, p: DVec3) -> f64;
    fn grad(&self, p: DVec3) -> DVec3 { /* central differences */ }
}
```

Everything else in the crate is a way of building or combining one of
those functions:

```
  Sphere, BoxField          shapes written in closed form
  Sampled                   a closed mesh turned into a distance grid
  Lattice, BeamLattice      periodic functions: gyroid, octet, kelvin
  Honeycomb                 hexagonal cell walls run through the part
  Union, Intersect, Subtract    min, max, and max with a negation
  SmoothUnion               min with a fillet of radius k built in
  Offset, Shell, Skin       add or subtract a constant from the value
  Graded                    thicken a centreline by a second field
  Ramp, Radial, Remap       scalar fields that drive a thickness
  Warp, CellRamp, Blend     change coordinates, cell size, or mix two fields
  GridField, PointMap       a field read from VTK or CSV data
  surface_nets              the field turned back into triangles
```

## Why booleans stop failing

A union of two fields is `min(a, b)`. There is no intersection curve to
compute, no sliver face to heal, and no ordering that makes it fail. A
fillet is a smooth minimum, which is arithmetic, not a rolling ball. A
lattice is a periodic function, so a million cells cost no more to
describe than one.

```
   B-rep union                     field union
   find the curve where            f(p) = min(a(p), b(p))
   A and B cross, split
   both, sew the halves,
   heal the slivers
```

The price is real. A field has no faces and no edges to pick, no exact
surfaces, and every export goes through a mesh at a voxel size you
choose. Anvil's Booleans, Fillet, Chamfer and Shell features return
`Unsupported` today (ADR 0001), which is exactly why the field path is
worth having.

## When to use which

Use the **history** when the part is prismatic, when downstream CAM or
inspection needs exact surfaces, when you will pick a face to place the
next Feature on, or when someone has to open the file in another CAD
system and edit it.

Use a **field** when:

* the geometry is periodic or there is far too much of it to model one
  feature at a time (lattices, ribs, infill);
* a dimension has to follow data (a stress map, a temperature, a
  distance to something else);
* the input is already a mesh or a voxel grid (an STL, a topology
  optimisation result);
* a boolean or a fillet in the history refuses to run.

The two mix. A Lattice fill Feature sits in the same history as the
Revolve it fills, reads its expressions from the same table, and hands a
Body to the Features after it. The field is a tool inside the history,
not a separate program.

## Check your build

```
cargo build -p anvil-cli --release
./target/release/anvil-cli --help
```

The `COMMANDS` block lists the field driven ones you will use in the
next seven tutorials:

```
    lattice  Fill a closed STL with a lattice under a skin and write the
             result as STL, 3MF, and STEP (the field driven tools).
    slice    Sample a closed STL into a distance field and write one SVG
             per layer straight from the field, plus layers.json with the
             area and perimeter of each layer.
    beams    Fill a closed STL with a beam lattice and write it as a 3MF
             beam lattice (the 3MF extension: nodes and beams with a
             radius, not triangles), clipped to the surface.
    run      Open a document, set named expressions from --set pairs or a
             JSON object, rebuild, export the bodies, and write a JSON
             report (expressions, feature notes, volumes, bounds, errors).
```

In the desktop app the same work lives on the **Solid** tab in the
**Field** group: Lattice fill, Density body, and Aircraft.

## Why it works

A signed distance function is closed under the operations designers
want. Min, max, addition of a constant, and composition with a change of
coordinates all take a distance-like function to another distance-like
function. The surface is wherever the value crosses zero, and surface
nets find those crossings on a grid. Nothing in that chain can fail on a
degenerate edge, because there are no edges until the very last step.

## Not yet available

* Fields have no pickable faces or edges, so Measure and face-placed
  features do not work on a lattice Body.
* An exact surface export. STEP from a field is a faceted solid, not
  analytic faces.
* A GPU evaluation path. Everything here is CPU, in parallel over the
  grid.

Next: [2. Fill a part with a TPMS lattice](02-tpms-lattice.md).
