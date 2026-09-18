# nTop capabilities: what it does, how its demos loop, and what Anvil should build next

Written 2026-09-17 from public nTop material, press and reviews; numbered
references at the end. Statements marked *inferred* are my reading, not
something nTop documents.

## 1. Core concepts

**Implicit body.** A part is a signed distance field: zero on the boundary,
negative inside, positive outside [2]. Union is the minimum of two fields,
intersection the maximum, offset subtracts a constant [2]. nTop's pitch is
that these operations "never fail" and stay cheap as feature count grows,
where B-reps become "impractical ... with thousands of faces and impossible
... with tens of thousands" [3].

**Field.** Any function of space: scalar (distance, stress, temperature),
vector (displacement), or a body's own distance. A stress field from FEA or
a pressure map can be sampled anywhere and fed into a thickness input
[2][15].

**Blocks and notebook.** A notebook is a visual program: blocks, each a
function with typed inputs and one output, whose outputs feed other
blocks [1]. The 3.0 review describes scalars edited "at the click and drag
of a mouse" with "instantaneous feedback" [4]. That it is a DAG with
dependency-ordered recompute is *inferred* from the input/output structure.

**Parametrisation and re-run.** Variables are promoted to Inputs ("Make
model input") and a block is dragged to Outputs; the notebook then compiles
to a Custom Block reusable in other notebooks [5]. The same Inputs and
Outputs are what nTop Automate reads and writes as JSON headless [6].

**Field-driven design.** The marketing term for any geometric parameter
(wall thickness, cell size, blend radius, rib height) being a field rather
than a number. The canonical block is Ramp: In min/In max on a scalar
field, Out min/Out max on the output, a continuity choice; the driving
field can be a body (distance), a plane, or a point map from simulation
[14].

## 2. Capability list

**Lattices** [8][9]. Three-step model: unit cell (30+ built in: graph,
TPMS, custom), cell map (rectangular, cylindrical, spherical, from a quad
mesh, from a CAD body's UVW), then thickness and cell size, both of which
can be fields (a pressure map varying cell size is the documented example).
Lattice Warping for conformal lattices; Surface Lattice from CAD Body,
Delaunay and Voronoi volume lattices. Beam filters (length, angle,
connectivity, thickness), Collapse Vertices, Remove Open Beams, Merge
Lattices. Claim: 50,000+ cells rebuild in 1 to 2 seconds, 50x the previous
generation.

**Shells, infill, ribs, textures.** Shell, Thicken Body, Offset; shell plus
lattice infill is a standard workflow and an optimisation target [11].
Ribs: Graph by Planar Projection (plane, cylinder or tube guides) maps a
2D unit cell onto a surface and Ribs from Graph extrudes it; Structural,
Voronoi and Honeycomb Ribbing blocks; rib height can be a field [10].
Texturing and perforation by boolean lists of primitives [22].

**Booleans and blends.** Union/Subtract/Intersect with Sharp, Rounded or
Chamfered blends; 5.0 special-cases lists of primitives for 100x to 1000x
speed on 10k to 100k items [23]. Smoothen Body removes mesh-scale artefacts
from topology optimisation output [13].

**Topology and field optimisation.** Built-in SIMP [11]. Output is a
density field; threshold and smooth it into an implicit body, or let it
drive lattice thickness directly [12]. Field Optimisation (beta from 3.45)
optimises parameters at every point through parametric FE components
(lattices, variable shells, shell plus infill) with homogenised materials
[11]. Nastran .op2 topology results import [16].

**Field operations.** Ramp [14]; Field from Point Map with interpolation
and extrapolation options; Von Mises and Displacement Point Maps from a
Static Analysis, vector fields exposing Length, X, Y, Z [15]; point maps
as CSV [16]; a Field Viewer with probe and contours [14].

**Meshing.** Mesh from Implicit Body is a voxel-grid method: tolerance sets
the voxel size, feature size filters small features; it "tends to remove
sharp edges" [19][20]. Sharpen Mesh reconstructs edges; Remesh Surface and
Parallel Remesh Surface control edge length, growth rate, feature angle
and chord height [19][23]. Mesh from Implicit by AT (5.27) is adaptive
tessellation with lower memory at fine tolerance [21]. Tetrahedral volume
meshes; FE meshes with linear or quadratic elements [20].

**Export** [16][17]
- Mesh: STL, OBJ, PLY, 3MF (3MF with beam extension for lattices [40]).
- CAD: STEP AP242, Parasolid, IGES, by reconstruction: feature-aligned
  NURBS that "only works reliably on pretty simple models, such as those
  produced by topology optimization", or G1 patch fitting for medium
  complexity; above about 10k beams or 1k TPMS cells nTop recommends a
  Simplified Body (envelope plus density bodies for mass properties) [17].
- Slices: Slice Body (layer height, feature size, frame), Slice Body by
  Marching Squares, Hatch, Merge Slice Stacks; CLI, CLF, Renishaw and EOS
  CLI, ABF, CLS, SLC, SSL, PNG stacks [16][18].
- FE: Abaqus .inp, Ansys .cdb, Fluent .msh, LS-DYNA, Nastran, Patran,
  CGNS, UNV; mesh, material or whole analysis [16][20].
- Implicit: the .implicit file, lossless and "up to 99%" smaller than a
  mesh, read by EOSPRINT (sliced directly by Marching Squares or Dual
  Contouring), Materialise Magics, and prototype links to NX and
  Intact.Simulation; nTop Core is the library partners embed [18][36][37].
  VDB voxel grids import and export [16].

**Simulation.** Native linear static, modal, buckling and thermal FE;
results import from Ansys .rst/.rth, Nastran .op2, Fluent .cas/.dat [16].
nTop Fluids (2025-05-28, from the cloudfluid acquisition): GPU lattice
Boltzmann on a voxel mesh generated from the implicit "without
translation", LES, incompressible internal flow first, conjugate heat
transfer on the roadmap, claimed 100x to 1000x faster than CPU solvers
[24][25]. Coupling to Fluent, CFX, Discovery, Abaqus, Star CCM+ and
Luminary is by mesh export and results import, not a live link
[26][30][33].

**Automation.** nTop Automate (formerly nTopCL): `nTopCL.exe -j input.json
-o out.json notebook.ntop`, driven from Python `subprocess` or MATLAB, with
a Linux build and container image [6][7]. There is no in-process Python
API; every Python article wraps the CLI (*documented by absence*).

## 3. The aircraft demos and other design loops

### 3.1 Parametric wing with field-driven airfoil (video) [26]
1. A wing built entirely in nTop where "airfoil shape, camber, and
   thickness vary continuously along the span". Instead of "hard-coding a
   NACA designation, each geometric variable (like camber and thickness) is
   exposed and editable across space": section parameters are fields of
   span position.
2. The skin drives the internals: as the outer shape changes "spars
   re-align automatically", "topology of ribs adapts to space constraints",
   and prototyping slots are cut, "in real time".
3. The page stops at "ready for analysis, optimization, or physical
   prototyping"; no CFD is run.

*Inferred* implementation: a 2D airfoil implicit evaluated in the chord
frame of each span station with section parameters as fields of span;
ribs and spars are booleans of thin slabs against the skin. This is the
loft-as-field structure Anvil's `wing.rs` already uses.

### 3.2 Design space exploration with Luminary CFD (blog) [27]
The most complete public pipeline:
1. Five parameters: NACA 4-digit airfoil, sweep 0 to 30 deg, taper 0.1 to
   1.0, winglet bend 0 to 25 deg, root chord to span 0.1 to 0.75.
2. Latin Hypercube Sampling, 200 samples, one JSON file each.
3. nTop Automate runs the notebook per sample: sections, planform,
   winglet, meshing, and a CGNS export with wall and farfield tagged.
4. Luminary Cloud's Python SDK meshes the volume and runs angle of attack
   sweeps at Mach 0.7.
5. L/D and CL max are parsed from stdout into CSV: 3400 CFD runs in under
   6 hours.
6. Feedback is offline: the author proposes a surrogate on the results. No
   optimiser closes the loop in this article.

### 3.3 Aircraft webinar series (2025)
*Analysis* (2025-09-08) [29]: sizing equations drive the geometry; adaptive
tessellation gives meshes for Star CCM+ and Fluent "in minutes"; nTop
Fluids does external aerodynamics, giving pressure and drag. *Parametric
optimisation* (2025-10-22) [28]: "built-in optimization algorithms" (not
named) on parametric implicit models, with nTop Fluids running each
iteration "without meshing bottlenecks"; variables are sweep, aspect
ratio, taper, fuselage and inlet; objectives are fuel capacity and
rigidity.

### 3.4 JetZero blended wing body (2026-06-01) [30]
One notebook encodes the whole aircraft. NVIDIA NemoClaw on GCP sends
parameter sets to nTop, routes geometry to AVL (panel method) and Flow360
(CFD), and returns structured results overnight. nTop's role is geometry
only.

### 3.5 Heat exchanger with Ansys CFX (white paper, 2020) [31]
1. Shell and domes from Creo as Parasolid, converted to implicits.
2. Cylindrical-coordinate gyroid core with circumference count, radius and
   height periods, cell size and wall thickness as parameters.
3. Fluid domains by Boolean Subtract of core and other fluid from the
   envelope; baffles by Boolean Intersect of thickened CAD faces; union
   with fillet to the shell.
4. Fluid volume meshes exported, refined in ICEM, solved in CFX.
5. Loop: CFD showed where energy went; the designer changed the radius
   period to push oil into the gyroid for a further 12% on HTC. A human
   reads a contour plot, edits a parameter, re-exports.

### 3.6 CFD field driving plumbing geometry [32]
Heat exchanger inlet and outlet plumbing regenerated from imported CFD
fields: pressure drop down 38% and 20%, average HTC up 34%. Solver and
field are not named.

### 3.7 DoE and FEA-driven parts [33][34][35][12]
With Ansys Discovery, a DoE table (lattice type, cell count, aspect ratio)
is imported into the notebook, nTop exports a .msh per row with boundary
conditions, and a Discovery script batch-runs CFD to PNG and CSV. The
lattice lightweighting video does the same for thickness, cell type and
size with Python and nTop Automate. For brackets, a static analysis gives
a Von Mises or displacement point map; Field from Point Map plus Ramp
drives shell thickness or lattice density. On a swing arm a topology
result drives a rib grid, then draft is added for casting.

**Pattern.** Every public loop is: parameters in JSON, nTop regenerates
geometry and a boundary-tagged mesh, a solver runs, scalars go to CSV, and
a human or outer optimiser picks the next parameters. Fields flow back
only as point maps driving thickness or density, never as shape gradients.

## 4. Why it is fast and robust, and where it is weak

**Fast and robust.**
- Distance-field booleans and offsets cannot produce invalid topology; cost
  is evaluation, not face counting, and meshing is deferred to export
  [2][3][22].
- GPU viewport rendering since 3.0, "ten and 100 times faster" [4]; nTop
  Fluids is GPU LBM [24].
- The 5.0 kernel (2024-06-24) added sharper edges, thin-wall precision and
  the primitive-list boolean fast path [22][23]; 5.27 added adaptive
  tessellation [21]; remeshing is multi-threaded [23].
- The .implicit file lets slicers evaluate the field per layer: sub-megabyte
  files where meshes would be gigabytes [36].
- How the kernel evaluates (interval arithmetic, sparse voxels, compiled
  expressions) is not public; a voxel grid plus sharpening is the only
  mechanism documented [19][20].

**Limitations.**
- No exact B-rep: CAD export is the reconstruction described in section 2,
  and analytic faces for mating and drawings are gone once a body is
  implicit [17]; reviewers flag this for companies that need a solid and a
  drawing to release a part [4].
- Meshing removes sharp edges unless sharpened; users report large meshes
  are slow, sometimes fail, and need top-end hardware [19][38].
- The block interface "may scare off the less adventurous" [4].
- Cost: no list price; third-party estimates put a commercial seat at
  10,000 to 20,000 USD per year [39]. The free EOSPRINT plugin reads only
  .implicit files, which nTop alone writes [18].
- Field Optimisation evaluates stress on a homogenised background mesh,
  not the final geometry [11].

## 5. What Anvil should build next, in order

Anvil today: `Field` trait, sphere and box, booleans, smooth union, offset,
shell, skin, TPMS sheets, beam lattices, graded thickness from ramp and
radial fields, cylindrical warp, sampled SDF from any closed mesh, surface
nets, the Lattice fill feature and `anvil-cli lattice`. It also has
`wing.rs` (NACA 4, lofted wing, fuselage) and `aero.rs` (lifting line,
Nelder Mead on taper and twist), so the aircraft demo's geometry side is
already covered.

1. **Field from point map (CSV, VTK).** nTop: Field from Point Map, Von
   Mises and Displacement Point Maps [15]. The hinge of every FEA-driven
   demo (3.7): it is why a thickness can follow stress. Read CSV
   `x,y,z,value` and legacy VTK point data into a k-d tree or resampled
   grid; expose nearest, linear and clamped extrapolation. (Roadmap F3.)
2. **Headless parametric run, JSON in, JSON and CSV out.** nTop: nTop
   Automate [6][7]. Every loop in section 3 is `ntopcl -j in.json`. Add
   `anvil-cli run doc.anvil --inputs in.json --outputs out.json` that sets
   named parameters, rebuilds, exports meshes under stable names, and
   writes mass, volume, bounds and expression values. This makes a DoE
   with OpenFOAM or CalculiX possible from Python.
3. **Boundary-tagged mesh export.** nTop: CGNS with wall and farfield [27],
   Fluent .msh with named selections [26], Abaqus sets [16]. Tag surface
   nets triangles by nearest primitive or picked Anvil face; write OBJ
   groups, Gmsh .msh v2 or multi-solid STL. Users pay because the mesh
   arrives ready to solve.
4. **Adaptive meshing with sharp features.** nTop: Mesh from Implicit by AT
   and Sharpen Mesh [19][21]. Octree surface nets or dual contouring so a
   0.3 mm gyroid wall in a 300 mm part needs no uniform grid, plus a remesh
   step so one field yields a print mesh and an FE mesh. (Roadmap F4.)
5. **Topology optimisation import.** nTop: density field, threshold,
   Smoothen Body, density-driven lattice thickness [12][13]. Read a density
   field (VTK, or per element from CalculiX or ToPy), threshold it as a
   `Field`, smooth on the grid, let the density drive `Graded`. Cheap once
   item 1 exists; the second most cited nTop use after lattices.
6. **Cell size grading and general warps.** nTop: field-driven cell map,
   Lattice Warping [8]. Cell size as a field (a varying period needs a
   phase integral or piecewise map to avoid tearing) and warps from a
   picked surface's UV, then a quad-mesh cell map [9].
7. **Ribs from a projected graph.** nTop: Graph by Planar Projection, Ribs
   from Graph, Structural/Voronoi/Honeycomb Ribbing [10]. A 2D cell
   projected onto a surface and extruded as slabs with height as a field.
   This is how nTop sells to casting and moulding users.
8. **Slices from the field.** nTop: Slice Body, Hatch, CLI and PNG export
   [18]; the EOSPRINT plugin's per-layer slicing [36]. Marching squares on
   a `Field` at layer height to CLI or PNG stacks; the one place Anvil can
   offer a meshless path today. (F6.)
9. **A scriptable CFD bridge.** nTop: nTop Fluids [24]. Not an LBM solver,
   but a tagged mesh plus generated `snappyHexMeshDict` for OpenFOAM and
   force coefficients read back. With item 2 this reproduces section 3.2
   on free software; `aero.rs` already gives the fast inner estimate.
10. **3MF with beam lattice extension.** nTop: Export 3MF [40]. Beams as
    beams; a third the size of STL; read by Magics and printer software.
11. **Compiled evaluation.** nTop: the 5.0 kernel and GPU rendering
    [4][22]. Fidget as a back end for the primitive tree (F5), after items
    1 to 4.

Not worth chasing: exact B-rep export from fields (nTop lacks it too), a
built-in SIMP optimiser before a solver bridge exists, and an in-process
Python API (nTop wins with a CLI and JSON).

## Sources

1. https://www.ntop.com/resources/blog/computational-modeling-with-ntop-platform/
2. https://www.ntop.com/resources/blog/implicits-and-fields-for-beginners/
3. https://www.ntop.com/resources/blog/understanding-the-basics-of-b-reps-and-implicits/
4. https://develop3d.com/cad/ntopology-3-0-review/
5. https://support.ntop.com/hc/en-us/articles/7042784039443-How-to-create-a-custom-block
6. https://support.ntop.com/hc/en-us/articles/360052703693-Running-nTop-Automate-in-Python-scripts
7. https://support.ntop.com/hc/en-us/articles/49499893352211-nTop-Automate-for-Linux-Container-Image
8. https://www.ntop.com/resources/blog/new-latticing-technology/
9. https://support.ntop.com/hc/en-us/articles/29651196870035-Guide-to-Conformal-Latticing
10. https://support.ntop.com/hc/en-us/articles/35117560848275-Guide-to-Rib-Design
11. https://support.ntop.com/hc/en-us/articles/15280003943571-Field-Optimization-FAQ
12. https://www.ntop.com/resources/blog/topology-optimization-in-a-world-of-fields-and-implicit-geometry/
13. https://learn.ntop.com/courses/340-topology-optimization/lessons/post-processing/
14. https://support.ntop.com/hc/en-us/articles/360041676813-How-do-I-use-the-Ramp-block
15. https://support.ntop.com/hc/en-us/articles/360054149874-How-to-use-simulation-results-to-create-a-Point-Map-or-Field
16. https://support.ntop.com/hc/en-us/articles/360048249514-What-file-types-can-nTop-import-and-export
17. https://support.ntop.com/hc/en-us/articles/360048784874-How-to-export-geometry-Overview-of-export-workflows
18. https://support.ntop.com/hc/en-us/articles/360042790233-How-to-create-and-export-slices and https://support.ntop.com/hc/en-us/articles/17319219880467-nTop-Plugin-for-EOSPRINT-Overview
19. https://www.ntop.com/resources/blog/meshing-in-fea-cfd-manufacturing/
20. https://support.ntop.com/hc/en-us/articles/30049593128211-nTop-5-0-Meshing-updates-for-Simulation-Optimization and https://learn.ntop.com/courses/102-guide-to-meshing/lessons/converting-to-a-mesh/
21. https://support.ntop.com/hc/en-us/articles/43355998101907-nTop-5-27-What-s-New
22. https://support.ntop.com/hc/en-us/articles/26062971882131-nTop-5-0-New-Implicit-Modeling-Kernel
23. https://support.ntop.com/hc/en-us/articles/30549947119251-nTop-5-0-What-s-New
24. https://www.ntop.com/resources/blog/introducing-ntop-fluids/
25. https://www.tctmagazine.com/ntop-launches-new-computational-fluid-dynamics-solution/
26. https://www.ntop.com/resources/videos/designing-a-parametric-wing-with-field-driven-airfoil-and-internal-structure/ and https://support.ntop.com/hc/en-us/articles/19341018332435-How-to-export-your-design-to-Ansys-Fluent
27. https://www.ntop.com/resources/blog/designing-a-automation-pipeline-for-high-fidelity-design-space-exploration/
28. https://www.ntop.com/resources/webinars/parametric-optimization-for-advanced-aircraft-conceptual-design/
29. https://www.ntop.com/resources/webinars/analysis-for-advanced-aircraft-design/
30. https://www.ntop.com/resources/blog/ntop-and-jetzero-are-building-the-next-generation-of-aircraft-design-with-nvidia-nemoclaw/
31. https://www.revolutioninsimulation.org/wp-content/uploads/2020/06/WP_HeatExchangerDesignSimulation.pdf
32. https://www.ntop.com/resources/videos/ntop-live-utilizing-cfd-fields-for-flow-optimization/
33. https://learn.ntop.com/courses/training-ntop-discovery-doe/
34. https://www.ntop.com/resources/videos/ntop-live-optimize-lattice-structures-for-lightweighting-with-design-automation/
35. https://www.ntop.com/resources/videos/ntop-live-field-driven-design-from-imported-simulation-data/ and https://www.ntop.com/resources/videos/ntop-live-topology-optimization-driven-ribs-on-a-swing-arm/
36. https://www.ntop.com/resources/product-updates/from-implicit-to-print/
37. https://www.ntop.com/software/ntop-core/
38. https://www.g2.com/products/ntop/reviews
39. https://gaugehow.com/tools/ntop (third-party price estimate; nTop publishes no list price)
40. https://www.ntop.com/resources/blog/ntop-platform-supports-the-3mf-file-format-with-beam-extension/
