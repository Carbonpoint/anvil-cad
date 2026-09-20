# Progress log

Newest first. One line per work session or milestone.

- 2026-09-20: Material Design 3 theme (crates/anvil-ui/src/theme.rs, docs/THEME.md): seed #3B6EA5, surface containers, M3 shape and state layers, a light and a dark scheme with Light/Dark/System in the settings window; the 3D view, the sketch overlay and the panels take their colours from the scheme. Contrast of every text pair is at least 4.5:1 by the WCAG formula. The rendered look is unverified: this box has no display.

- 2026-09-20: GPU viewport verified by the user on Intel Iris Xe (Windows). It was no faster because the software id and depth pass still ran every frame in every pane: now it runs only in the pane under the pointer and only when the view or the model changed (Camera::fingerprint plus Scene::id). The performance overlay draws in one pane. New View > Settings > Benchmark: it spins the view for 60 frames with the GPU and 60 without, then reports the middle and worst frame time with the triangle count.

- 2026-09-20: Optional GPU viewport (crates/anvil-ui/src/gpu.rs: egui_glow paint callback, own FBO, GL 3.3 core or GLES 300, same Projector maths, per body colour, highlights, section clip, edges; picking still uses the CPU id buffer at half resolution). It is OFF by default because no display or GPU exists on the build machine, so the GL path has never run: turn it on at View > Settings > GPU viewport. GPU numbers in the performance overlay (nvidia-smi, Linux amdgpu and i915 sysfs, GL_RENDERER; 'not available' elsewhere).

- 2026-09-20: Rollback bar in the Part Navigator (Document::rollback, one regenerate for any number of held back features; drag the bar, the | button on a row, or the < > buttons; a new feature is inserted AT the bar and the bar moves past it). Camera: box views are orthographic, the four panes start Front, Right, Top, Perspective, each pane's name is a menu of the eight views, ribbon view commands act on the pane under the pointer, Ortho/Persp toggle, and View > Camera has X up, Y up, Z up and Free orbit (trackball by quaternion).

- 2026-09-20: Viewport batch: drag arrow for Extrude and Press Pull (drag_handle.rs, Feature::drag_plane, Document::edit_feature_quiet so a drag is one undo step; a distance that uses a named expression is not dragged), four pane view (View > Camera > Four views: Front, Right, Top, angled; each pane keeps its own camera and framebuffer, shortcuts belong to the pane under the pointer), View > Settings > Invert scroll. Tests: four_panes_split_the_view, extrude_and_press_pull_get_an_arrow.

- 2026-09-19: Sketch regions follow the planar arrangement (overlapping circles give three pickable regions; touching picks merge into one body). Ribbon icons drawn in code (icons.rs, every command has one; dense sketch rows are icon only with tooltips) and an anvil logo (window icon, ribbon corner, docs/logo.svg).

- 2026-09-19: Part Navigator rows no longer spread over the panel height (a right_to_left layout took all the height left). Tangent between two circles or arcs (TangentCircles, inside or outside picked from the geometry). Examples tab (Parts, Workbook, Casting samples) instead of File > Samples. Other ribbon tabs stay reachable in sketch mode; a solid command finishes the sketch first. View > Settings > Performance shows CPU and memory (computer and Anvil, via sysinfo), fps and frame time.

- 2026-09-19: Laptop screens: with a kettle sample loaded, the expressions panel and the navigator grew to fit their content and left the 3D view 0 px high at 1366x768. Panels are now capped (expressions 25% of the height and scrolls, side panels 22% of the width, properties scroll), long feature names and status text are truncated, fit accounts for the view aspect, and zoom works by touchpad pinch, Ctrl+scroll, + and - keys, Home fits. Headless layout test `samples_leave_room_for_the_view_on_a_laptop` runs the whole window at 1024x640, 1280x720 and 1366x768.

- 2026-09-19: Sketches stay visible after Finish Sketch (model view overlay, faint where a body hides them; sketches a feature uses are hidden unless selected or "Used sketches" is on). Region picking: Extrude and Revolve have a Regions parameter; with the feature selected, its sketch regions are filled in the view (orange = used) and a click adds or removes one. Every profile bounds a region, so a circle inside a face outline can be extruded alone. Revolve now subtracts the holes of a region. New feature refs start at the latest sketch. Open: true arrangement regions for overlapping curves (profiles are still closed loops), Regions on Sweep and Loft.

