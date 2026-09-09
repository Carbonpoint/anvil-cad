# CAM, Simulation Hooks, and Interchange Formats: A Technology Review

Review date: 2026-09-09. Produced by a Sonnet research agent for the Anvil project.

## Purpose

This report looks at three tightly linked pieces of a parametric CAD/CAM application built from scratch in Rust: CAM toolpath generation, hooks into simulation tools, and file formats for moving data in and out. The goal is capability comparable to Siemens NX, including its CAM module, on CPU only, across platforms. The review draws on NX documentation, open-source CAM projects, and the current Rust crate ecosystem.

## 1. What NX CAM offers

NX CAM is organized as a set of operation types layered on a shared toolpath engine, a tool and material database, and a post-processing stage. A new app can copy this shape without copying NX itself.

**2.5D milling.** NX bundles face milling, planar milling, Z-level milling, and pocketing into one package for prismatic parts. Feature-based programming lets a user pick solid faces to define a pocket or boss instead of building boundary curves by hand, and the resulting toolpath stays associative to the model, so a design change regenerates the operation automatically.

**3-axis roughing and finishing.** NX splits this into Cavity Mill for roughing and Z-Level for finishing. Cavity Mill removes bulk stock in layers and leaves a uniform offset. A smaller-tool rest pass then clears corners the larger tool could not reach. Z-level roughing offers trochoidal and arc-shaped tool inserts to reduce cutting load in tight corners, which is the same problem Fusion 360's Adaptive Clearing solves. Z-level finishing keeps passes evenly spaced across steep and shallow areas for uniform surface finish.

**5-axis.** Variable-axis surface contouring automates programming of drafted or complex walls. Swarf milling uses the tool's side, aligned to the wall, and lets the user control tilt and lead-lag angles for tool life and surface quality. Tool-axis control exists specifically to avoid gouges and holder collisions on undercuts.

**Turning.** Covers face, turn, bore, and undercut roughing, multi-pass and helical finishing, grooving, and ID/OD threading, where NX reads pitch and thread geometry directly from the 3D model.

**Drilling.** Grouped as "hole making": deep drill, peck drill, boring, tapping, and user-defined cycles. Peck drilling periodically retracts the drill to clear chips on deep holes. This maps directly onto canned cycles like G81, G82, and G83, with the actual G-code choice controlled entirely by the post-processor, not the CAM logic.

**Post-processors.** A post-processor translates NX's generic tool-motion representation into a specific controller's dialect. NX Post Builder lets integrators build or customize a post through definition and event-handler files, without touching the core CAM engine. The key idea worth reusing: keep an internal, generic motion representation, then let a configuration-driven layer, not hardcoded logic, emit dialect-specific text, firing "events" on tool change, cycle start, rapid move, and feed move.

**Tool libraries.** NX's Manufacturing Resource Library manages cutting tools, fixtures, and measuring devices as searchable 3D assemblies usable for both toolpath generation and collision checking, tied to a Machining Data Library of speeds and feeds. ISO 13399 is the interchange standard behind cutting-tool data exchange across vendors.

**Stock simulation.** NX tracks an in-process workpiece (IPW), the accumulated removed-material state after each operation, which becomes the starting stock for the next operation. "Check Toolpath" runs gouge and collision checks against the IPW, holders, fixtures, and machine components.

**Feature-based machining.** Automatically detects holes, pockets, slots, and contours, and assigns an operation template to each, using attached tolerance and finish callouts to help pick tools and parameters.

## 2. Open-source CAM: tools and algorithms

**FreeCAD Path/CAM.** FreeCAD is LGPL-2+. Its Path workbench was renamed CAM and rewritten with new tool-bit management, a faster built-in simulator, and better adaptive roughing. The old per-machine post-processor design, one Python file per controller with heavy duplication, is a known weak point still being redesigned. CAM operations run in Python and call OpenCAMLib for some 3D work.

**OpenCAMLib (OCL).** A C++ library with Python and JS bindings that implements drop-cutter (drop a cutter at an XY point until it touches the mesh) and waterline/push-cutter (constant-Z contour following) toolpath generation for 3-axis work. Relicensed from GPL to LGPL in 2018. The original aewallin repo is largely inactive; a fork (fra589/opencamlib) is the one FreeCAD actually uses. Its waterline code has known correctness bugs flagged by maintainers as needing a rewrite.

