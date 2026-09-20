# 7. A full part, start to export

Goal: two complete jobs, each from nothing to a file you can hand over.
First a ribbed bracket, where the field does the lightening. Then the
aircraft wing, where the field is the part and a solver drives it.

## Part one: the bracket

A 60 x 40 x 10 mm mounting plate with a bolt boss at one end. The plate
has to be light, and stiff where the bolt loads it.

```
   y
   ^   +----------------------------+
   |   |                       ( )  |   boss: 12 mm across, 18 mm tall
   |   |   honeycomb ribs           |   ribs: 2.4 mm at the boss
   |   |                            |         0.8 mm at 30 mm away
   +   +----------------------------+--> x
       0                           60
```

### Step 1: the solid and the boss

`docs/tutorials/files/bracket.anvil` is three Features:

```
  0  box           plate_x by plate_y by plate_z
  1  cylinder      the boss, at (plate_x - 12, plate_y / 2), radius 6
  2  lattice_fill  honeycomb in body 0, graded by distance to body 1
```

Every number is an expression, so you can size the plate from the
command line and the ribs follow.

### Step 2: build it

```
./target/release/anvil-cli run docs/tutorials/files/bracket.anvil \
    --formats stl,3mf,step --out out/g/bracket
```

```
Wrote out/g/bracket/bracket.stl (STL, 2 features, 285056 triangles)
Wrote out/g/bracket/bracket.3mf (3MF, one object per feature, 2 features, 285056 triangles)
Wrote out/g/bracket/bracket.step (STEP, 2 faceted solids)
Wrote out/g/bracket/report.json (3 features, 0 errors)
```

Two Bodies come out, the ribbed plate and the boss, because Lattice fill
consumed the Box but not the Cylinder.

### Step 3: read the report before you send the file

```
python3 -c "
import json
r = json.load(open('out/g/bracket/report.json'))
for f in r['features']:
    b = f['bodies'][0]
    print(f['index'], f['kind'], round(b['volume_mm3'], 1), 'mm3', b['open_edges'], 'open edges', f['mass_g'] and round(f['mass_g'], 2))
print('visible bodies', r['visible_bodies'], 'errors', r['errors'])
"
```

```
0 box 24000.0 mm3 0 open edges None
1 cylinder 2029.9 mm3 0 open edges None
2 lattice_fill 11593.5 mm3 0 open edges 31.3
visible bodies 2 errors 0
```

Three things to check every time, in this order:

1. **`errors` is 0.** A failed Feature still writes a report.
2. **`open_edges` is 0** on the Body you are exporting. A lattice with
   open edges is a lattice that will not slice.
3. **The volume is what you meant.** 11593.5 mm3 against 24000 mm3
   solid, so the ribs left 48 percent of the plate, 31.3 g of aluminium
   6061 instead of 64.8 g.

### Step 4: check the layers before you print

Slicing the exported STL tells you what the printer will see:

```
./target/release/anvil-cli slice --stl out/g/bracket/bracket.stl \
    --layer 2 --res 0.5 --out out/g/bracket/sl
```

```
python3 -c "
import json
for l in json.load(open('out/g/bracket/sl/layers.json'))['layers']:
    print('z=%5.1f loops=%3d area=%8.1f' % (l['z'], l['loops'], l['area_mm2']))
"
```

```
z=  1.0 loops= 53 area=  1387.3
z=  3.0 loops= 58 area=   998.9
z=  5.0 loops= 58 area=   999.5
z=  7.0 loops= 58 area=   999.5
z=  9.0 loops= 53 area=  1388.8
z= 11.0 loops=  1 area=   111.6
z= 13.0 loops=  1 area=   111.6
z= 15.0 loops=  1 area=   111.6
z= 17.0 loops=  1 area=   111.6
```

Read it as a cross section of the part. The bottom and top skin layers
(z = 1 and z = 9) carry more area than the ribbed middle. Above z = 10
the plate has ended and only the boss is left: one loop of 111.6 mm2,
which against a 6 mm radius boss (pi times 36, 113.1 mm2) is within half
a percent, the voxel error you expect at a 0.5 mm resolution.

### Step 5: sizes, and which file to send

```
ls -l out/g/bracket/bracket.*
```

```
 14252884  bracket.stl
 54416293  bracket.3mf
112224804  bracket.step
```

Send the 3MF: it carries millimetres explicitly and one object per Body.
The STEP is a faceted solid, useful for dropping into another CAD
assembly but eight times the size and no more accurate.

### Step 6: resize the part without touching the file

