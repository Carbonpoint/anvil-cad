# Parametric Modeling Architecture and Constraint Solving: A Literature and Technology Review

Review date: 2026-09-09. Produced by a Sonnet research agent for the Anvil project.

This review covers the technical foundations needed to build a from-scratch Rust CAD/CAM application with capabilities comparable to Siemens NX: a feature history tree, robust B-rep solids, constrained sketches, and standard features like extrude, revolve, and fillet. It focuses on parametric architecture and constraint solving, not rendering or CAM.

## 1. What Makes Siemens NX's Parametric System Strong

NX's strength comes from a few concepts working together, not from any single algorithm.

**Feature history tree.** Every operation (sketch, extrude, fillet, pattern) is recorded as a node in an ordered tree, shown in the Part Navigator. Each feature stores its inputs, its parameters, and references to the geometry it consumes. Editing an early feature triggers regeneration of everything downstream, in order.

**Expressions.** Every dimension can be a literal number or a named expression, such as `hole_dia = shaft_dia + clearance`. Expressions can reference other expressions, part attributes, or even values in linked parts, forming a dependency network that drives geometry.

**Synchronous Technology.** Introduced in NX 5 (2007), this is NX's hybrid answer to the parametric-versus-direct-modeling tradeoff. It analyzes the current B-rep directly (face angles, tangencies, radii) rather than replaying history, so a user can grab a face and push it without needing to edit the feature that created it. The feature tree becomes a "feature collection," useful for selection, but it no longer dictates a strict rebuild order the way pure history-based modeling does. This matters for our project because pure history replay is fragile against topological naming failures (see Section 2); a direct-edit layer that reasons about the current shape is a genuine mitigation, not just a UX nicety.

**Associativity and WAVE linking.** In an assembly, geometry in one part can be copied into another as a live, associative link (a WAVE Geometry Linker). Downstream parts update automatically when the source changes. This is what enables true top-down assembly design, where a skeleton or control part drives the shape of many child parts. The tradeoff, well documented by NX practitioners, is that undisciplined WAVE linking creates circular references and fragile load-order dependencies, so top-down strategy and layering matter as much as the mechanism itself.

For a new system, the transferable ideas are: an explicit, inspectable feature graph; a first-class expression/parameter layer; and some direct-edit capability layered on top of history rather than a total replacement for it.

## 2. Feature History Architecture: DAG, Regeneration, and Topological Naming

**The feature graph.** A parametric history model is naturally a directed acyclic graph (DAG): nodes are features or geometric entities, edges are "consumes" or "produced by" relationships. Regeneration is a topological sort followed by re-execution of each feature's operation in dependency order, feeding each one the (possibly changed) output of its parents. When a parameter changes, the system need only invalidate and recompute the downstream subgraph, not the whole model. Recent academic work frames this explicitly as an "Attributed Feature Graph," encoding both hierarchy and geometric relationships for tasks like bidirectional CAD-to-graph translation.

**The topological naming problem.** This is the central architectural problem in history-based CAD. When a feature like "Fillet" refers to "Edge12" of the previous shape, and an earlier feature changes so that the B-rep kernel regenerates its topology, the numbering of faces, edges, and vertices assigned by the kernel can change or become ambiguous. A reference that meant one edge yesterday might silently point at a different edge today, or at nothing at all, breaking downstream features. This is not a bug in any one CAD system; it is a structural consequence of using index-based or generic names (Face1, Edge2) for entities whose identity is not preserved by the underlying kernel across a rebuild.

