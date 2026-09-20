# 6. Meshing, slices and the speed knobs

Goal: choose a resolution on purpose instead of accepting the default,
understand what sharp feature meshing buys you, and get layers straight
out of the field without meshing at all.

You need `out/plate/plate.stl` from tutorial 2 step 1.

## Step 1: sweep the resolution

One field, five voxel sizes:

```
for r in 0.8 0.6 0.5 0.4 0.3; do
  ./target/release/anvil-cli lattice --stl out/plate/plate.stl \
      --kind gyroid --cell 10 --wall 1.6 --skin 1.2 --res $r --out out/res/$r
done
```

| `--res` | Voxels | Triangles | Percent of solid | Time |
| --- | --- | --- | --- | --- |
| 0.8 | 140616 | 144964 | 56 | 2.03 s |
| 0.6 | 309520 | 292144 | 55 | 3.86 s |
| 0.5 | 498456 | 369292 | 56 | 5.76 s |
| 0.4 | 934752 | 614268 | 56 | 8.78 s |
| 0.3 | 2105320 | 1135688 | 56 | 16.72 s |

Read the last two columns together. The volume fraction is 55 to 56
percent at every resolution: the geometry is already right at 0.8 mm
voxels. What the finer grids buy is a smoother triangulation, at eight
times the triangles and eight times the time from 0.8 to 0.3.

So the rule is: **set the resolution from the wall, not from the
picture.** Two voxels per wall is the floor Anvil enforces; three to
four gives a clean surface. A 1.6 mm wall is well served by 0.5 mm, and
0.3 mm only makes the file bigger.

Cost grows as the cube of 1/res, and so does memory. The feature refuses
outright above 400 million voxels:

```
error: ... million voxels; raise the resolution
```

## Step 2: what surface nets does, and the sharp feature rule

`surface_nets` samples the field at the corners of a grid of cubes.
Every cube whose corners differ in sign gets one vertex; every grid edge
that changes sign joins the four cubes around it with two triangles. The
mesh is closed as long as the field is positive over the whole boundary
of the box, which is why Anvil pads the box by three voxels.

Where the vertex goes inside its cube is the interesting part:

```
   plain surface nets            dual contouring placement
   vertex = mean of the          vertex solves min sum (n_i . (x - p_i))^2
   edge crossings                over the crossings and their normals
   -> rounded edges              -> sharp edges and corners
```

Anvil uses the second, always: `surface_nets` calls
`surface_nets_with(..., sharp = true)`. The quadratic error function is
solved by Cramer's rule, pulled toward the mass point so flat cells stay
stable, and a vertex that leaves its own cube is thrown away in favour
of the mass point, because a vertex escaping its cube means the normals
disagreed. Crossings are bisected on the field where it has a crease.

The test that keeps this honest is
`mesh::tests::box_corners_come_out_sharp`: box corners land within 0.02
mm at a 0.5 mm step, which is a twenty-fifth of a voxel.

`surface_nets_with(field, lo, hi, step, false)` is there if you want the
rounded version from Rust. Nothing on the command line or in the
property panel exposes the switch.

## Step 3: slices straight from the field

A slicer meshes a part and then intersects the mesh with a plane. Anvil
can skip the mesh: `slice` samples the input into a distance field and
walks marching squares across each layer, bisecting every crossing on
the field itself.

```
./target/release/anvil-cli slice --stl out/res/0.5/plate_lattice.stl \
    --layer 2 --res 0.5 --out out/sl
```

```
Wrote 10 layers to out/sl
```

```
python3 -c "
import json
r = json.load(open('out/sl/layers.json'))
for l in r['layers']:
    print('z=%5.1f loops=%3d area=%8.1f perim=%8.1f' % (l['z'], l['loops'], l['area_mm2'], l['perimeter_mm']))
"
```

```
z=  1.0 loops= 53 area=  1827.1 perim=  1033.7
z=  3.0 loops= 20 area=  1207.5 perim=  1361.4
z=  5.0 loops=  9 area=  1036.5 perim=  1314.1
z=  7.0 loops= 20 area=  1207.5 perim=  1361.4
z=  9.0 loops= 53 area=  1235.2 perim=  1219.7
z= 11.0 loops= 53 area=  1235.2 perim=  1219.7
z= 13.0 loops= 20 area=  1207.5 perim=  1361.4
z= 15.0 loops=  9 area=  1036.5 perim=  1314.1
z= 17.0 loops= 20 area=  1207.5 perim=  1361.4
z= 19.0 loops= 53 area=  1827.1 perim=  1033.7
```

The numbers to check. The plate is 20 mm tall and the layer height is 2
mm, so there are 10 layers, sampled at the middle of each. A solid plate
would be 60 x 40 = 2400 mm2 on every layer; this one runs 1036 to 1827
mm2, so it is 43 to 76 percent dense by area. The loop count follows the
gyroid: 53 loops at the skins, 9 where the sheet is at its most open,
and the whole sequence is a mirror image about mid height, as a 10 mm
cell in a 20 mm plate should be. The first and last layers have the
largest area and the shortest perimeter, because the skin dominates
there.

Each layer is also an SVG you can open in a browser:

```
head -2 out/sl/layer_0003.svg
```

```
<svg xmlns="http://www.w3.org/2000/svg" width="61mm" height="41mm" viewBox="-0.5 -0.5 61 41">
<g transform="translate(0 40) scale(1 -1)">
```

The viewBox is the part's bounding box padded by one voxel, and the
transform flips Y so the SVG reads the same way up as the model. Holes
come out as loops of the opposite turn and the SVG fills by the even odd
rule, so a hole is a hole.

The layer height is free: it is not tied to the voxel size, because
nothing is meshed. `--layer 0.2` on the same input gives 100 layers with
no extra sampling cost per layer.

## Step 4: which file format for what

`anvil-cli lattice` writes three at once. `anvil-cli run --formats`
takes a comma list of `3mf, 3mf-group, stl, stl-parts, step, obj, ply,
off, amf, gltf`.

```
  stl          one mesh, no units, no colour. Every slicer reads it.
  stl-parts    one file per Body.
  3mf          one object per Body, with units. Prefer it over STL.
  3mf-group    3MF with the Bodies grouped.
  step         a faceted solid. Other CAD will open it; it is not analytic.
  obj/ply/off/amf/gltf   for viewers and meshing tools.
```

For a lattice, 3MF is the right default: it carries millimetres
explicitly, which STL does not. For a beam lattice, the 3MF beam
extension from tutorial 3 is better still.

## Why it works

Surface nets is a dual method: one vertex per cube rather than one per
edge crossing, which is why the mesh is closed and manifold by
construction and why the triangle count scales with the surface area and
not with the field's complexity. Sharp placement then recovers the
corners a dual method would otherwise round off, using the gradients the
`Field` trait supplies (analytic for spheres, boxes, TPMS and beam
lattices, central differences otherwise).

The speed comes from skipping: before sampling, blocks of cubes whose
corners are all the same sign and further from the surface than the
block diagonal cannot contain a crossing, because the field is close to
a distance. Those blocks are never sampled. A lattice is mostly empty
space at any one moment, so that is most of the grid.

## Not yet available

* An octree with interval pruning, the manifold clustering rule, and
  mesh smoothing or decimation. Milestone F4 lists all three as open.
* A resolution that adapts to curvature. One voxel size for the whole
  Body.
* Overhang maps and print preparation from the field. That is F6.
* A GPU path for evaluation or meshing.

Next: [7. A full part, start to export](07-a-full-part.md).