```
./target/release/anvil-cli run docs/tutorials/files/bracket.anvil \
    --set plate_x=90 --set plate_y=50 --set cell=10 --set res=0.6 \
    --formats 3mf --out out/g/bracket90
```

```
Wrote out/g/bracket90/bracket.3mf (3MF, one object per feature, 2 features, 203592 triangles)
Wrote out/g/bracket90/report.json (3 features, 0 errors)
```

The note reads `honeycomb thickness by distance to feature 1, 203804
triangles, 40 percent of the solid volume`, and the Body is 18107.9 mm3,
48.89 g. The boss stays at `plate_x - 12`, the reach stays 30 mm, and
the ribs regrade themselves around the new boss position. That is the reason to
keep the field inside the history instead of exporting a mesh and
lattice-ing it: the lattice is a Feature, so it regenerates.

## Part two: the aircraft wing

The Aircraft Feature builds a fuselage, a lofted NACA wing, a tailplane
and a fin as one implicit body, and reports a lifting line estimate for
it.

### Step 1: write the document

```
./target/release/anvil-cli aircraft --out out/plane
```

```
Wrote out/plane/plane.anvil
wing area 0.004 m2, aspect ratio 8.0, taper 0.50; at CL 0.50 and 20 m/s: alpha 4.0 deg, CDi 0.0107, CD0 0.0236, e 0.93, L/D 14.6; vortex lattice at the same angle: CL 0.49, CDi 0.0095
```

The document carries seven expressions: `span`, `root_chord`, `taper`,
`sweep`, `dihedral`, `twist_tip` and `resolution`. The tip chord is the
expression `root_chord * taper`, so taper is a real parameter and not a
number you have to keep consistent by hand.

### Step 2: change the design and rebuild

```
./target/release/anvil-cli run out/plane/plane.anvil \
    --set taper=0.35 --set twist_tip=-3 --formats 3mf,stl --out out/plane/opt
```

```
Wrote out/plane/opt/plane.3mf (3MF, one object per feature, 1 features, 36184 triangles)
Wrote out/plane/opt/plane.stl (STL, 1 features, 36184 triangles)
Wrote out/plane/opt/report.json (1 features, 0 errors)
```

The note in the report is the comparison you came for:

| | taper 0.50, twist -1 | taper 0.35, twist -3 |
| --- | --- | --- |
| aspect ratio | 8.0 | 8.9 |
| alpha at CL 0.50 | 4.0 deg | 4.7 deg |
| CDi | 0.0107 | 0.0109 |
| CD0 | 0.0236 | 0.0241 |
| span efficiency e | 0.93 | 0.82 |
| L/D | 14.6 | 14.3 |

A sharper taper with more washout raised the aspect ratio but cost span
efficiency, and L/D went down, not up. That is the loop working: one
command, one number, an answer you did not have to guess.

### Step 3: hand the shape to a real solver

```
./target/release/anvil-cli run out/plane/plane.anvil \
    --openfoam --speed 25 --alpha 4 --formats stl --out out/plane/cfd
```

```
Wrote out/plane/cfd/plane.stl (STL, 1 features, 38192 triangles)
Wrote an OpenFOAM case (15 files) to out/plane/cfd/openfoam (Aref 0.0073 m2, cref 0.0404 m)
Wrote out/plane/cfd/report.json (1 features, 0 errors)
```

```
ls out/plane/cfd/openfoam
```

```
0  Allrun  README.md  constant  system
```

A simpleFoam case with k omega SST and force coefficients, with the
reference area and chord taken from the bounding box unless you give
`--sref` and `--cref`. The faces of an Aircraft Body carry a component
tag, so the export writes one named STL solid and one boundary patch per
component: the wing, the fuselage, and the tail are separate patches in
the case.

`docs/examples/wing_loop.py` is a scripted sweep over the same
expressions, if you want the loop closed rather than run by hand.

## Why it works

Both parts are the same pattern. The geometry is a field, the dimensions
are expressions in the Document, and `anvil-cli run` is a pure function
from expressions to files plus a JSON report. Nothing about the design
lives in a GUI session, so a shell loop, a Python script or an optimiser
drives it the same way you do.

The bracket shows the field replacing modelling work: there is no
Feature in that history that draws a rib. The wing shows the field
replacing a surface model: a NACA section lofted with taper, sweep,
dihedral and twist is a nuisance in a B-rep and a closed form in a
field.

## Not yet available

* Mass properties for a lattice Body beyond volume and mass. No centre
  of mass or inertia tensor in the report.
* A structural check on the bracket. The report tells you what you
  built, not whether it holds.
* Running OpenFOAM or reading its results back. Anvil writes the case;
  you run it.

Next: [8. The Fidget back end](08-fidget.md).