**FreeCAD's case, in detail.** FreeCAD suffered from this problem for years: sketches and features attached to specific named sub-elements would break whenever an upstream edit reordered the B-rep. Developer realthunder built a fix in a long-running personal fork ("LinkStage"/Link branch) rather than upstream, which caused two problems: the fork drifted out of sync with mainline FreeCAD, and the code was not maintainable by anyone else. A multi-year task force then ported the approach into upstream FreeCAD, landing in FreeCAD 1.0 (November 2024). The mechanism: instead of naming a sub-element by its generic type-and-index, FreeCAD now builds an "element map" that records modeling history for each face, edge, and vertex, using OCCT's `BRepBuilderAPI_MakeShape` history-tracking API to map inputs to outputs across each operation. Because these historical names can grow very long as more operations accumulate, a `StringHasher` compresses them to integer IDs and, past a length threshold, discards the readable string and keeps only a SHA1 hash, storing the mapping in dedicated `IndexedName`/`MappedName` classes for speed and compact persistence. Ondsel, a commercial FreeCAD-adjacent startup, funded and coordinated much of this integration work before shutting down in late 2024; it merged 145 pull requests into upstream FreeCAD, including much of the toponaming fix, before ceasing operations.

**Academic persistent-naming literature.** This problem was studied long before FreeCAD hit it. Kripac (1997, "A mechanism for persistently naming topological entities in history-based parametric solid models," Computer-Aided Design 29(2)) proposed matching entities across regenerations by name, addressing ambiguity but not the basic problem of naming newly created faces. Capoyleas, Chen, and Hoffmann (1996, "Generic naming in generative, constraint-based design," Computer-Aided Design 28(1)) took a complementary approach focused on resolving ambiguity in generic (type-based) names, but did not handle face merging or splitting during Booleans. Later Korean-CAD-community work (KAIST, KoreaScience) extended this into full "identification of topological entities and naming mapping" schemes aimed at model exchange between systems. The practical lesson for a new Rust kernel: decide early whether entity identity is tracked via a persistent, kernel-level history map (the OCCT/FreeCAD approach) versus purely geometric/topological matching heuristics after the fact. The former is more robust but requires the B-rep kernel itself to expose creation history for every operation, which is a real architectural commitment, not an add-on.

## 3. 2D Geometric Constraint Solvers

Sketch solving is the other hard, well-studied subproblem.

**SolveSpace.** Represents constraints (coincident, tangent, parallel, dimension, etc.) as symbolic algebraic equations, then solves the resulting nonlinear system numerically with a modified Newton-Raphson iteration: each nonlinear constraint is linearized around the current guess, the resulting linear system is solved, and the process repeats until convergence. For underconstrained sketches, dragging uses a least-squares step so the solver moves the dragged point while disturbing everything else as little as possible. SolveSpace is GPLv3, and its solver core is reused by other projects, notably Dune3D and the Rust binding crate `rust_slvs`.

**FreeCAD's planegcs.** A separable 2D constraint solver library that supports multiple numeric optimization backends: DogLeg (the default), Levenberg-Marquardt, BFGS, and SQP (used automatically for "temporary" constraints during interactive dragging). It has been ported to WebAssembly (`@salusoft89/planegcs`) for browser use outside FreeCAD itself. Known weaknesses documented in FreeCAD's own tracker include the LM backend sometimes misreporting a sketch as fully constrained when redundancy is actually present, showing that solver choice affects not just speed but diagnostic correctness.

**Open CASCADE Technology.** OCCT (LGPL 2.1) provides the B-rep kernel, NURBS geometry, and Boolean operations that FreeCAD and Dune3D both build on, but it does not ship a general 2D sketch constraint solver of its own; historically a commercial add-on from LEDAS filled that gap for OCCT-based products. This is a useful data point: a B-rep kernel and a constraint solver are separable concerns and are commonly built or sourced independently.

**Rust crates.** The ecosystem is young but growing: `rust_slvs` wraps SolveSpace's own solver for use from Rust; `arael-sketch-solver` and `solverang` are pure-Rust Levenberg-Marquardt-based solvers supporting points, lines, arcs, circles, and a fair set of constraint types with parametric/expression-driven dimensions. None yet has the maturity of SolveSpace's decade-plus battle-tested solver.