**pycam.** GPLv3, generates G-code from 2D/3D models for 3-axis milling. Development stalled between 2012 and 2017 and now has essentially one active maintainer.

**Kiri:Moto.** MIT licensed, pure JavaScript, runs entirely in-browser. Does FDM slicing, CAM toolpaths, and laser cutting, with no native or WASM core.

**CAMotics.** GPLv2 G-code simulator for stock removal and collision visualization on 3-axis paths. It does not yet detect tool-shaft or fixture collisions or rapid-in-material events; those are planned, not implemented.

**LinuxCNC.** Not a CAM tool but the execution target: a motion controller whose RS274NGC interpreter parses G-code, including canned cycles and offsets, into canonical machine motion. Its documentation and open interpreter source are a useful reference implementation of the G-code dialect.

**bCNC.** GPL Python GRBL sender with basic built-in CAM (profiling, pocketing, drilling, cutout tabs). An open feature request shows users want adaptive/trochoidal clearing, which it currently lacks.

**Rust crates.** No mature Rust equivalent of OCL's drop-cutter or waterline algorithms exists yet, and no first-party Rust port of Clipper2 was found either; both are real gaps a from-scratch project would need to fill or bridge via FFI. What does exist: `cavalier_contours`, a Rust rewrite of the CavalierContours C++ library, offering true-arc polyline offsetting and polygon boolean operations, actively developed with a stable C FFI goal. `cnccoder` provides higher-level cutting instructions compiled to G-code with CAMotics export for simulation. `g-code` and `gcode` handle G-code text (see section 3).

**Core algorithms.** Pocketing is done either as contour-parallel offsetting or zig-zag raster passes; the CGAL straight-skeleton and polygon-offsetting module is the reference algorithm most implementations copy. Voronoi-diagram methods are used specifically to find and clean up leftover material in 2.5D pocket rest machining. Adaptive or trochoidal clearing, the technique behind Fusion 360's Adaptive Clearing, keeps radial tool engagement roughly constant (around 10 to 15 percent) using near-circular trochoidal motion, which lets axial depth and feed rate increase; the underlying idea dates to the 1980s and one related patent (US 8,694,149) is worth noting for IP awareness. Rest machining works by tracking the volume already removed by prior tools and diffing it against the target shape, either with a voxel model or per-Z-level 2D region comparison, to find what a smaller tool must clean up next. Five-axis gouge and collision checking generally works in two stages: generate cutter-contact points on the surface, then pick a tool orientation at each point that avoids local gouging and, separately, global collision with the part, fixture, or holder, using bounding-volume checks before finer sweep-plane or curvature-based tests.

## 3. G-code generation and post-processors

G-code descends from RS-274, standardized in the US as RS-274-D and internationally as ISO 6983-1. It is a minimal, line-oriented base language of motion words (G) and machine-control words (M), and every vendor extends it, so "G-code" is really a family of dialects, not one language.

Dialects differ in real, load-bearing ways. Fanuc and Haas use G81 to G89 canned cycles, G54 to G59 work offsets, IJK arc centers, and M98/M99 subprograms. LinuxCNC implements its own RS274NGC variant, documented with an explicit page listing where it departs from the original NIST specification. Siemens Sinumerik 840D goes further, layering a proprietary language with R-parameters, global user data, branches, and loops on top of DIN 66025 G-code, with its own cycle library rather than Fanuc-style canned cycles. Heidenhain controls are natively conversational, not G-code at all, though most can import or translate it. Marlin and GRBL are simplified hobby dialects; GRBL has no native canned drilling cycles and needs them pre-expanded into explicit plunge and retract moves.

The classical way commercial systems manage this is a two-stage split. A CAM system produces a neutral, machine-agnostic toolpath. Historically this was CLDATA, the cutter location data produced by compiling APT source, stored in a CLFILE in one of three encodings (BCL, ACL, SCL); NX's own variant is called a CLSF. A separate post-processor stage then reads that neutral data plus a machine-capability description and emits dialect-specific text. Fusion 360 keeps this split alive with a JavaScript post-processor engine, driven by a documented API, with an official VS Code editing extension (`Autodesk/cam-posteditor`, MIT licensed) and a hosted library of hundreds of machine-specific `.cps` files.

