# Anvil workbook

Six practice parts in the style of the CSWA exam. Each one lists the GUI
steps, the expected mass properties, and what it exercises. The same parts
are built through the feature API in `crates/anvil-io/src/workbook.rs`,
so every exercise is also a regression test, and each is a button under
Examples > Workbook (WB 1 to WB 6). Saved documents are in `examples/workbook/`
and renders in `docs/workbook/`.

Units are millimetres. Material is steel at 7.85 g/cm3 unless stated.
Mass properties come from Measure and Center of Mass on the Inspect tab.

## Reading the expected values

| Exercise | Volume (mm3) | Mass (g) | Bodies |
| --- | --- | --- | --- |
| 01 Mounting plate | 43185 | 339.0 | 1 |
| 02 L-bracket | 27310 | 214.4 | 1 |
| 03 Stepped shaft | 40967 | 321.6 | 1 |
| 04 Hex nut | 1327 | 10.4 | 1 |
| 05 Pipe elbow | 16560 | 130.0 | 1 |
| 06 Adapter | 25761 | 202.2 | 1 |

Values are from the faceted kernel, so curved parts read slightly low
(about 0.5 percent for a 48-segment circle). The hand calculation for 01
is 43075 mm3; the kernel reads 0.26 percent high, see the known limits.

The table is filled by `cargo run -p anvil-ui --example render_workbook`,
which prints volume and mass for each part. The tests in `workbook.rs`
check each volume against a hand calculation.

## 01 Mounting plate

100 x 60 x 8 plate, R10 corners, four 8 mm holes 10 mm from each edge, and
a 30 x 12 slot in the middle.

1. Solid > Sketch, click the XY datum square.
2. Draw the outline: Rectangle 2-pt from (0,0) to (100,60), then Fillet each
   corner with value 10 (click two lines per corner). Or use the sample.
3. Finish Sketch. Extrude, distance `8`.
4. Hole four times: plane XY, offset `8`, diameter `8`, depth `9`, at
   (10,10), (90,10), (90,50), (10,50). Each Hole defaults to the selected
   feature, so select the newest body first.
5. Sketch on the top face (click the face, then Sketch). Slot centre to
   centre from (41,30) to (59,30), width 12. Finish.
6. Extrude, distance `-9`, operation `cut`, target the body.

Exercises: rounded rectangle, holes with expressions, slot, cut extrude.

## 02 L-bracket

60 base, 40 upright, 50 wide, 6 thick, R6 inner fillet, two 7 mm holes in
the base, one 14 x 7 slot in the upright.

1. Sketch on the XZ datum. Draw the L with the Line tool: (0,0) (60,0)
   (60,6) (6,6) (6,40) (0,40), closing on the first point.
2. Fillet the inner corner: value 6, click the two lines meeting at (6,6).
3. Finish. Extrude `-50` (the XZ normal points to negative Y, so a negative
   distance builds toward positive Y).
4. Hole twice on plane XY, offset `6`, diameter `7`, depth `10`, at (45,12)
   and (45,38).
5. Sketch on the inside face of the upright. Slot from (18,25) to (32,25),
   width 7. Finish. Extrude `-8`, operation `cut`.

Exercises: sketch fillet, extrude direction, sketch on a vertical face.

## 03 Stepped shaft

Diameters 20, 30, 16 over lengths 30, 40, 20, with 1 mm chamfers at both
ends, a 6 x 3 x 25 keyway on the middle step, and a 4 mm cross hole in the
small end.

1. Sketch on XY. Draw the half profile above the X axis with Line:
   (0,0) (0,9) (1,10) (30,10) (30,15) (70,15) (70,8) (89,8) (90,7) (90,0),
   closing on the first point.
2. Finish. Revolve about axis X, 360.
3. Sketch on a plane at Z 15 (Construct > Offset Plane from XY by 15, then
   Sketch on it). Rectangle from (38,-3) to (63,3). Finish. Extrude `-3`,
   operation `cut`.
4. Hole on plane XZ, offset `10`, at (80,0), diameter `4`, depth `20`. The
   XZ datum normal points to negative Y, so offset 10 puts the plane at
   y = -10 and the hole drills toward positive Y through the shaft.

Exercises: revolve, chamfer in the profile, cut on an offset plane, a hole
along the Y axis.

## 04 Hex nut

M10 nut, 17 across flats, 8 thick, chamfered corners, 10 mm bore.

1. Sketch on XY. Polygon inscribed, centre (0,0), apothem 8.5, 6 sides.
   Finish. Extrude `8`.