**Algorithms and diagnosis, more generally.** A 2022 survey ("A review on geometric constraint solving," arXiv:2202.13795) frames the field as: numerical methods (Newton-Raphson, Levenberg-Marquardt, homotopy continuation) that iterate to a solution but need a good initial guess and struggle with non-square (over/under-determined) systems; and symbolic/graph-based methods that first analyze the constraint graph to decompose it into smaller, independently solvable clusters before applying numerical solving to each. Owen's decomposition algorithm (1991) and the decomposition-recombination (DR) planning framework developed by Hoffmann, Lomonosov, and Sitharam generalize degrees-of-freedom counting into a formal graph-reduction procedure: a constraint graph is repeatedly reduced by finding and collapsing rigid sub-clusters until the whole sketch is solved as a hierarchy of small systems, which is both faster and more diagnosable than solving one giant nonlinear system at once. This decomposition step is also what enables good over/under-constrained diagnosis: DOF counting per cluster localizes exactly where a sketch has too few or too many constraints, rather than reporting a single global failure.

## 4. Expression and Parameter Engines

The expression engine is architecturally simple but essential: parameters and dimensions are nodes in a dependency graph, expressions are edges, and evaluation order is a topological sort of that graph, same as feature regeneration. Fusion 360 and NX both allow any dimension field to hold an expression referencing other named parameters (arithmetic, trig functions, lookups); changing a driving parameter triggers recomputation of everything downstream, which in practice means replaying the feature timeline from the first affected feature forward. The clean design is to make the parameter/expression graph and the feature/geometry graph the same kind of object (nodes with dependencies, evaluated by topological order), so one regeneration engine drives both, rather than building two separate dependency-tracking systems.

## 5. Undo/Redo and Document Models

CAD undo/redo is conventionally built on the command pattern: every user action becomes a command object capturing enough state (or enough of a delta) to reverse itself, pushed onto an undo stack; redo pops from a parallel stack that is cleared whenever a new command is issued after an undo. Commands can be bundled so that one logical parametric operation (say, "add fillet feature," which touches the feature tree, the expression graph, and cached geometry) undoes and redoes as a single atomic unit. Because a parametric document is a graph of features and parameters rather than flat pixel or vertex data, many modern implementations favor persistent (structurally shared, immutable) data structures for the document model: an edit produces a new graph version that shares unchanged subgraphs with the old one, so keeping old versions on an undo stack is cheap in memory rather than requiring full deep copies at every step. This also composes naturally with multi-document undo scopes and with future collaborative editing.

## 6. Existing Rust Projects with Sketch/Constraint Work

