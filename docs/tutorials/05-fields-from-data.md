# 5. Fields from data: CSV point maps, VTK grids, density import

Goal: stop typing numbers and let data set them. Three routes, in the
order you are likely to meet them: a CSV of sampled values drives a wall
thickness, a VTK grid does the same, and a topology optimisation density
becomes a Body in its own right.

## Step 1: look at the data

`docs/tutorials/files/stress.csv` is a point map: one row per sample,
`x, y, z` and a value. Here the value is a stress in MPa, highest at the
x = 0 end of the plate.

```
head -3 docs/tutorials/files/stress.csv
tail -1 docs/tutorials/files/stress.csv
```

```
x,y,z,stress
0,0,0,250.0
0,0,10,250.0
60,40,20,70.0
```

The header line is skipped, the fourth column is the value, and the
samples do not have to be on a grid or cover the whole part. Anvil reads
them into a bucket grid and interpolates by inverse distance weighting,
so a coarse scatter is fine.

## Step 2: let the stress set the wall thickness

`docs/tutorials/files/stress_plate.anvil` is a Box with a cubic lattice
on it, `grade` set to `map`, and `map_file` pointing at the CSV. The two
map values say which stress means which thickness: 70 MPa gives
`wall_low`, 250 MPa gives `wall_high`.

```
./target/release/anvil-cli run docs/tutorials/files/stress_plate.anvil \
    --formats stl --out out/f/stress
```

```
Wrote out/f/stress/stress_plate.stl (STL, 1 features, 82738 triangles)
Wrote out/f/stress/report.json (2 features, 0 errors)
```

```
python3 -c "
import json
r = json.load(open('out/f/stress/report.json'))
print(r['features'][1]['note'])
print(round(r['features'][1]['bodies'][0]['volume_mm3'], 1), 'of', round(r['features'][0]['bodies'][0]['volume_mm3'], 1))
"
```

```
cubic thickness from docs/tutorials/files/stress.csv, 82984 triangles, 8 percent of the solid volume, 498456 voxels
3867.8 of 48000.0
```

The note names the file, so you can tell at a glance that the map was
read and not silently skipped. Eight percent is a very light part,
because a cubic cell at 10 mm with a 1 to 3 mm beam is sparse; the point
is where the material went, not how much there is.

Change where the material goes without touching the CSV:

```
./target/release/anvil-cli run docs/tutorials/files/stress_plate.anvil \
    --set wall_low=1.2 --set wall_high=5 --formats stl --out out/f/stress2
```

```
cubic thickness from docs/tutorials/files/stress.csv, 122908 triangles, 19 percent of the solid volume, 498456 voxels
```

A steeper response to the same data: 19 percent instead of 8, with the
extra mass all at the loaded end. Keep both thicknesses at two voxels or
more; `wall_low=0.8` at a 0.5 mm resolution is refused for the reason
tutorial 2 step 5 explains.

Set `map_lo` and `map_hi` both to 0 and Anvil uses the map's own range
instead, which saves you looking the numbers up but ties the answer to
whatever the worst sample happened to be.

## Step 3: a VTK grid instead of a CSV

The same `grade = map` path reads a legacy VTK file. `load_scalar_file`
takes either, decided by the contents:

```
  .csv                     x, y, z, value rows -> PointMap, inverse distance
  .vtk STRUCTURED_POINTS   ORIGIN, SPACING, DIMENSIONS -> GridField, trilinear
  .vtk POINTS + SCALARS    scattered points -> PointMap
```

Point the same document at a VTK file by editing `map_file`. A grid is
the better input: it interpolates trilinearly instead of by distance
weighting, and it costs one lookup per sample rather than a neighbour
search.

## Step 4: a topology optimisation density becomes a Body

`docs/tutorials/files/density.vtk` is what a topology optimiser hands
back: a scalar per voxel between 0 and 1, on a 25 x 17 x 13 grid at 2 mm
spacing. Here it is a bar with a hole through it.