2. Sketch on XZ. Draw the chamfer profile with Line: (0,0) (7.74,0)
   (9.81,1.2) (9.81,6.8) (7.74,8) (0,8), close.
3. Finish. Revolve about axis Y, 360, operation `intersect`, target the
   hexagon body.
4. Hole on XY, offset `8`, diameter `10`, depth `9`.

Exercises: polygon, revolve intersect as a chamfer, through hole.

## 05 Pipe elbow

12 mm pipe along a 40 mm line, a 20 mm radius quarter arc, and a 40 mm
line, with two 24 x 24 x 4 flanges and bolt holes.

1. Sketch on XY. Line (0,0) to (40,0). Arc centre: centre (40,20), start
   (40,0), end (60,20). Line (60,20) to (60,60). Finish.
2. Solid > Pipe, path = that sketch, diameter 12.
3. Box at (-2,-12,-12) size 4 x 24 x 24. Box at (48,58,-12) size 24 x 4 x 24.
4. Combine join pipe with the first box; Combine join the result with the
   second box.
5. Hole twice on plane YZ, offset `2`, diameter `3`, depth `5`, at (-8,-8)
   and (8,8).

Exercises: open path sketch, pipe, primitives, combine, holes on YZ.

## 06 Adapter

Square-to-round adapter: 40 mm square to 24 mm circle over 30 mm, a 12 mm
bore, and the word ANVIL engraved 0.5 mm into the bottom face.

1. Sketch on XY: Rectangle centre (0,0), 40 x 40. Finish.
2. Offset Plane from XY by 30. Sketch on it: Circle centre (0,0) radius 12.
   Finish.
3. Loft, sketches `0, 1`.
4. Hole on XY, offset `30`, diameter `12`, depth `31`.
5. Text "ANVIL", plane XY, y `-19`, size 5, height 0.5, operation `cut`,
   target the body.

Exercises: loft, offset plane, text as a cut.

## Findings from working the exercises

A review of the code against these six parts ran on 2026-09-11. Each
finding was checked by a second reviewer who tried to refute it. The run
stopped early on a usage limit, so the GUI, file, and documentation
dimensions were only partly verified.

### Confirmed and fixed

| Severity | Finding | Fix |
| --- | --- | --- |
| High | Exercise 03 cross hole sat beside the shaft and cut nothing, silently. | Offset sign corrected. Hole and cut Extrude now report an error when they remove no material. A test checks the hole's volume. |
| High | Deleting a feature did not renumber later references, so later features pointed at the wrong input. | Features now remap references on delete, insert, and move. References to a deleted feature show a clear error. |
| High | Midpoint, centre, and quadrant snap markers showed, but the click landed elsewhere. | The click now uses the same snap as the marker. |
| High | Delete, Q, L, R, C, D, and Ctrl+Z fired while typing in a text field. | Shortcuts are ignored while a text field has focus. |
| Medium | Sketching on a face projected its outline as normal lines, so extruding a slot also extruded the whole face outline. | Projected edges are construction geometry. |
| Medium | Hole needed a typed plane and offset; it ignored the selected face. | Select a face, then Hole, Text, QR Code, or Texture: the feature is placed on that face. |
| Medium | Sweep and Pipe pinched the section at sharp path corners. | Sections are mitred at bends. |
| Medium | Symmetric Extrude with a negative distance was off-centre. | Centred on the sketch plane. |
| Medium | Hole bridging in face triangulation could cross an edge. | Bridges are chosen with a visibility test. |
| Low | A hole touching the outer loop at one vertex became a second solid. | Nesting uses several sample points. |
| Low | Revolve made a separate vertex per ring on the axis. | One shared pole vertex. |
| Low | CSG results had T-junctions and open edges, bad for STL printing. | T-junction repair; results are watertight (tested). |

### Known limits after this pass

* Several booleans in a row (for example five holes in one plate) leave
  thin fragments on flat faces. They show as stray edge lines on the top
  faces of exercises 01 and 06, and exercise 01 reads 0.26 percent above
  its hand calculation. The mesh has no open edges, so slicers accept it,
  but some edges are shared by more than two faces. A tolerant boolean
  (ADR 0001, OpenCASCADE behind the kernel trait) is the real fix.
* Before this pass the workbook tests took 21 minutes because boolean
  fragments multiplied. Coplanar fragments are now merged and the tests
  take under 3 seconds.

### Reported but not yet verified

* Loft sections on flipped or rotated planes can twist. The workbook only
  uses parallel planes.