For a Rust app, the same split is the right architecture: an internal, dialect-neutral toolpath representation, and a pluggable post module, ideally scriptable or at least data-driven, that maps it to Fanuc, Haas, LinuxCNC, Siemens, or Marlin output. On the Rust side, the `gcode` crate is a no_std-friendly, dual MIT/Apache-2.0 parser with a zero-allocation visitor API, but it only reads G-code. The `g-code` crate both parses (via the `peg` parser generator) and emits G-code, including checksums and line numbering, which is the better fit for a tool that must write, not just read, NC code.

## 4. Interchange formats a CAD app must read and write

**STEP / AP242.** STEP AP203 handles geometry and configuration management with no PMI. AP214 adds color but still no GD&T. AP242 merges both and adds full product manufacturing information: GD&T, datums, and 3D annotations, and is the right target since it is the superset. Rust support here is genuinely immature: `ruststep` parses STEP exchange structures into generated Rust types but its own README says it is experimental and not for production use, with serialization still unimplemented, and no documented AP242 coverage. `opencascade-rs`, LGPL-2.1 bindings to the mature OpenCASCADE C++ kernel via `cxx.rs`, is the more production-viable near-term path, since OpenCASCADE already has STEP, IGES, and boolean operations solved; the tradeoff is an LGPL C++ dependency and an API still in flux.

**IGES.** No real Rust parser exists at all beyond a magic-byte MIME detector. This is a genuine gap for a project aiming at NX-level interchange, since IGES is legacy but still shows up from older sources.

**STL.** Simple and solved. The `stl_io` crate reads binary and ASCII STL and writes binary STL, with wide adoption (used by dozens of dependent crates).

**3MF.** Younger but real: `threemf`, extracted from the Fornjot kernel under a 0BSD license, is small but functional, and newer projects like `lib3mf-rs` claim broad coverage of the official 3MF extensions (materials, slicing, security, booleans) against the Consortium's own test suite. 3MF is a stronger target than STL for a modern app because color, material, and unit metadata live natively in its zip-plus-XML container.

**OBJ.** Mature via `tobj`, an MIT-licensed loader modeled on tinyobjloader with heavy real-world use.

**glTF/GLB.** Strong ecosystem via the `gltf` crate, actively maintained, supporting many Khronos extensions. This is the natural format for visualization and web preview rather than a native authoring format.

**DXF/DWG.** The `dxf` crate covers 2D DXF read and write, the right target for 2D CAM sources like laser and plasma work. DWG is proprietary and effectively closed; the only robust reader path is the Open Design Alliance's commercial Drawings SDK, which is not free for commercial use. A new app should plan to convert DWG via DXF or license the ODA SDK, not attempt a native DWG reader.

**Native format.** FreeCAD's `.FCStd` is a strong precedent: a plain zip container holding an XML manifest of the object graph and properties alongside per-object binary BREP blobs. That pattern, a compact manifest (CBOR or JSON, with a version field for migrations) plus a content-addressed blob store for meshes and B-rep geometry, is a good fit for Rust, since serde and CBOR tooling are mature, while an embedded SQLite container adds a heavier dependency for less benefit.

## 5. Simulation and analysis hooks

Gmsh is the de facto free meshing backend for open FEA pipelines: GPL licensed, with `.geo` scripting and Python, C++, and Julia APIs, producing `.msh` output. CalculiX is a free, GPL-licensed solver (`ccx`) and pre/post processor (`cgx`) pair, using an Abaqus-compatible `.inp` input format, with results in `.frd`; a companion tool, `ccx2paraview`, converts those results to VTU specifically for visualization. MFEM, from LLNL and BSD-3 licensed, is different in kind: a C++ library you link against to build a custom solver, not a standalone application with a fixed input format, and it has no official Rust bindings, so it is only reachable through FFI to its C++ API.

Siemens' own integration model, in Simcenter 3D built on NX Nastran, keeps direct association between CAD geometry and the simulation model: mesh, loads, and boundary conditions live as dependent objects referencing the geometry, so a design change triggers remeshing and boundary-condition remapping rather than a manual rebuild. This is the same feature-tree idea CAD kernels already use, just extended to simulation objects.

