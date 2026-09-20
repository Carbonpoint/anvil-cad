# 3. Beam lattices and the 3MF beam extension

Goal: fill the same plate with struts instead of sheets, compare the
four unit cells, pick a strut diameter, and write the result as a 3MF
beam lattice: nodes and radii instead of twenty million triangles.

You need `out/plate/plate.stl` from tutorial 2 step 1.

## Step 1: the four unit cells

A beam lattice is the distance to a fixed list of line segments inside
one cell, repeated. Anvil has four cells:

```
  cubic    12 edges of the cube            stiff along the axes, weak in shear
  bcc      8 body diagonals                good in shear, isotropic-ish
  octet    edges plus face diagonals       the stiffest per unit mass
  kelvin   the tetrakaidecahedron          nearly isotropic, prints well
```

Run all four at a 10 mm cell and a 0.8 mm beam radius:

```
for k in cubic bcc octet kelvin; do
  ./target/release/anvil-cli beams --stl out/plate/plate.stl \
      --kind $k --cell 10 --radius 0.8 --res 0.5 --out out/b/$k
done
```

```
Wrote out/b/cubic/plate_beams.3mf with 68 beams on 61 nodes, 680 mm of beam, about 1367 mm3 of material
Wrote out/b/bcc/plate_beams.3mf with 452 beams on 373 nodes, 3989 mm of beam, about 8020 mm3 of material
Wrote out/b/octet/plate_beams.3mf with 996 beams on 637 nodes, 7236 mm of beam, about 14548 mm3 of material
Wrote out/b/kelvin/plate_beams.3mf with 928 beams on 591 nodes, 6556 mm of beam, about 13181 mm3 of material
```

The numbers to check: the plate is 48000 mm3 solid, so octet at 0.8 mm
radius is about 30 percent dense and cubic is under 3 percent. The
material figure is the beam length times the circular area, so it double
counts the overlaps at the nodes; treat it as an upper bound.

## Step 2: choose the strut diameter

Radius scales the material as the square, and nothing else changes:

```
./target/release/anvil-cli beams --stl out/plate/plate.stl \
    --kind octet --cell 10 --radius 1.2 --res 0.5 --out out/b/octet12
```

```
Wrote out/b/octet12/plate_beams.3mf with 996 beams on 637 nodes, 7236 mm of beam, about 32734 mm3 of material
```

Same 996 beams and same 7236 mm of beam, but 32734 mm3 instead of 14548.
That is (1.2 / 0.8) squared, 2.25, exactly. Cell size is the other lever,
and it changes everything:

```
./target/release/anvil-cli beams --stl out/plate/plate.stl \
    --kind octet --cell 6 --radius 0.8 --res 0.5 --out out/b/octet6
```

```
Wrote out/b/octet6/plate_beams.3mf with 5871 beams on 2619 nodes, 24329 mm of beam, about 48917 mm3 of material
```

Halving the cell roughly cubes the beam count and here the struts have
begun to merge into each other: 48917 mm3 of "material" against a 48000
mm3 plate means the double counting has taken over. Drop the radius when
you drop the cell.

## Step 3: what the 3MF actually contains

```
ls -l out/b/octet/plate_beams.3mf
unzip -p out/b/octet/plate_beams.3mf 3D/3dmodel.model | head -20
```

```
<model unit="millimeter" ... xmlns:b="http://schemas.microsoft.com/3dmanufacturing/beamlattice/2017/02" requiredextensions="b">
<object id="1" name="plate" type="model"><mesh><vertices>
<vertex x="0.00122" y="10.00000" z="10.00000"/>
...
<b:beamlattice radius="0.80000" minlength="0.08000" cap="sphere"><b:beams>
<b:beam v1="0" v2="1"/>
```

A vertex list, then one `<b:beam>` per strut with the radius and the cap
style declared once on the lattice. The file is 59629 bytes. The same
octet lattice meshed to triangles and written as STL (step 4 below) is
16670684 bytes, 280 times larger. The slicer or the printer builds the
solid geometry itself, at its own resolution, which is both smaller and
more accurate than sending it a mesh you already sampled.

```
   meshed lattice                 3MF beam lattice
   field -> 342148 triangles      637 nodes + 996 beams + one radius
   -> 16.7 MB STL -> slicer       -> 60 kB -> slicer builds the geometry
```

Anvil clips the beams to the surface of the input mesh. `beams_in`
keeps a beam whose ends are both inside the sampled field, drops one
with both ends outside, and bisects the field twelve times to find where
a half inside beam crosses the surface, so nothing is left hanging
outside the part.

## Step 4: a beam lattice as a Body instead

If you want the struts as real geometry in the history, use Lattice fill
with a beam kind. The `--wall` flag is the beam thickness there:

```
./target/release/anvil-cli lattice --stl out/plate/plate.stl \
    --kind octet --cell 10 --wall 1.0 --skin 1.0 --res 0.5 --out out/g2/n
```

```
Lattice fill (octet 10 mm): octet lattice, 342148 triangles, 27 percent of the solid volume, 498456 voxels
```

Now you have a closed Body that later Features can move, pattern, split
and export, at the price of 342148 triangles.

## Why it works

The beam field is the distance to the nearest segment of the unit cell,
evaluated in cell local coordinates, so one function describes every cell
in the part. Thickening it by the radius is a subtraction of a constant,
which is why the radius is free and why grading it later (tutorial 4)
costs nothing either. Writing the 3MF beam lattice skips the field
entirely: `BeamLattice::beams_in` lists the segments whose ends are
inside the part and hands the list to the writer.

## Not yet available

* Cell size grading for beam lattices. `CellRamp` grades sheet lattices
  only, so `cell_end` does nothing with a beam kind.
* Per beam radii in the 3MF. Anvil writes one radius for the whole
  lattice, although the extension allows a radius per beam end.
* A skin around a 3MF beam lattice. `beams` writes struts only; the
  solid skin in tutorial 2 belongs to the meshed Lattice fill route.

Next: [4. Grading: thickness and cell size](04-grading.md).
