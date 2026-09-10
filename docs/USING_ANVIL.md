# Using Anvil

This page walks through the sketch and feature workflow as it works today.

## Start a sketch

1. On the Solid tab, click **Sketch**.
2. Three datum squares appear at the origin. Hover one and click it, or
   click a face of an existing body. The camera turns to look straight at
   that plane and the Sketch tab opens.

## Draw

The Line tool is active first. Click points. Click an existing point to
close the loop. Right-click or Esc ends the chain.

Other tools: Rect 2-pt, Rect centre, Circle centre, Circle 2-pt, Circle
3-pt, Arc 3-pt, Arc centre, Polygon (set sides in the ribbon), Slot (set
width in the ribbon), Point. Each tool's hint shows in the ribbon message.

Clicks that land on an existing point reuse that point. Turn on **Snap grid**
to snap to the grid.

## Constrain and dimension

1. Choose **Select**. Click an entity. Shift+click adds to the selection.
2. Click a constraint button. Each needs a specific selection:
   Coincident (2 points), Horizontal or Vertical (lines), Parallel,
   Perpendicular, Equal (2 lines or 2 circles), Tangent (line + circle),
   Midpoint (point + line), Concentric (2 circles), Symmetric (2 points +
   line), On curve (point + line or circle), Fix (points).
3. For a dimension, type the value in the field, then click Length /
   Distance (a line, or two points), Radius (a circle or arc), or Angle
   (two lines).

The top-left corner shows the degrees of freedom. "Fully constrained" means
zero. Drag a point with the Select tool to see the solver hold constraints.

**Delete** removes selected entities and their constraints. **Construction**
toggles selected lines to construction lines, which do not form profiles.

## Finish and build

Click **Finish Sketch**. The camera returns. Select the sketch in the Part
Navigator, then click Extrude, Revolve, Sweep, or Loft. The new feature
points at the selected sketch. Edit its parameters in the Properties panel.

Double-click a sketch in the Part Navigator to edit it again. Everything
downstream regenerates.

## Bodies

Click a body in the viewport to select its feature. Move/Copy, Scale,
Mirror, and the patterns take a body feature as input; they default to the
selected feature. Measure shows the volume and bounding box in the status
bar.

## What is not there yet

Fillet, Chamfer, Shell, Combine, and Hole are on the ribbon but report
"not supported by this kernel yet". Extrude cannot cut into another body.
These wait on the kernel decision in ADR 0001.
