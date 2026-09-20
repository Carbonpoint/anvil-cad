# 4. Grading: thickness and cell size

Goal: make the lattice heavier where the part needs it and lighter where
it does not. Four graders, in order of how much you have to set up:
along an axis, outward from the centre, by distance to another Body, and
the cell size itself.

You need `out/plate/plate.stl` from tutorial 2 step 1.

## Step 1: a baseline to compare against

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind octet --cell 10 --wall 1.0 --skin 1.0 --res 0.5 --out out/g2/n
```

```
Lattice fill (octet 10 mm): octet lattice, 342148 triangles, 27 percent of the solid volume, 498456 voxels
```

27 percent, a 1.0 mm beam everywhere.

## Step 2: ramp the thickness along an axis

`--grade z` ramps from `--wall` at the bottom of the Body's bounding box
to `--wall-end` at the top. Use it when the load comes in at one face.

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind octet --cell 10 --wall 1.0 --wall-end 2.5 --grade z \
    --skin 1.0 --res 0.5 --out out/g2/z
```

```
Lattice fill (octet 10 mm): octet graded along z, 422420 triangles, 45 percent of the solid volume, 498456 voxels
```

27 percent becomes 45. The note now says "graded along z" instead of
"lattice", which is how you confirm the grade took. `x` and `y` work the
same way, from the low face to the high face.

```
   z = 20 mm   ======  2.5 mm beams
               \/\/\/
   z = 10 mm   /\/\/\  1.75 mm beams
               \/\/\/
   z = 0 mm    ------  1.0 mm beams
```

## Step 3: ramp outward from the centre

`--grade radial` ramps from `--wall` at the centre of the bounding box
to `--wall-end` at its outside. Use it for a part loaded on its skin, or
against one loaded through the middle by swapping the two numbers.

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind gyroid --cell 10 --wall 1.0 --wall-end 2.5 --grade radial \
    --skin 1.0 --res 0.5 --out out/g2/r
```

```
Lattice fill (gyroid 10 mm): gyroid graded along radial, 420428 triangles, 59 percent of the solid volume, 498456 voxels
```

The radius of the ramp is the largest half extent of the bounding box,
30 mm on this plate, so anything 30 mm or further from the centre gets
the full 2.5 mm and the middle of the 40 mm and 20 mm faces does not.

## Step 4: grade by distance to another Body

This is the useful one, and it needs a Document rather than the command
line, because it refers to a second Body.
`docs/tutorials/files/bracket.anvil` has three Features:

```
  0  box       60 x 40 x 10 mm plate
  1  cylinder  a 12 mm boss at (48, 20), 18 mm tall
  2  lattice_fill  honeycomb ribs in body 0, graded by distance to body 1
```

The grade parameters read: `wall` is the thickness **at** the reference
Body's surface, `wall_end` is the thickness at `map_hi` millimetres away
and beyond. So thick ribs at the boss and thin ribs far from it mean
`wall` 2.4 and `wall_end` 0.8, not the other way round.

```
./target/release/anvil-cli run docs/tutorials/files/bracket.anvil \
    --formats stl,3mf --out out/g/bracket
```

```
Wrote out/g/bracket/bracket.stl (STL, 2 features, 285056 triangles)
Wrote out/g/bracket/bracket.3mf (3MF, one object per feature, 2 features, 285056 triangles)
Wrote out/g/bracket/report.json (3 features, 0 errors)
```

```
python3 -c "
import json
r = json.load(open('out/g/bracket/report.json'))
f = r['features'][2]
print(f['note'])
print(round(f['bodies'][0]['volume_mm3'], 1), 'mm3', round(f['mass_g'], 2), 'g', f['bodies'][0]['open_edges'], 'open edges')
"
```

```
honeycomb thickness by distance to feature 1, 287876 triangles, 48 percent of the solid volume, 517452 voxels
11593.5 mm3 31.3 g 0 open edges
```

The numbers to check: the plate is 60 x 40 x 10, so 24000 mm3 solid; the
ribbed Body is 11593.5 mm3, the 48 percent the note gives. At aluminium
6061 (2.70 g/cm3) that is 31.3 g. Open edges 0, so it is a closed Body.

Flip the grade to see it is really doing something:

```
./target/release/anvil-cli run docs/tutorials/files/bracket.anvil \
    --set wall=0.8 --set wall_end=2.4 --formats stl --out out/g/flip