- 2026-09-14: Docker: anvil-cli headless tool (card, fonts), multi-stage Dockerfile (build, test, runtime), CI job that builds the image and makes a card in it. Docker is not installed on the WSL box or lab hosts, so the image is verified in GitHub Actions.

- 2026-09-14: Business card with LinkedIn QR code published as a template: export_qr_card example (name and URL on the command line), docs section with preview.


- 2026-09-14: Text gains letter spacing; export and render helpers (render_view, render_top). Design lesson: build printable multi-part parts from extruded profiles with holes rather than chains of booleans, which keeps meshes watertight.



- 2026-09-11: Fonts: searchable Font dropdown (bundled plus installed fonts), bundled Archivo Black and Liberation Sans Bold, Thicken strokes, stroke-width printability note. Business card defaults to Archivo Black.

- 2026-09-11: Workbook of six CSWA-style parts (docs/WORKBOOK.md, anvil_io::workbook, File > Samples WB 1-6). Multi-agent bug hunt on them; 12 fixes (see WORKBOOK.md findings). Features: CSG booleans with T-junction repair, Extrude/Revolve/Text join-cut-intersect, Hole, Combine, edge Fillet and Chamfer with edge picking, face placement for Hole/Text/QR/Texture, reference remapping and navigator reorder, selection filter, section view, interference, view buttons, hover labels, expression-driven sketch dimensions, sketch chamfer and tangent arc, DXF import. Builds run on stalker via scripts/remote-test.sh.
- Open: ear clipping inverts one triangle in some glyphs (notch in R). Test glyph_cap_triangles_all_face_the_same_way is ignored until fixed; a diagonal-split fallback fixed it but shifted boolean volumes in workbook 01.
- Next: Shell and Draft (offset surfaces), timeline rollback, loft twist on rotated planes, SVG import, construction axes and points, persistent face and edge references.

- 2026-09-10: Business card batch, part A: kernel faces with holes, Text (bundled DejaVu Sans), QR Code, Texture, Press Pull, nested sketch loops become holes, rounded rectangle builder, STL per feature, 3MF multi-object export, Business Card sample (anvil_io::business_card). 50 tests. Local builds capped at 4 jobs (.cargo/config.toml) after WSL crashes.
- 2026-09-10: Part B done: face hover and click selection, Press Pull from a face (Q), Sketch on selected face, right-click menus in model and sketch modes, Ctrl multi-select, Shift+drag box select (model) and drag box select (sketch, window or crossing), Dimension tool by clicking with label editing, midpoint/centre/quadrant/curve snaps with markers, 3MF and STL-per-part export buttons, Business card sample button. Remote test script scripts/remote-test.sh (stalker). 50 tests.
- 2026-09-10: docs/FUSION_PARITY.md added: every Fusion Solid and Sketch command with status.
- Next: sketch dimension values as expressions, edge picking for per-edge fillet, units in sketch dimensions, ViewCube.

- 2026-09-10: Batch 2 (Solid tab parity) done: Coil, Pipe, Insert Mesh (STL), Split Body, Plane 3 Points, Midplane, Appearance colour, Physical Material with mass, Center of Mass marker, Bill of Materials, Delete, Compute All, Units mm/in. 44 tests.
- Next: edge picking for per-edge fillet, dimension expressions, draggable dimension labels, Draft/Thicken/Rib (need kernel), Named views, Section view.
- 2026-09-10: Batch 1 (Sketch tab parity) done: ellipse, spline, rectangle/circle/arc/polygon/slot variants, fillet, trim, extend, offset, mirror, move/copy, scale, patterns, collinear, smart Dimension (D), project edges (auto on faces), palette toggles, L/R/C keys. 41 tests.
- 2026-09-09: Sketch editor, pick a plane or face, z-buffer viewport, Solid tab with primitives, sweep, loft, patterns, mirror, move, scale, planes. Commit a1ef1aa.
- 2026-09-09: Initial scaffold, literature reviews, private repo. Commit c199927.
