# Cast iron kettle (tetsubin)

A sample project that drives Anvil toward casting work. The target is a
kettle in the style of the Iwachu "9 type flat round" (hiramaru) tetsubin:
a flat bottom, a plain lower band, a sharp ridge at about one third of the
height, a domed upper body covered in raised dots, a short curved spout,
two lugs for a bail handle, and a domed lid with a lotus bud knob.

Reference (Iwachu product 11702, tortoiseshell pattern): 18 x 16 x 18.5 cm
with the handle, 1.1 L full, about 1.5 kg. The 7-type and 9-type arare
kettles have single-size dots that shrink toward the mouth.

## Plan

Each stage is usable on its own. Stages 1 to 3 are the shape. Stage 4 is
the mold. Stage 5 is the simulation.

| Stage | Result | Anvil work |
| --- | --- | --- |
| 1 | Body of revolution with dots | Hobnail feature (revolve with relief), localized booleans |
| 2 | Spout, lugs, bail, lid, knob | Tapered pipe, sample document `kettle()` |
| 3 | Sprue, runner, gates, riser | Casting ribbon tab |
| 4 | Pattern halves, core, core prints | Parting plane split, draft check, shrink scale |
| 5 | Fill and solidification | Export to Truchas or OpenFOAM, or a built-in voxel model |

Spout: the shape must pour without dripping. The rules of thumb are a
sharp thin lip, a bore that narrows toward the tip, an outlet above the
full water level, and a rise of 40 to 50 degrees. The spout is parametric
so these can be tuned, and stage 5 can test pouring later.

## Design decisions

Dots are a displacement of a fine revolve mesh, not a boolean of many
spheres. One kettle has about 1500 dots. A boolean per dot would take
minutes and leave fragments. The Hobnail feature revolves the profile at
a chosen resolution (segments around, step along the profile) and moves
each vertex along the surface normal by the bump height. The result is a
single watertight triangle mesh, so slicers, the mold split, and a voxel
simulation all accept it.

Dot rows keep a constant count, so the dots line up in spiral columns and
shrink as the radius shrinks, the way a real arare kettle looks. The
tortoiseshell layout is one large dot ringed by small dots in each cell.

Booleans on the dotted body (spout, lugs, bore) must stay fast. The BSP
boolean now keeps polygons that lie outside the other body's bounding box
without clipping them, and builds the BSP trees only from the polygons
near the overlap. Leaves of a partial tree are classified with a ray test
against the full mesh, so the result is still exact.

## Dimensions (mm)

| Item | Value | Note |
| --- | --- | --- |
| Body diameter at the ridge | 160 | |
| Ridge height | 30 | flat band below |
| Body height to the rim | 95 | without lid |
| Mouth diameter | 88 | lid seat |
| Wall thickness | 3.5 | cast iron |
| Dot pitch | 4.5 | at the ridge |
| Dot height | 1.0 | |
| Spout base, tip diameter | 24, 15 | outside |
| Bail bar diameter | 8 | |
| Lid diameter | 96 | sits on the mouth |

## Progress log

* 2026-09-16: plan written. Reference photos reviewed. Kernel relief op
  and localized booleans in progress.