The closest working precedent for an open-source app is FreeCAD's FEM workbench. It shells out to Gmsh (or Netgen) for meshing and supports multiple solvers, CalculiX primarily, through a plugin layer: each solver module writes its own native input file from the FreeCAD document, invokes the external solver executable, and parses native results back into an internal mesh/result object for VTK-based rendering. It is a shell-out-plus-file-interchange pattern, not a tight in-process API, and that is a reasonable, low-risk model to copy.

For the plugin boundary itself, WASM-sandboxed extensions, as used by the Zed editor, are a stronger choice than native dynamic-library plugins: they run under WASI with a restricted capability set, so a misbehaving plugin cannot crash or compromise the host, and they stay portable across platforms, matching a from-scratch Rust project's goals. A practical hook design has four parts: an export path that serializes geometry and a generated mesh to STEP or Gmsh format and boundary conditions to a CalculiX-style input file; subprocess invocation of the external solver; an import path that reads results back, ideally as VTU for portability, matching the `ccx2paraview` pattern; and a WASM-based plugin boundary rather than native FFI. On the Rust side, the `vtkio` crate is a mature reader and writer for both legacy and XML VTK formats, including `.vtu`, and is directly usable for an in-house results viewer without linking the C++ VTK library.

## Conclusion

Nothing here requires exotic new theory. NX's CAM feature set maps cleanly onto well-understood algorithms (offset-based pocketing, drop-cutter and waterline 3D toolpaths, trochoidal clearing, IPW-based simulation) that open-source tools like OpenCAMLib, FreeCAD, and CAMotics already implement, just not yet in Rust. The clearest gaps for a from-scratch Rust build are: no mature Rust 3D toolpath library (drop-cutter, waterline, rest machining) comparable to OCL; no Rust Clipper2 port; STEP support (`ruststep`) explicitly not production-ready and lacking AP242; and no usable Rust or open-source IGES reader. The pragmatic path is to bridge to OpenCASCADE for STEP/IGES/boolean geometry via FFI while building toolpath algorithms, G-code emission, and simulation-export hooks natively in Rust, following the CLDATA-then-postprocess split for G-code and the shell-out-plus-file-interchange pattern FreeCAD's FEM workbench already validates for simulation.

## References

