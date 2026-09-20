# 2. Fill a part with a TPMS lattice

Goal: take a solid plate and replace its inside with a gyroid sheet
under a solid skin, twice: once from an STL, once from a Document body.
Then check the result is printable.

A TPMS is a triply periodic minimal surface. Anvil has three of them:
gyroid, schwarz (Schwarz P) and diamond. A sheet lattice is that surface
thickened to a wall, repeating every cell in all three directions.

Time: about ten minutes. Everything runs from the repository root.

## Step 1: make the starting solid

`docs/tutorials/files/plate.anvil` is one Box Feature driven by three
expressions.

```
./target/release/anvil-cli run docs/tutorials/files/plate.anvil \
    --formats stl --out out/plate
```

You should see:

```
Wrote out/plate/plate.stl (STL, 1 features, 12 triangles)
Wrote out/plate/report.json (1 features, 0 errors)
```

Twelve triangles is a box: six faces of two triangles. Check the volume
in the report:

```
python3 -c "import json;print(json.load(open('out/plate/report.json'))['features'][0]['bodies'][0]['volume_mm3'])"
```

`48000` mm3, which is 60 x 40 x 20. Hold on to that number.

## Step 2: fill the STL with a gyroid

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind gyroid --cell 10 --wall 1.2 --skin 1.2 --res 0.5 \
    --out out/gyroid
```

```
Lattice fill (gyroid 10 mm): gyroid lattice, 372108 triangles, 48 percent of the solid volume, 498456 voxels
Wrote out/gyroid/plate_lattice.stl (STL, 1 features, 372108 triangles)
Wrote out/gyroid/plate_lattice.3mf (3MF, one object per feature, 1 features, 372108 triangles)
Wrote out/gyroid/plate_lattice.step (STEP, 1 faceted solids)
```

Read the note left to right. The cell repeats every 10 mm, so the plate
holds 6 x 4 x 2 cells. The wall is 1.2 mm of sheet and the skin is 1.2
mm of solid under the outside surface. The result is 48 percent of the
solid volume, so the plate lost a little over half its mass. 498456
voxels is the sampling grid: 60/0.5 by 40/0.5 by 20/0.5 plus the three
voxel pad on each side.

Change one thing at a time and watch the percentage:

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl --kind schwarz \
    --cell 10 --wall 1.2 --skin 1.2 --res 0.5 --out out/schwarz
./target/release/anvil-cli lattice --stl out/plate/plate.stl --kind diamond \
    --cell 10 --wall 1.2 --skin 1.2 --res 0.5 --out out/diamond
```

| Kind | Triangles | Percent of solid |
| --- | --- | --- |
| gyroid | 372108 | 48 |
| schwarz | 329464 | 44 |
| diamond | 428652 | 54 |

Same cell, same wall, same skin: diamond packs more surface into a cell
than Schwarz P does, so it keeps more material. Pick by stiffness and
printability, not by the picture.

## Step 3: fill a Document body instead

The STL route throws away the history. `plate_lattice.anvil` keeps it: a
Box Feature and a Lattice fill Feature on it, with the material set to
PLA so the report can give a mass.

```
./target/release/anvil-cli run docs/tutorials/files/plate_lattice.anvil \
    --formats 3mf,stl --out out/pl
```

```
Wrote out/pl/plate_lattice.3mf (3MF, one object per feature, 1 features, 372108 triangles)
Wrote out/pl/plate_lattice.stl (STL, 1 features, 372108 triangles)
Wrote out/pl/report.json (2 features, 0 errors)
```

372108 triangles, the same as step 2: the same field, sampled the same
way. Now read the numbers the STL route could not give you:

```
python3 -c "
import json
r = json.load(open('out/pl/report.json'))
for f in r['features']:
    print(f['index'], f['kind'], round(f['bodies'][0]['volume_mm3'], 1), f['mass_g'] and round(f['mass_g'], 2), f['bodies'][0]['open_edges'])
"
```

```
0 box 48000.0 59.52 0
1 lattice_fill 23267.9 28.85 0
```

The numbers to check:

* **Volume** 23267.9 mm3 against 48000 mm3 solid: 48.5 percent, which is
  the 48 the note rounded to.
* **Mass** 28.85 g of PLA against 59.52 g solid. You saved 30.7 g.
* **Open edges** 0. The Body is closed, so a slicer will take it.

Only one Body is visible at the end (`visible_bodies` is 1): Lattice
fill consumes the Box it filled, as a moved Body replaces its source.

## Step 4: change the numbers without editing the file

Every dimension in the document is an expression, so drive them from the
command line:

```
./target/release/anvil-cli run docs/tutorials/files/plate_lattice.anvil \
    --set cell=6 --set wall=1.0 --formats stl --out out/pl6
```

```
Wrote out/pl6/plate_lattice.stl (STL, 1 features, 514632 triangles)
Wrote out/pl6/report.json (2 features, 0 errors)
```

The note reads `gyroid lattice, 514632 triangles, 57 percent of the
solid volume`. Smaller cells mean more sheet area, so a 6 mm cell with a
1.0 mm wall is heavier (57 percent) than a 10 mm cell with a 1.2 mm wall
(48 percent), not lighter. Cell size and wall thickness pull in opposite
directions; always read the percentage back.

## Step 5: check printability

Two limits matter, and they are different limits.

**The nozzle.** A 1.2 mm wall prints fine with a 0.4 mm nozzle: three
passes. Below about 0.8 mm you are asking one perimeter to carry the
part. Set the wall from your nozzle, not from the picture.

**The voxel.** The field is sampled at `--res`, so a wall thinner than
two voxels cannot be resolved. Anvil refuses rather than giving you a
lattice full of holes:

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind gyroid --cell 10 --wall 0.6 --res 0.5 --out out/thin
```

```
error: feature 1 (Lattice fill (gyroid 10 mm)): wall 0.6 is thinner than two voxels; lower the resolution to 0.30 or less
```

Take the advice or thicken the wall. Tutorial 6 covers what dropping the
resolution costs you.

**The sheet thickness rule.** Keep the wall under about a fifth of the
cell. The sheet distance is a first order estimate of the distance to
the surface, so a thick wall in a small cell comes out thinner than you
asked for. At cell 10 that means a wall up to about 2 mm.

## Why it works

The plate is sampled into a signed distance grid by ray parity plus an
exact distance transform, which gives a field that is negative inside
the plate. The gyroid is a periodic function divided by its analytic
gradient, so its value is close to a true distance near the sheet and
thickening it by 1.2 mm really gives a 1.2 mm wall. The two are combined
as

```
    inside the core    Intersect(Offset(plate, -skin), lattice)
    plus the skin      Union(that, Skin(plate, skin))
```

and surface nets meshes wherever the combined value crosses zero. No
B-rep boolean runs anywhere in that, which is why it works on a mesh
input and why the cost depends only on the grid size and not on how many
cells there are.

## Not yet available

* Conformal lattices that follow a curved surface in general. `--conform
  cylinder` wraps cells around the Z axis, and that is the only map.
* A lattice inside a picked region of a Body. Lattice fill takes a whole
  Body.

Next: [3. Beam lattices and the 3MF beam extension](03-beam-lattices.md).