- **CADmium**: a from-scratch browser CAD program built on `truck` (a pure-Rust B-rep kernel using NURBS, targeting WebGPU/WASM). Its own crate provided sketches, extrusions, and constraint structures, with a first-order 2D constraint solver as a stated but not fully completed goal. Licensed under the Elastic License 2.0 (source-available, not OSI-open: it forbids offering it as a hosted service to third parties). The repository was marked archived and inactive as of September 2025, though it retains around 1.6k GitHub stars, making it a useful architecture reference rather than a living dependency.
- **Fornjot**: an early-stage Rust B-rep kernel by Hanno Braun, dual-licensed MIT/Apache-2.0 in its ecosystem plugins. The main repository is now archived (as of mid-2026) and explicitly marked "no longer in development," with the maintainer noting the project's goals were not reached. A cautionary example of how hard a from-scratch B-rep kernel is to sustain as a solo or small effort.
- **Zoo (formerly KittyCAD) modeling-app and KCL**: an actively developed, MIT-licensed hybrid CAD app (about 1.3k stars, thousands of commits) where every point-and-click action is really generating KCL, a dedicated Rust-implemented DSL for parametric CAD, so the underlying model is always plain, version-controllable text. Important caveat for a CPU-only, fully local design goal: the app's 3D geometry evaluation is streamed from a hosted geometry engine to the browser as video, meaning the actual solid-modeling kernel is a cloud service, not a local Rust library the user runs offline. KCL and the client tooling are open, but the core modeling engine is not something you can vendor into an offline CPU-only application today.
- **Dune3D**: a C++ (not Rust) parametric CAD app, GPLv3, actively developed, built by combining SolveSpace's constraint solver with OCCT's B-rep kernel and UI conventions borrowed from the same author's Horizon EDA. It is the most concrete existing example of "compose a proven solver with a proven kernel" rather than writing either from scratch, and is a useful architectural template even though it is not Rust.
- **Ondsel**: not a Rust project but relevant context. A venture-backed startup (Lens PDM plus a FreeCAD flavor) that funded much of the FreeCAD 1.0 topological-naming fix and other upstream work, then shut down in October-November 2024 after failing to find commercial traction, transferring its IP to the FreeCAD community. It illustrates that funding this class of hard architectural fix (persistent naming) is expensive and that even well-resourced efforts on it have a high failure rate as businesses, even when the technical work succeeds.
- **truck**: the underlying B-rep kernel CADmium used; a general-purpose, actively maintained pure-Rust shape-processing library (NURBS-based B-rep, WebGPU rendering, WASM target), organized as a set of small interdependent crates. Likely the most reusable single Rust artifact for a new project's geometry kernel layer, though it does not include a constraint solver.

## Practical Takeaways

