# Anvil vs Fusion Design Workspace: Command Reference and Parity Status

> Reference date: 2026-09-10. Written by a research agent from the Fusion help pages listed at the end, then checked against Anvil.
>
> Corrections to the status tables: orbit, pan, and zoom (left drag, middle or right drag, scroll wheel), the Part Navigator tree (Fusion's Browser), Look At, Construction toggle, Show Points, and Show Constraints all exist in Anvil today. Sketch dimensions take numbers only, not expressions, for now. Extrude makes new bodies only until the kernel gains booleans (ADR 0001).


This document lists every command in Autodesk Fusion's Design workspace **Solid** tab and contextual **Sketch** tab, plus the core viewport interactions. It is meant as a build target for Anvil, an open source parametric CAD/CAM tool written in Rust. Each command gets a name, its ribbon group, a one-sentence description, and its inputs. A status section at the end marks what Anvil has today.

Autodesk reorganizes ribbon panels between releases. The groupings below reflect the mainstream 2024-2026 layout as documented in Fusion Help and confirmed tutorials. A few commands (Press Pull, sketch Mirror and Pattern) have moved panels across versions; this document notes the current placement.

## Part 1: Solid Tab

### 1.1 Create panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Extrude | Solid > Create | Adds depth to a sketch profile or face to make or modify a solid. | Profile or face, extent type (distance, to object, to next, all, two sides), taper angle, operation (join, cut, intersect, new body) |
| Revolve | Solid > Create | Spins a profile or face around an axis to make a solid. | Profile or face, axis, extent (full circle, angle, to object), operation |
| Sweep | Solid > Create | Moves a profile along a path to make a solid. | Profile, path, orientation (perpendicular to path or parallel to sketch plane), taper angle, twist angle, operation |
| Loft | Solid > Create | Blends a smooth solid between two or more profiles. | Two or more profiles, optional rails, optional centerline, operation |
| Rib | Solid > Create | Turns an open sketch curve into a thin wall for reinforcement. | Open profile, thickness, extent, draft angle, side |
| Web | Solid > Create | Builds a network of thin ribs perpendicular to the sketch plane from branching curves. | Open profile network, thickness, draft, extent |
| Emboss | Solid > Create | Raises or recesses a sketch profile or text relative to a face. | Profile or text, target face, depth, emboss or deboss mode |
| Boss | Solid > Create | Adds a fastening post between two bodies, often paired with a screw boss hole. | Sketch point, diameter, height, fastener settings |
| Snap Fit | Solid > Create | Builds a cantilever or torsion connector that clips two bodies together. | Placement point or edge, connector type, width, thickness, length, catch angle |
| Thicken | Solid > Create | Adds thickness to a surface to turn it into a solid. | Faces, thickness, direction, operation |
| Create Base Feature | Solid > Create | Marks a point in the timeline to allow direct edits ahead of parametric features. | None (toggles a marker) |
| Create Form | Solid > Create | Opens the Form environment for freeform T-spline sculpting. | None (switches workspace) |
| Create Mesh | Solid > Create | Converts a solid or surface body into a mesh body for print prep. | Body, resolution/refinement settings |
| Create PCB | Solid > Create | Links an electronic PCB design into the model as 3D geometry. | PCB file or link, board outline |
| Coil | Solid > Create | Creates a helical solid such as a spring or thread form. | Profile or diameter, axis, revolutions or height or pitch, rotation direction, taper |
| Pipe | Solid > Create | Sweeps a circular or custom section along a path with wall thickness. | Path, section size, wall thickness, operation |
| Box | Solid > Create | Creates a rectangular solid primitive. | Base plane or face, two corner points or center, length, width, height |
| Cylinder | Solid > Create | Creates a cylindrical solid primitive. | Base plane or face, center point, diameter, height |
| Sphere | Solid > Create | Creates a spherical solid primitive. | Center point, diameter |
| Torus | Solid > Create | Creates a donut-shaped solid primitive. | Center point, major diameter, minor (tube) diameter |
| Rectangular Pattern (body) | Solid > Create | Copies bodies, faces, features, or components in a grid. | Objects, one or two directions, spacing, quantity |
| Circular Pattern (body) | Solid > Create | Copies objects around an axis. | Objects, axis, angle or full circle, quantity |
| Pattern on Path | Solid > Create | Copies objects along a curve. | Objects, path, spacing method |
| Mirror (body) | Solid > Create | Copies objects across a plane or face. | Objects, mirror plane or face |
| Thread | Solid > Create | Adds cosmetic or fully modeled internal or external threads to a cylindrical face. | Cylindrical face, thread designation, modeled or cosmetic, internal or external, length |
| Press Pull | Solid > Create/Modify | Lets you click a face, edge, or profile directly and drag or type a value; the tool infers extrude, fillet, or offset behavior from what you pick. | Face, edge, or profile, distance, inferred operation |

### 1.2 Modify panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Fillet | Solid > Modify | Rounds edges by adding or removing material. | Edges or faces, radius (constant, variable, chord, or face fillet), tangent chain toggle |
| Chamfer | Solid > Modify | Bevels edges by adding or removing material. | Edges, distance(s) or distance and angle |
| Shell | Solid > Modify | Hollows a solid, leaving walls of a set thickness. | Faces to remove (optional), wall thickness, direction |
| Draft | Solid > Modify | Applies a manufacturing draft angle to faces. | Faces, pull direction or parting line, draft angle |
| Scale | Solid > Modify | Enlarges or shrinks a body. | Body, scale type (uniform or non-uniform), scale point, factor(s) |
| Combine | Solid > Modify | Joins, cuts, or intersects one body with another. | Target body, tool body, operation, keep-tools option |
| Offset Face | Solid > Modify | Shifts one or more faces inward or outward. | Faces, offset distance |
| Replace Face | Solid > Modify | Removes faces and extends or trims the body to new face locations. | Faces to remove, replacement faces |
| Split Face | Solid > Modify | Divides a face using a curve, plane, or surface. | Face, splitting tool |
| Split Body | Solid > Modify | Divides a body into two bodies. | Body, splitting tool (plane, face, or sketch curve) |
| Silhouette Split | Solid > Modify | Splits a body along its outline as seen from a chosen direction. | Body, view direction or plane |
| Align | Solid > Modify | Repositions geometry to match a point, line, plane, or coordinate system. | Geometry to move, target reference |
| Change Parameters | Solid > Modify | Opens the parameters dialog to add or edit named expressions. | Parameter name, expression or value, unit, comment |

### 1.3 Assemble panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| New Component | Solid > Assemble | Creates a new component to hold bodies for multi-part designs. | Name, empty or from selected bodies, position |
| Joint | Solid > Assemble | Connects two components that are not yet positioned relative to each other. | Two joint origins or geometry references, joint type (rigid, revolute, slider, cylindrical, pin-slot, planar, ball), offset and angle values, motion limits |
| As-Built Joint | Solid > Assemble | Connects two components that are already in place, capturing their current position. | Two components, joint motion type |
| Joint Origin | Solid > Assemble | Predefines a point, plane, and axis on a component to use later in a joint. | Point, plane, and axis references |
| Rigid Group | Solid > Assemble | Locks two or more components so they move as one. | Two or more components |

### 1.4 Construct panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Offset Plane | Solid > Construct | Makes a plane parallel to a face or plane at a set distance. | Base face or plane, offset distance |
| Plane at Angle | Solid > Construct | Makes a plane at an angle from an edge or axis. | Edge, axis, or line, angle |
| Tangent Plane | Solid > Construct | Makes a plane tangent to a curved face. | Curved face, orientation reference |
| Midplane | Solid > Construct | Makes a plane centered between two faces or planes. | Two faces or planes |
| Perpendicular Plane | Solid > Construct | Makes a plane perpendicular to a curve at a point. | Curve or edge, point or plane reference |
| Plane Through Two Edges | Solid > Construct | Makes a plane through two coplanar edges or axes. | Two edges or axes |
| Plane Through Three Points | Solid > Construct | Makes a plane through three points. | Three vertices or snap points |
| Plane Along Path | Solid > Construct | Makes a plane perpendicular to a path at a chosen distance along it. | Path curve, distance or ratio |
| User Coordinate System | Solid > Construct | Defines a custom origin and axis set for downstream use. | Origin point, axis references |
| Axis Through Cylinder/Cone/Torus | Solid > Construct | Makes an axis through the center of a round feature. | Cylindrical, conical, or toroidal face |
| Axis Perpendicular to Face | Solid > Construct | Makes an axis perpendicular to a face at a point. | Face, point |
| Axis Through Two Planes | Solid > Construct | Makes an axis where two planes intersect. | Two planes |
| Axis Through Two Points | Solid > Construct | Makes an axis through two points. | Two vertices or points |
| Axis Through Edge | Solid > Construct | Makes an axis along a straight edge. | One linear edge |
| Point at Vertex | Solid > Construct | Places a point at an existing vertex or snap point. | Vertex or snap point |
| Point Through Two Edges | Solid > Construct | Places a point at the intersection of two edges. | Two edges |
| Point Through Three Planes | Solid > Construct | Places a point where three planes meet. | Three planes or faces |
| Point at Center of Circle/Sphere/Torus | Solid > Construct | Places a point at the center of a round feature. | Circular, spherical, or toroidal face |
| Point at Edge and Plane | Solid > Construct | Places a point where an edge meets a plane. | Edge or axis, plane or face |
| Point Along Path | Solid > Construct | Places a point at a distance along a curve. | Path curve, distance or ratio |

### 1.5 Inspect panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Measure | Solid > Inspect | Reports distance, angle, area, or position between selected items. | Points, edges, faces, or bodies |
| Section Analysis | Solid > Inspect | Cuts a live view through the model to see inside it. | Cutting plane, offset or depth |
| Interference | Solid > Inspect | Reports overlapping volume between bodies or components. | Two or more bodies or components |
| Center of Mass | Solid > Inspect | Shows a glyph at the center of mass of selected objects. | Bodies or components |
| Curvature Comb Analysis | Solid > Inspect | Draws a comb plot of curvature along an edge. | Edge, comb density and scale |
| Curvature Map Analysis | Solid > Inspect | Colors a face by curvature to spot high and low areas. | Face or body, color scale |
| Draft Analysis | Solid > Inspect | Colors faces by draft angle for manufacturability. | Body, pull direction, angle thresholds |
| Environment Map Analysis | Solid > Inspect | Applies a temporary chrome look to check reflective surface quality. | Face or body |
| Isocurve Analysis | Solid > Inspect | Extracts UV curves from a surface to check its quality. | Surface, direction, spacing |
| Zebra Analysis | Solid > Inspect | Projects stripes on a surface to check curvature continuity. | Face or body, stripe direction and density |
| Accessibility Analysis | Solid > Inspect | Colors a body to show areas reachable from a chosen direction. | Body, tool axis or plane |
| Minimum Radius Analysis | Solid > Inspect | Colors concave faces by their minimum radius. | Body, radius threshold |
| Design Advice | Solid > Inspect | Runs automated checks on the design and reports issues. | Whole design (no manual input) |
| Fastener Stack Analysis | Solid > Inspect | Checks a fastener stack for length and clearance issues. | Selected fastener stack |
| Display Component Colors | Solid > Inspect | Colors each component differently to tell them apart. | Whole assembly (toggle) |
| Display Mesh Face Groups | Solid > Inspect | Shows or hides face groups on a mesh body. | Mesh body (toggle) |
| Find Similar Components | Solid > Inspect | Uses AI search to find geometrically similar parts in your hub. | Selected component |

### 1.6 Insert panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Canvas | Solid > Insert | Places a reference image in the canvas or on a plane. | Image file, placement plane or face, scale calibration |
| Decal | Solid > Insert | Places a sticker or label image on a flat or curved face. | Image file, target face, position, rotation, scale, opacity |
| Insert Mesh | Solid > Insert | Brings in an STL or OBJ mesh file as a mesh body. | Mesh file, placement, orientation, unit |
| Insert SVG | Solid > Insert | Imports an SVG file as sketch geometry. | SVG file, target sketch plane, scale |
| Insert DXF | Solid > Insert | Imports a DXF file as sketch geometry. | DXF file, target sketch plane, scale, layer mapping |
| Insert McMaster-Carr Component | Solid > Insert | Downloads a standard part model from the McMaster-Carr catalog (online only). | Part number or search term |

### 1.7 Select panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Window Selection Mode | Solid > Select | Default mode; drag direction sets window (fully enclosed) or crossing (touched) selection. | Drag box, direction |
| Freeform Selection Mode | Solid > Select | Draws a lasso instead of a box; direction sets enclosed versus crossing. | Freehand outline |
| Paint Selection Mode | Solid > Select | Click and drag across items to select each one under the cursor. | Drag path |
| Select by Name | Solid > Select | Finds bodies or components by typed name. | Search text, object type filter |
| Select by Boundary | Solid > Select | Uses a resizable 3D shape to capture objects inside or crossing it. | Boundary shape, size |
| Select by Size | Solid > Select | Filters bodies by bounding box dimensions. | Size thresholds |
| Invert Selection | Solid > Select | Flips the current selection to everything not selected. | Current selection |
| Selection Filters | Solid > Select | Restricts what object types can be picked. | Type checkboxes (bodies, faces, edges, vertices, sketches, components, joints, mesh) |
| Select Through | Solid > Select | Lets clicks pick geometry hidden behind other geometry. | Toggle |
| Selection Priority | Solid > Select | Locks picking to one object type at a time. | Priority choice (bodies, faces, edges, components) |
| Create Selection Set | Solid > Select | Saves a named group of objects for quick reselection. | Current selection, name |

## Part 2: Sketch Contextual Tab

### 2.1 Create panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Line | Sketch > Create | Draws connected straight segments, with inline tangent arcs on direction change. | Start point, end point, chainable |
| Rectangle | Sketch > Create | Draws a four-sided profile. Sub-types: two-point, center, three-point. | Corner or center points, dimensions or angle |
| Circle | Sketch > Create | Draws a circle. Sub-types: center-diameter, two-point, three-point, tangent. | Center and radius, or defining points, or tangent references |
| Arc | Sketch > Create | Draws a curved segment. Sub-types: three-point, center-point, tangent (continuing from an existing endpoint). | Three points, or center plus start and end, or a tangent handle |
| Polygon | Sketch > Create | Draws a regular polygon. Sub-types: circumscribed, inscribed, edge. | Center point and side count with a radius point, or one edge length and side count |
| Ellipse | Sketch > Create | Draws an ellipse. | Center point, major axis point, minor axis point |
| Slot | Sketch > Create | Draws a rounded-end slot. Sub-types: center-to-center, overall, center point, three-point arc. | Two center points and width, or overall length and width, or center and width, or three points |
| Spline | Sketch > Create | Draws a smooth curve. Sub-types: fit point, control point. | A series of points, degree setting |
| Conic Curve | Sketch > Create | Draws an elliptical, parabolic, or hyperbolic curve. | Two endpoints, a shape (Rho) value or a shoulder point |
| Text | Sketch > Create | Places editable text as sketch geometry. | Insertion point or path, string, font, size, style |
| Point | Sketch > Create | Places a single reference point. | Location click or coordinates |
| Project/Include | Sketch > Create | Brings existing edges, faces, or other sketch geometry into the active sketch. Related tools: Include, Intersect, Intersection Curve, Isoparametric Curve. | Edges, faces, bodies, or sketches to reference |

### 2.2 Modify panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Sketch Fillet | Sketch > Modify | Adds a tangent arc between two curves at their intersection. | Two curves, radius |
| Sketch Chamfer | Sketch > Modify | Cuts a straight bevel between two lines. Sub-types: equal distance, distance and angle, two distance. | Two lines, one or two distances and/or an angle |
| Trim | Sketch > Modify | Removes a curve segment up to its nearest intersections. | Hovered segment |
| Extend | Sketch > Modify | Extends a curve to its nearest intersection. | Hovered segment |
| Break | Sketch > Modify | Splits a curve into two or more pieces at an intersection. | Curve, break point |
| Offset | Sketch > Modify | Creates a copy of curves at a set distance. | Source curves, distance, side |
| Sketch Scale | Sketch > Modify | Resizes selected sketch geometry. | Geometry, scale point, factor |
| Blend Curve | Sketch > Modify | Creates a smooth transition curve between two curve endpoints. | Two endpoints, continuity (tangent or curvature) |
| Move/Copy | Sketch > Modify | Moves or duplicates sketch geometry. | Geometry, move vector or rotation, copy toggle |
| Mirror | Sketch > Modify | Copies geometry across a line. | Geometry, mirror line |
| Rectangular Pattern | Sketch > Modify | Copies geometry in a grid. | Geometry, one or two directions, spacing, quantity |
| Circular Pattern | Sketch > Modify | Copies geometry around a point. | Geometry, center point, angle or full circle, quantity |
| Change Parameters | Sketch > Modify | Opens the same parameters dialog as the Solid tab. | Parameter name, expression or value, unit |

### 2.3 Constraints panel

| Command | Group | What it does | Inputs |
|---|---|---|---|
| Coincident | Sketch > Constraints | Locks two points, or a point and a curve, together. | Two entities |
| Collinear | Sketch > Constraints | Forces two lines to share one straight line. | Two lines |
| Horizontal/Vertical | Sketch > Constraints | Locks a line, or two points, to the horizontal or vertical axis. | One line or two points |
| Parallel | Sketch > Constraints | Forces two lines to stay parallel. | Two lines |
| Perpendicular | Sketch > Constraints | Forces two entities to meet at 90 degrees. | Two entities |
| Equal | Sketch > Constraints | Forces two similar entities to match in size. | Two entities |
| Tangent | Sketch > Constraints | Forces a curve to touch another entity without crossing it. | Two entities |
| Midpoint | Sketch > Constraints | Locks a point to the midpoint of another entity. | Point, entity |
| Concentric | Sketch > Constraints | Forces circles, arcs, or ellipses to share a center. | Two or more entities |
| Symmetry | Sketch > Constraints | Forces two entities to mirror each other across an axis. | Two entities, symmetry line |
| Curvature | Sketch > Constraints | Forces a spline to blend into another curve with matched curvature (G2). | Spline, adjoining curve |
| Fix/Unfix | Sketch > Constraints | Locks or unlocks an entity's position and size. | One entity |
| Dimension | Sketch > Constraints | Sets an exact length, distance, radius, diameter, or angle value. | One to three references, typed value or expression |

### 2.4 Sketch Palette

| Option | Group | What it does | Inputs |
|---|---|---|---|
| Linetype | Sketch Palette | Changes selected geometry to a different line style. | Geometry, line style |
| Construction | Sketch Palette | Converts geometry to non-solid reference construction geometry. | Geometry (toggle) |
| Centerline | Sketch Palette | Converts a line to a centerline for use with symmetric features. | Line (toggle) |
| Look At | Sketch Palette | Rotates the camera to face the active sketch plane directly. | None |
| Sketch Grid | Sketch Palette | Shows or hides the background grid. | Toggle |
| Snap | Sketch Palette | Snaps new points to the grid while sketching. | Toggle |
| Slice | Sketch Palette | Temporarily cuts bodies where they cross the sketch plane, for tracing. | Toggle |
| Show Profile | Sketch Palette | Shades closed profiles blue. | Toggle |
| Show Points | Sketch Palette | Shows or hides sketch points. | Toggle |
| Show Dimensions | Sketch Palette | Shows or hides dimension labels. | Toggle |
| Show Constraints | Sketch Palette | Shows or hides constraint glyphs. | Toggle |
| Show Construction Geometries | Sketch Palette | Shows or hides construction geometry. | Toggle |
| Show Projected Geometries | Sketch Palette | Shows or hides projected and included geometry. | Toggle |
| 3D Sketch | Sketch Palette | Lets new geometry leave the 2D sketch plane into 3D space. | Toggle |

## Part 3: Viewport and Interaction Model

| Interaction | Category | What it does |
|---|---|---|
| Orbit | Navigation | Rotates the camera around the model. Default mapping: hold Shift and drag the middle mouse button. |
| Pan | Navigation | Slides the view sideways without rotating. Default mapping: drag the middle mouse button. |
| Zoom | Navigation | Moves the camera closer or farther, centered on the cursor. Default mapping: scroll the mouse wheel; a Zoom Window drag-box is also available. |
| Look At | Navigation | Aligns the camera to face a chosen planar face or sketch plane straight on. |
| Fit/Zoom All | Navigation | Frames all visible geometry in the current view. |
| ViewCube | Navigation | A widget fixed in the top right corner. Click a face, edge, or corner to snap to that standard view. Drag it to free-orbit. Click the house icon for Home view. Right-click it for view options. |
| Click select | Selection | Left-click one entity in the canvas, browser, or timeline to select only it, with hover highlighting the entity under the cursor first. |
| Ctrl/Cmd+click | Selection | Adds or removes the clicked entity from the current selection. |
| Shift+click | Selection | Adds the clicked entity to the current selection without removing others. |
| Window select | Selection | Drag left to right; selects only entities fully enclosed by the box. |
| Crossing select | Selection | Drag right to left; selects entities the box touches or encloses. |
| Selection filters | Selection | A menu that limits which entity types (bodies, faces, edges, vertices, sketches, components) can be picked at all. |
| Right-click marking menu | Interaction | Right-clicking an entity or empty canvas opens a context menu of commands relevant to the current selection. |
| Object snaps in sketches | Sketching | While drawing, the cursor snaps to endpoints, midpoints, centers, quadrants, on-curve points, intersections, and grid points, shown with glyphs and alignment guide lines. |
| Dimension entry | Sketching | Click Dimension, pick one to three references, place the line, then type a value or expression into an inline box. Tab moves between multiple fields when several are shown at once. |
| Timeline | Model history | A horizontal strip at the bottom of the canvas listing every parametric feature in order as an icon. Dragging the marker rolls the model back. Right-click a feature to edit, suppress, or delete it. Dragging icons reorders features. |
| Browser | Model structure | A tree panel on the left listing the document structure: root component, sub-components, bodies, sketches, the Origin folder, joints, and canvases. Checkboxes toggle visibility; right-click gives per-item commands. |

### Extras beyond the core ribbon

Two Anvil features do not map to a native Solid or Sketch tab command, but have Fusion equivalents elsewhere:

| Feature | Fusion equivalent | What it does | Inputs |
|---|---|---|---|
| QR Code | QR Code Creator (official Autodesk Store add-in) | Generates 3D QR code geometry from encoded text. | Encoding mode (single, sequence, vCard, Wi-Fi), text to encode, style, segment size and spacing, frame |
| Texture (relief pattern) | Texture Extrude (Mesh workspace) | Extrudes a grayscale image onto a surface as physical relief geometry. | Target face, source image, depth, tiling |

## Anvil Status

Status is Done, Partial, or Missing. Partial means the command exists on the ribbon but returns "unsupported." Anything not listed in Anvil's feature set is Missing, even common items like orbit and the timeline UI, because they were not confirmed as shipped; this is a completeness rule, not a claim that they are absent.

### Solid > Create

| Command | Status | Note |
|---|---|---|
| Extrude | Done | New body only; join, cut, intersect, and new-component operations not yet supported |
| Revolve, Sweep, Loft | Done | |
| Box, Cylinder, Sphere, Torus, Coil, Pipe | Done | |
| Mirror (body), Rectangular Pattern, Circular Pattern | Done | |
| Press Pull | Done | Face extrude to a new body |
| Pattern on Path, Rib, Web, Emboss, Boss, Snap Fit, Thicken, Create Base Feature, Create Form, Create Mesh, Create PCB, Thread | Missing | |

### Solid > Modify

| Command | Status | Note |
|---|---|---|
| Fillet, Chamfer | Done | Straight edges between flat faces, outside and inside corners |
| Combine, Hole | Done | CSG booleans; Hole has counterbore |
| Shell | Done | Wall thickness, one open face picked in the view (a plane reference); closed shells keep a hollow inside |
| Scale, Move/Copy | Done | |
| Split Body | Done | Plane only; face and sketch-curve splitting not supported |
| Change Parameters, Appearance, Physical Material, Bill of Materials, Delete, Compute All, undo/redo | Done | |
| Draft, Offset Face, Replace Face, Split Face, Silhouette Split, Align | Missing | |

### Solid > Assemble

| Command | Status | Note |
|---|---|---|
| New Component, Joint, As-Built Joint, Joint Origin, Rigid Group | Missing | |

### Solid > Construct

| Command | Status | Note |
|---|---|---|
| Offset Plane, Plane at Angle, Midplane, Plane Through Three Points | Done | |
| Tangent Plane, Perpendicular Plane, Plane Through Two Edges, Plane Along Path, User Coordinate System, all axis tools, all point tools | Missing | |

### Solid > Inspect

| Command | Status | Note |
|---|---|---|
| Measure, Center of Mass, Interference | Done | |
| Section Analysis | Done | Axis-aligned plane with offset |
| Everything else in Inspect | Missing | |

### Solid > Insert

| Command | Status | Note |
|---|---|---|
| Insert Mesh (STL) | Done | |
| QR Code | Done | Bundled generator, not an installed add-in |
| Insert DXF | Done | Into the active sketch |
| Canvas, Decal, Insert SVG, Insert McMaster-Carr Component | Missing | |

### Solid > Select

| Command | Status | Note |
|---|---|---|
| Click select with hover highlight | Done | |
| Ctrl+click multi-select | Done | |
| Window select | Done | Model view |
| Window/Crossing select | Done | Sketch view |
| Right-click marking menu | Done | |
| Paint Selection Mode, Freeform Selection Mode, Select by Name, Select by Boundary, Select by Size, Invert Selection, Selection Filters, Select Through, Selection Priority, Selection Sets | Missing | |

### Sketch > Create

| Command | Status | Note |
|---|---|---|
| Sketch on datum plane or face | Done | |
| Line | Done | Includes a Midpoint Line variant |
| Rectangle (2-pt, center, 3-pt) | Done | |
| Circle (center, 2-pt, 3-pt) | Done | Tangent circle Missing |
| Arc (3-pt, center, tangent) | Done | |
| Polygon (circumscribed, inscribed, edge) | Done | |
| Ellipse | Done | |
| Slot (center-to-center, center point) | Done | Overall and three-point arc slot Missing |
| Spline | Done | |
| Text | Done | Bundled font, one body per glyph |
| Point | Done | |
| Project | Done | |
| Conic Curve, Include, Intersect, Intersection Curve, Isoparametric Curve | Missing | |

### Sketch > Modify

| Command | Status | Note |
|---|---|---|
| Fillet, Trim, Extend, Offset, Mirror, Move/Copy, Sketch Scale, Rectangular Pattern, Circular Pattern | Done | |
| Change Parameters | Done | Shared with the Solid tab |
| Chamfer (sketch) | Done | |
| Break, Blend Curve | Missing | |

### Sketch > Constraints

| Command | Status | Note |
|---|---|---|
| Coincident, Collinear, Horizontal, Vertical, Parallel, Perpendicular, Equal, Tangent, Midpoint, Concentric, Symmetry, Fix | Done | |
| Point on curve | Done | Implemented as its own constraint rather than folded into Coincident |
| Dimension (length, distance, radius, angle) | Done | Click-to-place with inline label editing |
| Curvature (G2) | Missing | |

### Sketch Palette

| Option | Status | Note |
|---|---|---|
| Sketch Grid, Snap | Done | |
| Object snaps (point, midpoint, centre, quadrant, on-curve) | Done | |
| Construction, Centerline, Look At, Slice, all Show toggles, 3D Sketch, Linetype | Missing | |

### Viewport and Interaction

| Interaction | Status | Note |
|---|---|---|
| Click select with hover highlight, Ctrl+click multi-select, right-click marking menu, dimension entry with inline label editing, object snaps in sketches | Done | |
| Window/Crossing select | Done | Both in sketches, window-only in the model view |
| Orbit, Pan, Zoom mouse mapping, ViewCube, Selection Filters, Timeline (interactive UI), Browser tree | Missing | Not confirmed shipped; underlying parametric history exists through Compute All and Change Parameters |

### Extras and file I/O

| Feature | Status | Note |
|---|---|---|
| QR Code, Texture (relief pattern) | Done | |
| STL export | Done | |
| STL per feature | Done | |
| 3MF export | Done | One object per feature |
| Business card sample | Done | Sample project, not a ribbon command, included for tracking |

## References

- [Fusion Help: Fusion interface (desktop)](https://help.autodesk.com/view/fusion360/ENU/?guid=GS-THE-FUSION-INTERFACE)
- [Fusion Help: Solids from sketches](https://help.autodesk.com/cloudhelp/ENU/Fusion-Model/files/SLD-CREATE-SOLID-FROM-SKETCH.htm)
- [Fusion Help: Modify tools for solid bodies](https://help.autodesk.com/cloudhelp/ENU/Fusion-Model/files/SLD-MODIFY-SOLID-BODY.htm)
- [Fusion Help: Construction geometry](https://help.autodesk.com/cloudhelp/ENU/Fusion-Model/files/SLD-CONSTRUCT-TOOLS.htm)
- [Fusion Help: Analysis (Inspect) tools](https://help.autodesk.com/cloudhelp/ENU/Fusion-Model/files/SLD-INSPECT-TOOLS.htm)
- [Fusion Help: Selection in Fusion](https://help.autodesk.com/view/fusion360/ENU/?guid=SLD-SELECTION)
- [Fusion Help: Assembly relationships (joints)](https://help.autodesk.com/cloudhelp/ENU/Fusion-Assemble/files/ASM-JOINTS.htm)
- [Fusion Help: Create bodies (primitives)](https://help.autodesk.com/view/fusion360/ENU/?guid=GUID-4A0FC166-2FBE-47AE-8F87-52178B05441F)
- [Fusion Help: Pattern reference](https://help.autodesk.com/cloudhelp/ENU/Fusion-Model/files/SLD-REF-PATTERN.htm)
- [Fusion Help: Sketch modification tools](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/SKT-SKETCH-MODIFY-TOOLS.htm)
- [Fusion Help: Mirrors and patterns in sketches](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/SKT-SKETCH-CREATE-MIRRORS-PATTERNS.htm)
- [Fusion Help: Constraints in sketches](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/SKT-CONSTRAINTS.htm)
- [Fusion Help: Sketch Palette reference](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/GUID-4183A4B7-E002-4396-AD5A-7FF3C8B2F33A.htm)
- [Fusion Help: Slots in sketches](https://help.autodesk.com/view/fusion360/ENU/?contextId=SKT-SKETCH-CREATE-SLOTS)
- [Fusion Help: Lines in sketches](https://help.autodesk.com/view/fusion360/ENU/?guid=SKT-SKETCH-CREATE-LINES)
- [Fusion Help: Dimensions in sketches](https://help.autodesk.com/view/fusion360/ENU/?guid=SKT-SKETCH-CREATE-DIMENSIONS)
- [Fusion Help: Create solids with Press Pull](https://help.autodesk.com/view/fusion360/ENU/?guid=GUID-02F9ADA3-7556-42A9-8AD1-552728D537AB)
- [Fusion Help: Add a decal](https://help.autodesk.com/view/fusion360/ENU/?guid=SLD-INS-DECAL)
- [Fusion Help: Insert canvas (image)](https://help.autodesk.com/view/fusion360/ENU/?guid=SLD-INSERT-CANVAS)
- [Fusion Help: Insert a mesh body](https://help.autodesk.com/view/fusion360/ENU/?guid=MESH-INSERT-MESH)
- [Autodesk: Unable to Pan, Zoom, or Orbit with mouse or trackpad in Fusion](https://www.autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/Unable-to-use-Mouse-Wheel-to-activate-Orbit-and-Pan-in-Fusion-360.html)
- [Fusion Blog: How to Set Your Pan, Zoom, and Orbit Controls](https://www.autodesk.com/products/fusion-360/blog/quick-tip-pan-zoom-orbit-preferences/)
- [Fusion Blog: Custom Texture Patterns in Autodesk Fusion](https://www.autodesk.com/products/fusion-360/blog/custom-texture-patterns-in-autodesk-fusion/)
- [Autodesk App Store: QR Code Creator for Fusion](https://apps.autodesk.com/FUSION/en/Detail/Index?id=5291506789765828003&appLang=en&os=Mac)