```

```
honeycomb thickness by distance to feature 1, 283008 triangles, 52 percent of the solid volume, 517452 voxels
12439.3 mm3 33.59 g
```

Thin at the boss, thick at the far corner: 52 percent instead of 48, so
a heavier part that is weak exactly where the bolt is. That is the
mistake the parameter order makes easy, which is why it is worth running
once.

`reach` (30 mm here) is how far the ramp takes to get from `wall` to
`wall_end`. Beyond it the thickness stays at `wall_end`.

## Step 5: grade the cell size

Thickness is not the only lever. A sheet lattice can change its cell
size along an axis, which changes the surface area per unit volume as
well as the mass. `docs/tutorials/files/graded_cell.anvil` ramps a
gyroid from a 5 mm cell at x = 0 to a 15 mm cell at x = 60.

```
./target/release/anvil-cli run docs/tutorials/files/graded_cell.anvil \
    --formats stl --out out/g/cell
```

```
python3 -c "
import json
r = json.load(open('out/g/cell/report.json'))
print(r['features'][1]['note'])
print(round(r['features'][1]['bodies'][0]['volume_mm3'], 1), 'of', round(r['features'][0]['bodies'][0]['volume_mm3'], 1))
"
```

```
gyroid graded along x, cell 5 to 15, 193528 triangles, 31 percent of the solid volume, 266616 voxels
7411.1 of 24000.0
```

```
   x = 0                                      x = 60
   |||||||||  |  |  |  |    |     |     |     |
   5 mm cells                       15 mm cells
```

The command line has no flag for this: `--cell` sets the near end and
there is no `--cell-end`, so cell grading is a Document parameter
(`cell_end`) or a field in the property panel. It needs `grade` set to
`x`, `y` or `z`, and it works on gyroid, schwarz and diamond only.

## Step 6: conform cells to a round wall

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind cubic --cell 10 --wall 1.2 --conform cylinder \
    --skin 1.0 --res 0.5 --out out/g2/c
```

```
Lattice fill (cubic 10 mm): cubic lattice, 163032 triangles, 19 percent of the solid volume, 498456 voxels
```

The cells are now evaluated in cylindrical coordinates about the Z axis,
so they wrap around a round wall instead of being cut by it. Note the
note does not mention the conform: check the shape, not the text.

## In the desktop app

Lattice fill is on the **Solid** tab in the **Field** group. The
property panel carries the same parameters under these labels:

```
  Lattice                         gyroid ... honeycomb
  Cell size / Wall thickness      cell, wall
  Thickness at far end (0 = same) wall_end
  Cell size at far end (0 = same) cell_end
  Grade along                     none, x, y, z, radial, map, distance
  Distance to body                the reference Body for "distance"
  Conform to                      none, cylinder
  Map value for Thickness at far end   the reach in mm for "distance"
```

Every length field takes an expression, so `wall = plate_z / 12` is a
legal entry and it updates when `plate_z` does.

## Why it works

`Graded` takes a centreline field (the mid sheet of a TPMS, the axis of
a beam, the centre of a honeycomb wall) and a second field that gives a
thickness, and returns `centre(p) - thickness(p) / 2`. Because the
centreline field is close to a true distance, subtracting a varying
half thickness gives a varying wall with no seams and no blending step.
`Ramp` and `Radial` are the two thickness fields; `Remap` turns any
other scalar into one.

Cell size grading cannot work that way, because scaling the coordinate
by a varying factor tears the cells apart. `CellRamp` instead integrates
the phase along the ramp axis: the phase is the integral of `2 pi /
cell(t)`, so the local period equals the local cell size everywhere and
the sheet stays continuous.

## Not yet available

* Cell size grading for beam lattices or honeycomb. Sheet lattices only.
* Cell size grading along a radius, or by a data field. The ramp is
  along one of x, y or z.
* Conformal maps other than the cylinder.

Next: [5. Fields from data](05-fields-from-data.md).