A competitive from-scratch Rust CAD system should treat the feature/parameter dependency graph as one regeneration engine (Section 2 and 4), commit early to a persistent-naming strategy at the kernel level rather than retrofitting one later (the FreeCAD experience shows this is a multi-year fix once deferred), reuse or port a proven 2D solver algorithm (SolveSpace's Newton-Raphson-plus-least-squares, or DR-planning-style decomposition) rather than inventing one, and build the document model on immutable, structurally-shared data so undo/redo and future collaboration are cheap. Among current Rust efforts, `truck` is the most promising reusable kernel component, SolveSpace's algorithm (via `rust_slvs` or a fresh reimplementation) is the most proven solver approach, and Dune3D's "combine an existing solver with an existing kernel" strategy, even though it is C++, is the most realistic near-term architectural template.

## References

- [Understanding Parametric and Direct Modeling in CAD Tools](https://blogs.sw.siemens.com/thought-leadership/understanding-parametric-and-direct-modeling-in-modern-cad-tools/)
- [Synchronous technology, Siemens](https://www.siemens.com/en-us/technology/synchronous-technology/)
- [Synchronous Modeling Walk-through, NX Design blog](https://blogs.sw.siemens.com/nx-design/synchronous-modeling-walk-through/)
- [Synchronous Technology White Paper (2008)](https://www.plm.automation.siemens.com/legacy/docs/Synchronous_Technology_CPDA_WhitePaper.pdf)
- [NX WAVE Link Best Practices, Applied CAx](https://www.appliedcax.com/wave-linking-best-practices/)
- [Siemens NX WAVE Geometry Linker, LearnNX](https://learnnx.com/lesson/siemens-nx-wave-geometry-linker-create-associative-geometry-links/)
- [Topological naming problem fixed in RealThunder's Link branch, Hacker News](https://news.ycombinator.com/item?id=40433024)
- [realthunder/FreeCAD, LinkStage3 branch README](https://github.com/realthunder/FreeCAD/blob/LinkStage3/README.md)
- [FreeCAD's topological naming problem is (officially) history, Ondsel blog](https://www.ondsel.com/blog/toponaming-problem-is-history/)
- [Milestone: Ondsel and FreeCAD team complete phase 2 of toponaming fix](https://www.ondsel.com/blog/milestone-toponaming-fix-phase-2-done/)
- [Topological Naming Algorithm, realthunder wiki](https://github.com/realthunder/asm3-wiki/blob/master/Topological-Naming-Algorithm.md)
- [Topo Naming PR, FreeCAD/FreeCAD #7427](https://github.com/FreeCAD/FreeCAD/pull/7427)
- [We are shutting down Ondsel](https://www.ondsel.com/blog/goodbye/)
- [The End of Ondsel, Hackaday](https://hackaday.com/2024/11/12/the-end-of-ondsel-and-reflecting-on-the-commercial-prospects-for-freecad/)
- [A Feature-Based Solution to the Persistent Naming Problem, CAD Journal](https://www.cad-journal.net/files/vol_2/CAD_2(1-4)_2005_517-526.pdf)
- [Identification of Topological Entities and Naming Mapping, KAIST](https://koasas.kaist.ac.kr/bitstream/10203/6600/1/Identification%20of%20Topological%20Entities%20and%20Naming.pdf)
- [Mechanisms of Persistent Identification of Topological Entities in CAD Systems: A Review, ScienceDirect](https://www.sciencedirect.com/science/article/pii/S1110016818300814)
- [Persistent Naming for Parametric Models, ResearchGate](https://www.researchgate.net/publication/2367893_Persistent_Naming_for_Parametric_Models)
- [SolveSpace Technology page](https://solvespace.com/tech.pl)
- [SolveSpace, Wikipedia](https://en.wikipedia.org/wiki/SolveSpace)
- [solvespace/solvespace, GitHub](https://github.com/solvespace/solvespace)
- [Salusoft89/planegcs, GitHub](https://github.com/Salusoft89/planegcs)
- [Levenberg-Marquardt false "fully constrained" report, FreeCAD issue #5861](https://github.com/FreeCAD/FreeCAD/issues/5861)
- [Constraints solver for sketch geometry in OCCT?, OCCT forum](https://dev.opencascade.org/content/constraints-solver-sketch-geometry-occt)
- [Geometric Constraint Solvers by Ledas, Open Cascade](https://old.opencascade.com/content/geometric-constraint-solvers-ledas)
- [Open Cascade Technology License](https://dev.opencascade.org/doc/overview/html/occt_public_license.html)
- [A review on geometric constraint solving, arXiv:2202.13795](https://arxiv.org/pdf/2202.13795)
- [Decomposition Plans for Geometric Constraint Problems, Part II](https://www.cise.ufl.edu/~sitharam/pdfs/drtwo-final.pdf)
- [Geometric constraint solving, Wikipedia](https://en.wikipedia.org/wiki/Geometric_constraint_solving)
- [rust_slvs, GitHub](https://github.com/thekakkun/rust_slvs)
- [arael-sketch-solver, crates.io](https://crates.io/crates/arael-sketch-solver)
- [solverang, crates.io](https://crates.io/crates/solverang)
- [CADmium: A Local-First CAD Program Built for the Browser](https://mattferraro.dev/posts/cadmium)
- [CADmium-Co/CADmium, GitHub](https://github.com/CADmium-Co/CADmium)
- [ricosjp/truck, GitHub](https://github.com/ricosjp/truck)
- [Truck: CAD Kernel in Rust, Hacker News](https://news.ycombinator.com/item?id=35071317)
- [hannobraun/fornjot, GitHub (archived)](https://github.com/hannobraun/fornjot)
- [KittyCAD/modeling-app, GitHub](https://github.com/kittycad/modeling-app)
- [KCL: A Programming Language for Parametric CAD, Zoo](https://zoo.dev/research/introducing-kcl)
- [dune3d/dune3d, GitHub](https://github.com/dune3d/dune3d)
- [Dune 3D, LinuxLinks](https://www.linuxlinks.com/dune-3d-parametric-cad/)
- [Bridging CAD and Data-Driven Design: Attributed Feature Graphs, arXiv:2606.06405](https://arxiv.org/pdf/2606.06405)
