# Anvil implicit modelling tutorials

Eight worked tasks for the field driven side of Anvil: lattices, graded
walls, fields from measured or simulated data, and the mesh or slice
files that come out the other end. This is the kind of work people do in
tools such as nTopology, done with the crates in this repository.

Every command below was run before it was written down, and the output
quoted in each tutorial is the output those commands actually printed
from a release build on a multi core Linux workstation. Your triangle
counts and volumes should match; your times will differ.

## Before you start

Build the headless tool once:

```
cargo build -p anvil-cli --release
```

Run every command from the repository root, because the example
documents name their data files relative to it. Where a tutorial writes
`anvil-cli`, use `./target/release/anvil-cli` (or put it on your PATH).

Read the "Vocabulary" section of the top level `README.md` first. A
**Feature** is one step in the part history, a **Document** is the
feature list plus the expression table, a **Body** is one solid the
history produced, and **Regenerate** runs the whole list again. The
tutorials use those words and no others.

## The tutorials

| # | Tutorial | What you end up with |
| --- | --- | --- |
| 1 | [What implicit modelling is here](01-what-implicit-modelling-is.md) | A clear rule for when to reach for a field instead of a history feature |
| 2 | [Fill a part with a TPMS lattice](02-tpms-lattice.md) | A 60 x 40 x 20 mm plate at 48 percent of its solid volume, as STL, 3MF and STEP |
| 3 | [Beam lattices and the 3MF beam extension](03-beam-lattices.md) | The same plate as 996 octet beams on 637 nodes, 60 kB instead of 16.7 MB |
| 4 | [Grading: thickness and cell size](04-grading.md) | Walls that ramp along an axis, outward from the centre, or with distance to a boss |
| 5 | [Fields from data](05-fields-from-data.md) | A lattice thickened by a stress CSV, and a body from a topology optimisation density grid |
| 6 | [Meshing, slices and the speed knobs](06-meshing-and-output.md) | A resolution you chose on purpose, and SVG layers straight from the field |
| 7 | [A full part, start to export](07-a-full-part.md) | A ribbed bracket at 31.3 g and a wing rerun at a new taper |
| 8 | [The Fidget back end](08-fidget.md) | A JIT compiled field, and an honest list of what it cannot do yet |

## Keeping these honest

`crates/anvil-cli/tests/tutorials.rs` runs the commands from tutorials 2
to 7 against the same example documents with small sizes and coarse
voxels, so `cargo test --workspace` fails if a tutorial stops working.
It takes about half a minute. If you change a command here, change it
there.

## Example files

`docs/tutorials/files` holds the documents and data the tutorials use:

```
plate.anvil          a 60 x 40 x 20 mm box, the starting solid
plate_lattice.anvil  the same box with a gyroid Lattice fill on it
graded_cell.anvil    a bar whose gyroid cell size ramps along x
bracket.anvil        a plate with a boss and honeycomb ribs graded by distance
stress_plate.anvil   a plate whose cubic lattice thickens with stress.csv
stress.csv           x, y, z, stress point samples over the plate
density.anvil        a body from density.vtk
density.vtk          a legacy VTK density grid, a bar with a hole
```

Every one is small enough to read. Sizes, wall thicknesses and voxel
sizes are expressions, so `anvil-cli run --set name=value` changes them
without editing the file.