```
head -8 docs/tutorials/files/density.vtk
```

```
# vtk DataFile Version 3.0
topology optimisation density
ASCII
DATASET STRUCTURED_POINTS
DIMENSIONS 25 17 13
ORIGIN 0 0 0
SPACING 2.0 2.0 2.0
POINT_DATA 5525
```

`docs/tutorials/files/density.anvil` is one Density body Feature on it.

```
./target/release/anvil-cli run docs/tutorials/files/density.anvil \
    --formats stl --out out/f/density
```

```
Wrote out/f/density/density.stl (STL, 1 features, 12784 triangles)
Wrote out/f/density/report.json (1 features, 0 errors)
```

```
python3 -c "
import json
r = json.load(open('out/f/density/report.json'))
f = r['features'][0]
print(f['note'])
print(round(f['bodies'][0]['volume_mm3'], 1), 'mm3', f['bodies'][0]['open_edges'], 'open edges')
print('bounds', [round(v, 1) for v in f['bodies'][0]['min']], [round(v, 1) for v in f['bodies'][0]['max']])
"
```

```
grid 25 x 17 x 13, values 0.000 to 1.000, above 0.5: 12784 triangles, 9988 mm3
9987.6 mm3 0 open edges
bounds [3.0, 7.0, 3.0] [45.0, 25.0, 21.0]
```

The numbers to check: the grid spans 48 x 32 x 24 mm (24 spacings of 2
mm, 16 of 2 mm, 12 of 2 mm), and the Body sits well inside it, from
(3, 7, 3) to (45, 25, 21), because the density is 0 near the edges of
the grid. 0 open edges says the Body is closed and printable.

The three parameters that matter:

* **Threshold** (`level`, 0.5): where the density counts as solid. Raise
  it for a lighter, thinner part, lower it for a heavier one.
* **Smoothing** (`smooth`, 1 cell): a box blur before the threshold,
  which takes the checkerboard out of a raw optimiser result. 0 gives
  you the voxels back, staircase and all.
* **Resolution** (`res`, 0.8 mm): the output voxel size. 0 uses the
  grid spacing, 2 mm here, which is much coarser than the 0.8 the
  document asks for.

Try the threshold:

```
./target/release/anvil-cli run docs/tutorials/files/density.anvil \
    --set level=1.2 --formats stl --out out/f/d2
```

```
error: 1 feature(s) failed; see out/f/d2/report.json
```

and the report's feature error says what went wrong:

```
nothing above 1.2; the file's values run 0 to 1
```

That is the fastest way to find a file in the wrong units, or a solver
that wrote a density on a different scale than you assumed.

## Why it works

Data becomes a field in two steps. First the samples become a function:
`GridField` interpolates a grid trilinearly, `PointMap` buckets scattered
samples and weights the nearby ones by inverse distance. Then that scalar
becomes geometry, and there are two ways to spend it:

```
   scalar field ----> Remap ----> a thickness ----> Graded ----> lattice
                \
                 ---> Threshold ----> a signed value ----> surface nets ----> Body
```

`Remap` maps the value range onto a thickness range and clamps outside
it. `Threshold` subtracts the level and scales by a width taken from the
grid spacing and the smoothing, which turns a density into something
close enough to a signed distance for surface nets to mesh.

The same file can do either job. That is the point of keeping the data
as a field rather than as geometry: it is an input to the model, not a
step in it, and rerunning the solver and rerunning the model are the
same command.

## Not yet available

* Binary VTK, XML VTK (`.vtu`, `.vti`), or cell data. The reader takes
  legacy ASCII with point data.
* Tensor or vector fields. One scalar per point.
* Rescaling VTK point data. `scale` works on a grid; on VTK point data
  it must be 1, and Anvil says so rather than guessing.
* Reading the field back out of a solver automatically. You export the
  CSV or VTK yourself, and `anvil-cli run` is the hook for the loop.

Next: [6. Meshing, slices and the speed knobs](06-meshing-and-output.md).