**NX CAM**
- [NX CAM 2.5-Axis Milling](https://www.siemens.com/en-us/products/nx-manufacturing/offerings/cam-25-axis-milling/)
- [NX CAD/CAM 2.5-Axis Milling & Turning](https://www.siemens.com/en-us/products/nx-manufacturing/offerings/cad-cam-multi-axis-mill-turn/)
- [Cavity Milling Guide](https://www.tuofamachining.com/news/cavity-milling-guide-strategies-tools-cam-tactics-for-deep-pocket-machining-259348.html)
- [NX CAM 5-Axis Machining (Applied CAx)](https://www.appliedcax.com/training/nx-cam/nx-cam-5-axis-machining-nx-fixed-and-variable-axis-milling/)
- [NX CAD/CAM Turning](https://plm.sw.siemens.com/en-US/nx/products/cad-cam-turning/)
- [Canned Cycles in CNC (G81 to G89)](https://cnccode.com/2025/07/11/canned-cycles-in-cnc-g81-to-g89-drilling-boring-and-tapping-explained/)
- [Post processor overview (Wikipedia)](https://en.wikipedia.org/wiki/Post_processor)
- [NX CAM postprocessors explained](https://industryinsider.eu/metalworking/nx-cam-postprocessors/)
- [Tool library management connected to NX CAM](https://resources.sw.siemens.com/en-US/fact-sheet-an-essential-tool-library-management-system-that-connects-directly-to-nx-cam/)
- [ISO 13399 (Wikipedia)](https://en.wikipedia.org/wiki/ISO_13399)
- [NX CAM simulation validates machines and processes](https://blogs.sw.siemens.com/nx-manufacturing/how-nx-cam-simulation-capabilities-validate-machines-and-processes/)
- [Feature-Based Machining in NX CAM](https://blogs.sw.siemens.com/nx-manufacturing/feature-based-machining-in-nx-cam/)

**Open-source CAM and algorithms**
- [OpenCAMLib documentation](https://opencamlib.readthedocs.io/en/latest/)
- [OpenCAMLib (fra589 fork)](https://github.com/fra589/opencamlib)
- [PyCAM (SebKuzminsky)](https://github.com/SebKuzminsky/pycam)
- [Kiri:Moto / grid-apps](https://github.com/GridSpace/grid-apps)
- [CAMotics repository](https://github.com/CauldronDevelopmentLLC/CAMotics)
- [LinuxCNC RS274NGC G-code docs](https://linuxcnc.org/docs/2.2/html/gcode_main.html)
- [bCNC repository](https://github.com/vlachoudis/bCNC)
- [bCNC adaptive clearing feature request](https://github.com/vlachoudis/bCNC/issues/815)
- [cavalier_contours repository](https://github.com/jbuckmccready/cavalier_contours)
- [Clipper2 repository](https://github.com/AngusJohnson/Clipper2)
- [Fusion 360 Adaptive Clearing](https://help.autodesk.com/view/fusion360/ENU/?guid=3D-ADAPTIVE-READ)
- [US Patent 8,694,149](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/8694149)
- [Voronoi-based pocket rest-material paper](https://www.sciencedirect.com/science/article/abs/pii/S0010448505001661)
- [FreeCAD CAM postprocessor architecture issue](https://github.com/FreeCAD/FreeCAD/issues/6117)
- [CGAL Straight Skeleton / Polygon Offsetting](https://doc.cgal.org/latest/Straight_skeleton_2/index.html)
- [cnccoder crate docs](https://docs.rs/cnccoder)

**G-code and post-processors**
- [G-code overview (LinuxCNC)](https://linuxcnc.org/docs/html/gcode/overview.html)
- [RS274NGC Differences (LinuxCNC)](https://linuxcnc.org/docs/2.6/html/gcode/rs274ngc.html)
- [G-code (Wikipedia)](https://en.wikipedia.org/wiki/G-code)
- [Cutter location (Wikipedia)](https://en.wikipedia.org/wiki/Cutter_location)
- [Autodesk cam-posteditor repository](https://github.com/Autodesk/cam-posteditor)
- [Autodesk Post Library for Fusion](https://cam.autodesk.com/hsmposts)
- [Siemens SINUMERIK 840D Advanced Programming Guide](https://itscnc.com/pub/media/documents/fadal_manuals/siemansmanuals/Advanced_Programming.pdf)
- [gcode crate (crates.io)](https://crates.io/crates/gcode)
- [g-code crate (crates.io)](https://crates.io/crates/g-code)

**Interchange formats**
- [ruststep repository](https://github.com/ricosjp/ruststep)
- [truck repository](https://github.com/ricosjp/truck)
- [truck tutorial/overview](https://ricos.gitlab.io/truck-tutorial/v0.1/overview.html)
- [opencascade-rs repository](https://github.com/bschwind/opencascade-rs)
- [stl_io repository](https://github.com/hmeyer/stl_io)
- [threemf crate (crates.io)](https://crates.io/crates/threemf)
- [lib3mf-rs repository](https://github.com/sscargal/lib3mf-rs)
- [tobj crate (crates.io)](https://crates.io/crates/tobj)
- [gltf repository](https://github.com/gltf-rs/gltf)
- [dxf crate docs](https://docs.rs/dxf)
- [Open Design Alliance pricing](https://www.opendesign.com/pricing)
- [FreeCAD .FCStd file format spec](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/File_Format_FCStd.md)
- [STEP AP203 vs AP214 vs AP242 comparison](https://cadshift.com/blog/step-ap203-ap214-ap242-version-comparison/)

**Simulation hooks**
- [Gmsh official site](https://gmsh.info/)
- [CalculiX official site](https://www.calculix.de/)
- [ccx2paraview (CalculiX to VTK converter)](https://github.com/calculix/ccx2paraview)
- [MFEM repository](https://github.com/mfem/mfem)
- [MFEM at LLNL](https://computing.llnl.gov/projects/mfem-scalable-finite-element-discretization-library)
- [FreeCAD FEM Workbench documentation](https://wiki.freecad.org/FEM_tutorial)
- [vtkio repository](https://github.com/elrnv/vtkio)
- [Siemens Simcenter 3D product page](https://www.siemens.com/en-us/products/simcenter/mechanical-simulation/simcenter-3d/)
- [Zed extension architecture (WASM sandboxing)](https://zed.dev/blog/language-extensions-part-1)
