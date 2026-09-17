# Casting Simulation: A Survey for Anvil

Review date: 2026-09-16. Written for the Anvil kettle project (see
`docs/KETTLE.md`, stage 5). The question: how should Anvil predict mold
fill and solidification for a sand-cast grey iron kettle, and later
aluminum A356 and magnesium AZ91 parts, either by exporting to an
existing open-source solver or by a small built-in model.

## 1. Open-source and free casting simulators

### Truchas (LANL)

Truchas is a multiphysics code from Los Alamos National Laboratory built
specifically for metal casting, with growing support for additive
manufacturing. Source: [github.com/lanl/truchas](https://github.com/lanl/truchas)
(a thin pointer) and the real repository at
[gitlab.com/truchas/truchas](https://gitlab.com/truchas/truchas), mirrored
read-only at [github.com/truchas/truchas](https://github.com/truchas/truchas).

* **License.** 3-clause BSD. Confirmed in
  [LICENSE.md](https://gitlab.com/truchas/truchas/-/raw/master/LICENSE.md).
  Government-funded but not export-restricted in the license text itself.
* **Language.** Mostly Fortran (87.6% "Fortran Free Form" by GitLab's
  language bar), with Python (6.7%), C (2.4%), and CMake (2.2%) for
  build scripting and tooling.
* **Build difficulty.** Real, but documented. Needs a Fortran/C/C++
  compiler set (Intel oneAPI, NAG, or GNU Fortran 10.2 to 12.1; GNU 9.x,
  10.1, and 11.1 are called out as broken), CMake 3.16+, MPI (an
  `mpicc` wrapper on the path), LAPACK, zlib, and Python 3.5+ with h5py
  and numpy. A first build is two stages: build a third-party library
  (TPL) bundle from the companion
  [truchas-tpl](https://gitlab.com/truchas/truchas-tpl) repo (this pulls
  in HDF5, NetCDF, Exodus II, PETSc-adjacent solvers, etc.), then build
  Truchas itself against that TPL install. See
  [BUILDING.md](https://gitlab.com/truchas/truchas/-/raw/master/BUILDING.md)
  and [SPACK.md](https://gitlab.com/truchas/truchas) (a Spack package
  exists for an alternative path). No Docker image was found in the
  repo, but pre-built UBI8 binary packages (Intel MPI and oneAPI
  variants) are produced by CI, per recent commit titles.
* **Input format.** Meshes are Exodus II (`.g`/`.exo` files), the format
  Cubit/Genesis and Gmsh-with-Exodus-export both produce. The input deck
  is a plain text file of Fortran namelists (`&MESH ... /`, `&PHYSICS
  ... /`, `&MATERIAL ... /`, etc.), read in any order. A full grammar is
  in the [Truchas Reference Manual](https://www.truchas.org/docs/reference-manual/introduction/index.html).
* **Physics.** This is the strong point: incompressible multi-material
  flow with interface tracking (free-surface fill), conjugate heat
  transfer, solid/liquid phase change with latent heat, enclosure
  (view-factor) thermal radiation, species transport, elastic-plastic
  solid mechanics with contact, and electromagnetics (induction
  heating). Confirmed in the repo
  [README](https://gitlab.com/truchas/truchas/-/raw/master/README.md).
  A real minimal example that couples flow and freezing lives at
  `test/freezing-flow/freezing-flow-1.inp` in the repo (fetched
  directly, see the input deck excerpt below). It is not a mold-filling
  case, it is a column of water that freezes from one end while flow is
  active, but it shows the exact namelist shape needed for a
  fill-and-freeze problem: `&MESH`, `&PHYSICS` with `flow = .true.` and
  `heat_transport = .true.`, `&FLOW`, `&FLOW_BC`, `&THERMAL_BC`,
  `&BODY`, `&MATERIAL` with `phases = 'solid', 'liquid'`, and
  `&PHASE_CHANGE` with `solidus_temp`, `liquidus_temp`, `latent_heat`.
  Truchas does not appear to compute shrinkage porosity directly as a
  first-class output; it reports temperature and liquid-fraction
  history, from which a user (or post-processing script) infers hot
  spots and likely porosity the way commercial tools like SOLIDCast do
  with a Niyama-type criterion.
* **Material data.** None shipped. No database of cast iron, A356, or
  AZ91 properties was found in the repo or docs. Every density, specific
  heat, conductivity, latent heat, solidus, and liquidus value must be
  typed into `&MATERIAL`/`&PHASE`/`&PHASE_CHANGE` blocks by the user.
* **Output format.** Exodus II results files, viewable in ParaView or
  Visit.
* **Maintenance.** Very active. The GitLab commit API shows a commit on
  2026-08-05, roughly six weeks before this review, from maintainer
  Neil N. Carlson, plus commits in late July 2026 adding new CI binary
  package workflows. This is a live project, not an abandoned one.
* **Real-world use.** Truchas has a long publication record specifically
  as a casting tool, e.g. "Truchas, a multi-physics tool for casting
  simulation" (see the ResearchGate listing:
  [researchgate.net/publication/233571093](https://www.researchgate.net/publication/233571093_Truchas_-_A_multi-physics_tool_for_casting_Simulation)),
  and LANL's own framing of the project is casting-first, not a general
  CFD code retrofitted for casting.

### OpenFOAM (interFoam plus solidification)

OpenFOAM is a C++ finite-volume toolbox distributed under GPLv3 by two
independent organizations that forked apart years ago: the OpenFOAM
Foundation at [openfoam.org](https://openfoam.org/), and ESI-OpenCFD
(now owned by Keysight after the 2025 ESI acquisition) at
[openfoam.com](https://www.openfoam.com/), which released v2606 on
2026-06-26. Both are GPLv3 and free. A third branch, `foam-extend`,
continues an older codebase with some solvers not present in the other
two.

* **Language and build.** C++17, built with its own build system on top
  of standard `make`/`wmake`, needs MPI for parallel runs, and needs a
  matching compiler (GCC is standard). Building from source is a known
  multi-hour process on Linux; most users take the prebuilt
  Ubuntu/apt or Docker packages both organizations publish instead of
  compiling from scratch.
* **Geometry input.** STL is the standard path: `snappyHexMesh` builds a
  hex-dominant volume mesh from one or more triangulated STL surfaces
  placed in `constant/triSurface`, refined and snapped to the surface
  through a `snappyHexMeshDict`. Confirmed in the official user guide:
  [openfoam.com/documentation/user-guide/4-mesh-generation-and-conversion/4.4-mesh-generation-with-the-snappyhexmesh-utility](https://www.openfoam.com/documentation/user-guide/4-mesh-generation-and-conversion/4.4-mesh-generation-with-the-snappyhexmesh-utility).
  This means an Anvil STL export is directly usable, no Exodus/Cubit
  step needed, unlike Truchas.
* **Physics for casting.** Free-surface fill is `interFoam`, OpenFOAM's
  stock two-phase VOF solver, mature and heavily used. Solidification
  with latent heat is also stock, but as a separate piece: the
  `solidificationMeltingSource` `fvOption`, an enthalpy-porosity model
  after Voller and Prakash (1987) and Swaminathan and Voller (1992),
  which is normally dropped into a single-phase buoyant solver (e.g.
  `buoyantPimpleFoam`) to melt or freeze a fluid in place, not to fill
  an empty mold. Source:
  [github.com/OpenFOAM/OpenFOAM-6/.../solidificationMeltingSource.H](https://github.com/OpenFOAM/OpenFOAM-6/blob/master/src/fvOptions/sources/derived/solidificationMeltingSource/solidificationMeltingSource.H).
  Combining free-surface fill and solidification in one run (what mold
  filling actually needs) is not a stock OpenFOAM solver. It shows up
  as custom, hand-built solvers in coursework and papers, for example a
  Chalmers University "OS_CFD" course project literally called
  `solidificationInterFoam` that bolts the enthalpy-porosity source onto
  `interFoam`
  ([tfd.chalmers.se/~hani/kurser/OS_CFD_2024/PariaKhosravifar/solidificationInterFoam_Paria_Khosravifar.pdf](https://www.tfd.chalmers.se/~hani/kurser/OS_CFD_2024/PariaKhosravifar/solidificationInterFoam_Paria_Khosravifar.pdf)).
  A related stock solver, `interPhaseChangeFoam`, does VOF plus phase
  change, but it is built for cavitation (evaporation/condensation of
  the same two fluids under pressure), not casting-style freezing by
  temperature, and would need rework.
  There is also an older, narrower `OpenFOAM-Solidification` repository
  contributed by Purdue researchers for columnar solidification of
  binary alloys, but its last commit is from 2018-10-08 (checked via the
  GitHub API), so it is effectively dormant:
  [github.com/OpenFOAM/OpenFOAM-Solidification](https://github.com/OpenFOAM/OpenFOAM-Solidification).
  `foam-extend` was checked for a ready-made casting solver too; nothing
  purpose-built for casting solidification turned up, only the general
  `interPhaseChangeFoam` family shared with mainline OpenFOAM.
* **Shrinkage porosity.** Not predicted directly by any stock solver.
  Would have to be inferred from cooling-rate/temperature-gradient
  fields after the fact, the same Niyama-criterion approach used
  elsewhere.
* **Material data.** None shipped for casting alloys. OpenFOAM ships
  generic thermophysical model infrastructure (`constant/thermophysicalProperties`)
  but no cast iron/A356/AZ91 entries; the user supplies density,
  viscosity, conductivity, specific heat, and latent heat by hand.
* **Output format.** OpenFOAM's own time-directory field format, readily
  read by ParaView (via the `paraFoam`/reader plugin) or converted to
  VTK/VTU.
* **Maintenance.** Both OpenFOAM.org and OpenFOAM.com are actively
  released (OpenFOAM.com shipped v2606 in June 2026). The gap is not the
  base toolbox, which is extremely well maintained, it is the absence of
  a maintained, purpose-built casting solver on top of it.

### "OpenCast": two different things, neither a ready open-source tool

Two unrelated things surface under this name, worth disentangling since
the project brief asked directly whether it exists:

1. An academic C++ die-casting code, actually named **OpenCast**, built
   at the University of Illinois at Urbana-Champaign (Shahane, Aluru,
   Ferreira, Kapoor, Vanka) with funding tied to the North American Die
   Casting Association. It uses a finite-volume method on unstructured
   hex/tet grids, Gmsh for meshing, and was tested on Ubuntu 16.04/18.04.
   It covers mold filling, solidification, heat transfer, and natural
   convection, and even wraps a formal uncertainty-quantification layer
   around the deterministic solve. Papers:
   [aluru.mechse.illinois.edu/Journals/AMM19.pdf](https://aluru.mechse.illinois.edu/Journals/AMM19.pdf),
   [arxiv.org/pdf/1810.08572](https://arxiv.org/pdf/1810.08572),
   [asmedigitalcollection.asme.org/.../474938](https://asmedigitalcollection.asme.org/manufacturingscience/article/141/4/041003/474938/Simulations-of-Die-Casting-With-Uncertainty).
   A GitHub/GitLab search for a public "OpenCast" die-casting repository
   turned up nothing; only unrelated screen-casting and media-streaming
   projects use the same name on GitHub. So despite the "open" framing
   in its own papers, this OpenCast does not appear to have a public
   downloadable source release. Treat it as a well-documented research
   result, not a tool Anvil can integrate with today.
2. **OCast** ([ocast.org](https://www.ocast.org/)), a similarly-spelled
   but completely unrelated open-source screen-casting/media project,
   not a casting simulator at all. Easy to confuse by name alone; worth
   flagging so nobody chases the wrong URL later.

### Elmer FEM (CSC, Finland)

Repository: [github.com/ElmerCSC/elmerfem](https://github.com/ElmerCSC/elmerfem).

* **License.** Split: the `ElmerSolver` core library is LGPL v2.1 (since
  2012), while `ElmerGUI`, `ElmerGrid`, `ElmerParam`, and most physics
  modules are GPL v2.1. See
  [elmerfem.org/blog/license](https://www.elmerfem.org/blog/license/).
* **Language.** Fortran, with a large C++ portion in places (GUI, some
  solvers).
* **Physics.** General-purpose multiphysics FEM: structural mechanics,
  heat transfer, electromagnetics, fluid dynamics, and (its best-known
  niche) glacier/ice-sheet modeling. It has an enthalpy-based phase
  change capability, `PhaseChangeSolve`/`TransientPhaseChange`, that
  folds latent heat into an effective heat capacity and has been used by
  forum users for casting-style solidification in a mold (see the
  [Elmer forum thread on phase change comparison with Comsol](https://www.elmerfem.org/forum/viewtopic.php?t=3846)
  and the [Elmer Models Manual](https://www.nic.funet.fi/index/elmer/doc/ElmerModelsManual.pdf)).
  It also has a free-surface Navier-Stokes capability, but that
  capability is built and documented around glacier surface evolution,
  not turbulent, splashing mold-filling flow, and no dedicated casting
  module, gating system model, or alloy database exists.
* **Verdict.** A real, well-maintained FEM toolbox (last push to the
  main repo was 2026-09-16, the day of this review) that could in
  principle be bent toward solidification-only analysis (skip the fill
  step, start from a "full mold" initial condition) but is not a casting
  tool out of the box the way Truchas is.

### Code_Saturne (EDF)

[code-saturne.org](https://code-saturne.org/en),
[github.com/code-saturne/code_saturne](https://github.com/code-saturne/code_saturne).
GPL-licensed, general-purpose industrial CFD from EDF (the French
electric utility), built for single-phase and, via the companion
`neptune_cfd` code, two-phase nuclear thermal-hydraulics work
(steam/water, air/water). It handles arbitrary unstructured meshes and
has combustion, MHD, and atmospheric-flow modules, but no solidification
or casting-specific physics were found anywhere in its documentation or
module list. Relevance to sand casting is low: it would take as much
custom solver development as OpenFOAM does, with a smaller casting-adjacent
user community to draw on.

### SU2

SU2 is an open-source (LGPL-licensed, originally Stanford-developed)
finite-volume solver built for compressible aerodynamics: RANS,
Euler, and design-optimization workflows for aircraft and turbomachinery.
It has no free-surface/VOF capability and no solidification or phase
change model, and its numerics target compressible, often supersonic or
transonic, external flow. It is a poor fit for an incompressible,
free-surface, phase-changing internal-flow problem like mold filling,
and was not investigated further.

### Nothing else genuinely open source turned up

No other actively maintained, source-available casting-specific code was
found. Academic CAFE (cellular automaton finite element) solidification
codes exist in the literature for grain-structure prediction, but the
ones found in searches are described in papers without an accompanying
public repository, the same pattern as the UIUC OpenCast code above.

## 2. Free but closed tools (brief)

* **PoligonSoft FREE**
  ([poligoncast.com/poligonsoftfreeversion](https://www.poligoncast.com/poligonsoftfreeversion)).
  Genuinely free, not a time-limited trial, from the Russian firm
  NPP PoligonSoft. Uses the same FEM core as the paid product, capped at
  500,000 elements. Accepts IGES and STEP for geometry (not STL).
  Windows 10 64-bit only. The free tier covers solidification, hot-spot,
  and macro/microporosity prediction, but explicitly disables mold
  filling flow calculation and stress analysis, and is licensed for
  education/academic/non-profit use only, not commercial production.
* **SOLIDCast** (Finite Solutions Inc.,
  [castingsimulation.com](https://www.castingsimulation.com/),
  [finite.solutions/en/Products/SOLIDCast](https://finite.solutions/en/Products/SOLIDCast)).
  A long-established PC solidification/shrinkage tool (the company says
  it originated PC-based casting simulation in 1985), Windows-only. No
  clearly free tier was found in this search, only trade literature and
  a workbook; treat it as commercial with an unconfirmed trial rather
  than a genuinely free option.
* **Others by reputation, not deeply checked here:** Flow-3D Cast,
  MAGMASOFT, ESI ProCAST, Click2Cast, AnyCasting, and Novacast
  NovaFlow&Solid all sell trial or academic licenses aimed at production
  foundries and die casters, all Windows-based, all a poor match for a
  hobbyist or small-shop workflow driven from a Linux/Rust CAD tool.
  None of these are worth Anvil integrating against; they are proprietary
  file formats behind a sales funnel.

## 3. Engineering models Anvil could implement directly

These are closed-form or curve-fit formulas, good for a fast, coarse
check inside the CAD tool itself, not a replacement for a real solver.

### Chvorinov's rule (solidification time)

    t = C * (V / A)^n

`t` is solidification time, `V` is casting volume, `A` is the mold-contact
surface area, `n` is usually taken as 2 (Askeland; DeGarmo gives a range
of 1.5 to 2 instead), and `C` is the mold constant, with SI units of
s/m^2 (also commonly quoted in s/cm^2 or s/in^2 in older US texts, so
units must be checked before using a published `C` value). `C` depends
on the metal (density, specific heat, latent heat, superheat) and the
mold (initial temperature, density, thermal conductivity, specific heat,
wall thickness) and is normally an experimentally fitted number for a
given metal/mold combination, not a universal constant. Source:
[Wikipedia, Chvorinov's rule](https://en.wikipedia.org/wiki/Chvorinov%27s_rule),
which cites Askeland's and DeGarmo's textbooks for the exponent values.
No single authoritative numeric table of `C` for grey iron or aluminum
in sand molds was found in this search; a practical path for Anvil is to
let the user calibrate `C` from one known pour of the same alloy and
mold material, rather than hard-code an unverified constant.

### Casting modulus and riser modulus rule

Modulus of a section is defined the same way as the V/A term above:

    M = V / A

Because solidification time scales with M^2 (from Chvorinov), a riser
freezes after the casting section it feeds only if the riser's modulus
is larger. The standard rule of thumb is:

    M_riser >= 1.2 * M_casting

(the 1.2 safety factor is the commonly cited figure). Source: NPTEL
lecture notes,
[elearn.psgcas.ac.in/nptel/courses/video/112107215/lec22.pdf](http://elearn.psgcas.ac.in/nptel/courses/video/112107215/lec22.pdf),
and multiple foundry course slide decks reachable from the same search
(e.g. the SlideShare riser-design decks found alongside it).

### Caine's method (riser sizing)

Caine's method relates a "freezing ratio" X, the ratio of (surface
area/volume) of the casting to (surface area/volume) of the riser, to
the required ratio Y of riser volume to casting volume, through an
empirical hyperbolic-type curve:

    X = a / (Y - b) + c

where X is the freezing ratio (SA/V of casting divided by SA/V of
riser), Y is riser volume divided by casting volume, and a, b, c are
constants fitted per alloy family from experiment. One commonly quoted
set, for steel, is a = 0.1, b = 0.03, c = 1.0; textbooks are explicit
that a, b, c must be refit per alloy/mold system, so treat any single
numeric triple as an example, not a universal constant. Caine's own
experimental work put the practical freezing ratio range at about 0.8
to 1.2. Source: course notes and slide decks surfaced by search, e.g.
[scribd.com/doc/86657573](https://www.scribd.com/doc/86657573/Riser-Design-Caines-Method-Compatibility-Mode)
and the EduRev worked problem
[edurev.in/question/2677636](https://edurev.in/question/2677636/Chvorinov-and-Caine-gave-rules-for-solidification-time-and-freezing-ratio-for-a-riser--A-cylindrical).

### Bernoulli/Torricelli fill time and sprue sizing

Metal falling through a sprue of head height `h` reaches, at the sprue
base, the Torricelli velocity:

    v = sqrt(2 * g * h)

with `g` = 9.81 m/s^2 and `h` in meters (v in m/s), consistent
dimensionally in any unit system. Combined with the continuity equation
(Q = A1*v1 = A2*v2) between the sprue top and base, a tapered sprue
sized to stay choked (full of metal, no air aspiration) needs a
base-to-top area ratio of:

    A_top / A_base = sqrt(h_base / h_top)

A commonly cited efficiency factor for a tapered sprue is about 0.74,
so the ideal area from the Bernoulli/continuity calculation should be
divided by about 0.74 (multiplied by about 1.35) to size the real sprue.
Overall pour time is often estimated separately with a simple empirical
form:

    t = k * sqrt(W)

where `t` is pour time, `W` is the pour weight in pounds, and `k` is an
empirical constant typically given in the range 0.4 to 1.2 (units
implied: seconds per sqrt(pound), i.e. this is a fitted engineering
formula, not a dimensionally pure physical law, so it needs its constant
recalibrated if used in SI). Sources: a Bangladesh University of
Professionals foundry course document,
[studocu.com/.../lec-13-calculation-of-gating-system-dimensions](https://www.studocu.com/row/document/bangladesh-university-of-professionals/math/lec-13-calculation-of-gating-system-dimensions/23195849),
and a general foundry design reference PDF,
[old.foundrygate.com/upload/artigos/K3ePtMbZRtz4gRWOTfVVjj3gbKc7.pdf](https://old.foundrygate.com/upload/artigos/K3ePtMbZRtz4gRWOTfVVjj3gbKc7.pdf).

### Gating ratios

Gating ratio is sprue-area : runner-area : ingate-area (total area at
each stage). Two broad families:

* **Pressurized systems** (choke at the ingates, smallest area last):
  good for metals that do not oxidize badly, notably grey and ductile
  iron. Example ratios seen: 1:2:1 and 1:1.1:1.2.
* **Unpressurized (naturally pressurized) systems** (choke at the sprue
  base, runners and gates get progressively bigger to slow the metal and
  fill gently): needed for metals that oxidize easily and tear their
  own oxide skin in turbulent flow, notably aluminum and magnesium.
  Example ratios seen: 1:2:4, 1:3:3, and a pressurized-style 1:1.2:1.0
  alternative was also quoted for aluminum in some sources, so treat the
  exact numbers as house-style ranges, not a single fixed rule, and pick
  the unpressurized family (choke at the sprue, expanding downstream)
  as the safe default for aluminum and magnesium. Source: foundry
  reference pages surfaced by search, e.g.
  [foundrymax.com/what-is-gating-system-5-key-components-in-metal-casting](https://foundrymax.com/what-is-gating-system-5-key-components-in-metal-casting/)
  and [giessereilexikon.com, "downsprue-runner gating ratio"](https://www.giessereilexikon.com/en/foundry-lexicon/Encyclopedia/show/downsprue-runner-gating-ratio-4352/?cHash=10bcbe12790f5d259161715e62c53f9b).

### Pouring temperatures

* **Grey cast iron:** about 1360 to 1450 C (about 2480 to 2640 F) is
  the pouring temperature range for sand casting; tapping temperature
  out of the furnace runs a little higher, about 1380 to 1420 C.
  Source:
  [zhycasting.com, "Effect of pouring temperature on the quality of sand castings"](https://www.zhycasting.com/effect-of-pouring-temperature-on-the-quality-of-sand-castings/).
* **Aluminum A356:** liquidus is about 615 C and solidus about 575 C, so
  any pour must clear 615 C by a useful superheat. Reported sand/permanent
  mold pouring practice spans roughly 620 to 730 C depending on section
  thickness (thin sections poured hotter to keep fluidity), with values
  around 700 C common in lost-foam and general foundry practice. Sources:
  China Foundry journal article on A356 pouring/cooling temperature
  effects,
  [link.springer.com/article/10.1007/s41230-019-9068-8](https://link.springer.com/article/10.1007/s41230-019-9068-8),
  and a general foundry-practice summary,
  [vietnamcastiron.com/aluminum-casting-temperature](https://vietnamcastiron.com/aluminum-casting-temperature/).
* **Magnesium AZ91 (AZ91D):** sand and permanent mold practice favors
  about 640 to 675 C; high-pressure die casting and lost-foam practice
  run hotter, roughly 700 to 730 C. Source: a Springer/International
  Journal of Metalcasting study on mold and pouring temperature effects
  on AZ91D hot tearing,
  [link.springer.com/article/10.1007/BF03355421](https://link.springer.com/article/10.1007/BF03355421).

### Material property values for a built-in model

Neither Truchas nor OpenFOAM ships alloy data, and a built-in Chvorinov
or voxel model needs liquidus, solidus, latent heat, and density to do
anything useful. These numbers disagree across sources more than the
pouring temperatures above, so treat them as starting points to
calibrate, not fixed constants:

* **Grey cast iron.** Liquidus is composition dependent (carbon
  equivalent), so a single number is unreliable. One source gives
  liquidus 1380 C and solidus 1180 C; a composition-specific source for
  a typical automotive grey iron gives liquidus 1160 to 1188 C. Use the
  lower figure as more representative of a near-eutectic grey iron and
  let the user override it. Latent heat of fusion is often quoted around
  280 J/g, but a dedicated study, "Evaluation of latent heat of
  solidification of grey cast iron from cooling curves,"
  [Canadian Metallurgical Quarterly, 44(1), 2005](https://www.tandfonline.com/doi/abs/10.1179/cmq.2005.44.1.1),
  is a better source to pull an exact value from than a generic web
  number. Solid density is about 7150 kg/m^3
  ([vcalc.com](https://www.vcalc.com/wiki/density-of-gray-cast-iron));
  liquid density is approximately 6980 kg/m^3, but no single
  authoritative source was found for that figure, so flag it as
  approximate. See also
  [matweb.com](https://www.matweb.com/search/datasheet.aspx?MatGUID=f3cd25980ab24fdaa5893252cd2bc192)
  and
  [azom.com](https://www.azom.com/properties.aspx?ArticleID=783).
* **A356.0 aluminum.** Liquidus 615 C, solidus 555 C, per
  [MakeItFrom](https://www.makeitfrom.com/material-properties/A356.0-F-Cast-Aluminum).
  Latent heat is reported two ways, about 389 kJ/kg and about 500 kJ/kg;
  the lower figure is closer to typical Al-Si eutectic-ish alloy values
  in the literature, so prefer it, but cite both until confirmed.
  Solid density is about 2680 kg/m^3
  ([sunrise-metal.com](https://www.sunrise-metal.com/density-of-aluminum-and-aluminum-alloys/));
  liquid density near the liquidus is typically 2380 to 2450 kg/m^3 for
  Al-Si alloys in the wider literature, but this was not directly
  sourced here and needs its own citation before being hard-coded.
* **AZ91D magnesium.** Liquidus about 595 C, solidus about 470 C.
  Latent heat is reported as about 373 kJ/kg and, separately, about 350
  kJ/kg, close enough to treat as consistent; the
  [diva-portal AZ91D doctoral thesis](https://www.diva-portal.org/smash/get/diva2:1165186/FULLTEXT01.pdf)
  is a strong primary source for exact values. Density figures conflict:
  one generic magnesium-alloy source gives 1740 kg/m^3, but a more
  AZ91-specific figure is closer to 1810 kg/m^3 solid; liquid density
  was not found. Magnesium also needs a protective atmosphere or flux
  during melting and pouring because it is flammable near these
  temperatures, a safety point worth carrying into any Anvil pouring
  guidance, not just the simulation model.

None of these numbers should be hard-coded as ground truth. The safer
design is a small, editable material table (already in the spirit of
Anvil's expression system) seeded with these values and a clear "verify
before production use" note, the same way `C` in Chvorinov's rule above
should be calibrated, not assumed.

### Shrinkage allowance (pattern oversize)

Linear pattern allowance, applied as `pattern_dim = nominal_dim * (1 +
shrink_fraction)`:

* Grey/gray cast iron: about 0.6 to 1.0 percent linear.
* Aluminum alloys: reported figures vary a lot by source, from about
  1.3 to 1.6 percent up to 3.5 to 4.0 percent depending on alloy and
  section; a mid-single-digit-percent default with a user override is
  the safer design than one hard-coded number.
* Magnesium: no specific figure was confirmed in this search; magnesium
  alloys are generally reported (elsewhere in foundry literature, not
  independently re-verified here) to shrink somewhat more than aluminum,
  so treat any magnesium default as unconfirmed until checked against an
  alloy datasheet.

Source:
[casting-china.org, "5 Types of Pattern Allowances in Casting"](https://casting-china.org/5-types-of-pattern-allowances-in-casting/)
and
[langhe-industry.com, "Metal Shrinkage in Castings"](https://langhe-industry.com/metal-shrinkage-in-castings/).
Given how much these numbers disagree across sources, Anvil should treat
shrinkage allowance as a per-project expression, not a baked-in
constant, the same way lengths and angles are already expressions
elsewhere in the tool.

## 4. Recommendation

**Export target: Truchas first.** It is the only option found that is
actually built for casting (free-surface fill plus phase-change
solidification in one coupled solver), actively maintained (commits
within the last two months of this review), permissively licensed
(3-clause BSD), and has a real example input deck showing the exact
namelist shape needed
(`test/freezing-flow/freezing-flow-1.inp` in the repo). The build is
real work, Fortran, MPI, a two-stage TPL-then-solver CMake build, but it
is documented and the project ships CI binary packages, so a user does
not have to build from source to try it. OpenFOAM is the fallback: its
base toolbox is easier to get on a Linux box (apt packages, Docker
images) and its STL-via-snappyHexMesh path is more convenient than
Truchas's Exodus II requirement, but there is no ready-made,
maintained casting solver in stock OpenFOAM, only a free-surface
solver (`interFoam`) and a separately maintained solidification
`fvOption` that someone still has to wire together.

**Export path for Anvil.** The natural sequence, matching the ribbon
plan already sketched in `docs/KETTLE.md` stage 3-4:

1. Anvil exports the filled mold cavity (the negative space the metal
   occupies: casting plus sprue, runners, gates, risers) as a
   watertight STL, the same triangle mesh format Anvil already writes.
2. For Truchas: convert STL to a volume mesh in Exodus II format. This
   is the real friction point: Truchas wants Exodus II, not STL, and no
   direct STL-to-Exodus path lives inside Truchas itself. The practical
   route is Gmsh (already the meshing backend referenced by both the
   Truchas-adjacent tooling and the UIUC OpenCast paper, and already
   proposed as the FEA meshing backend for Anvil in
   `docs/research/04-cam-simulation-and-formats.md`): mesh the STL to a
   volume mesh in Gmsh, then export to Exodus II (or export to a format
   a small conversion step turns into Exodus II) before handing it to
   Truchas. For OpenFOAM: no conversion needed, `snappyHexMesh` reads
   STL directly.
3. Anvil writes the physics-and-material namelists (Truchas) or
   dictionaries (OpenFOAM) itself, since it already knows the alloy,
   pour temperature, and mold material chosen in the CAD document; the
   user should not have to hand-type a `&MATERIAL` block Anvil already
   has the numbers for.
4. Anvil shells out to the solver binary (matching the
   shell-out-plus-file-interchange pattern already recommended for FEA
   in `docs/research/04-cam-simulation-and-formats.md`'s FreeCAD FEM
   comparison), waits for it to finish, then reads back Exodus II or
   OpenFOAM results for a built-in viewer, reusing the `vtkio` crate
   path already scoped for FEA results.

**What a built-in coarse model can reasonably do**, without any external
solver, as a first cut inside Anvil before the export path exists:

* A voxel fill check: rasterize the mold cavity mesh to a coarse voxel
  grid, and do a simple top-down or gravity-direction flood/fill
  simulation (fill from the sprue entry point downward and outward,
  respecting the voxel occupancy) to catch obvious misruns, trapped air
  pockets, or a sprue/riser placed somewhere metal cannot reach. This
  is not real fluid dynamics, no momentum, no turbulence, but it is
  enough to catch a kettle spout or lug placed where the mold cannot
  fill.
* A per-voxel or per-section Chvorinov solidification-time estimate,
  using each local region's local V/A (computed from the voxel
  neighborhood) to build a coarse "solidification order" map. Compare
  neighboring regions: if a thin section between two thicker ones
  solidifies first, that is an isolated liquid pocket and a shrinkage
  risk, exactly the hot-spot check commercial tools sell. This reuses
  Chvorinov directly, no new physics, and is cheap to compute even on a
  fine voxel grid.
* Riser modulus check: once a riser is placed as its own body, compute
  its modulus and each feeding casting section's modulus, and flag any
  section whose local modulus is not comfortably below the riser
  modulus (the M_riser >= 1.2 * M_casting rule above), plus a basic
  Caine volume check for riser size.
* Sprue and gate sizing calculator: given pour height, casting weight,
  target fill time, and alloy family (iron vs. light alloy, to pick a
  pressurized vs. unpressurized gating ratio family), compute sprue
  base area from Torricelli/continuity and suggested runner/gate areas
  from the gating ratio tables above. This is pure arithmetic, a good
  fit for Anvil's existing expression system, since sprue/runner/gate
  dimensions could be expressions driven by casting weight and alloy
  choice the way other feature dimensions already are.

None of this replaces a real free-surface, turbulent, thermally coupled
solve, which is exactly why Truchas (or, with more integration work,
OpenFOAM) stays the answer for a trustworthy go/no-go on a specific
kettle geometry. But the built-in checks above are cheap, fast, run on
every regeneration the way other feature checks do, and catch the most
common sand-casting mistakes (a mold that cannot fill, a riser that is
too small or badly placed, a gating system too violent for the alloy)
without leaving the CAD tool or waiting on an external solver run.

## References

All URLs used in this report, grouped by topic.

**Truchas**
- [github.com/lanl/truchas](https://github.com/lanl/truchas)
- [gitlab.com/truchas/truchas](https://gitlab.com/truchas/truchas)
- [github.com/truchas/truchas](https://github.com/truchas/truchas)
- [LICENSE.md](https://gitlab.com/truchas/truchas/-/raw/master/LICENSE.md)
- [README.md](https://gitlab.com/truchas/truchas/-/raw/master/README.md)
- [BUILDING.md](https://gitlab.com/truchas/truchas/-/raw/master/BUILDING.md)
- [Truchas Reference Manual, Introduction](https://www.truchas.org/docs/reference-manual/introduction/index.html)
- [truchas.org](https://www.truchas.org)
- [test/freezing-flow/freezing-flow-1.inp](https://gitlab.com/truchas/truchas/-/raw/master/test/freezing-flow/freezing-flow-1.inp)
- [Truchas, a multi-physics tool for casting simulation (ResearchGate)](https://www.researchgate.net/publication/233571093_Truchas_-_A_multi-physics_tool_for_casting_Simulation)
- [GitLab commits API result used for last-commit date](https://gitlab.com/api/v4/projects/truchas%2Ftruchas/repository/commits)

**OpenFOAM and casting solidification**
- [openfoam.org (Foundation)](https://openfoam.org/)
- [openfoam.com (ESI-OpenCFD/Keysight)](https://www.openfoam.com/)
- [openfoam.com current release](https://www.openfoam.com/current-release)
- [snappyHexMesh user guide section](https://www.openfoam.com/documentation/user-guide/4-mesh-generation-and-conversion/4.4-mesh-generation-with-the-snappyhexmesh-utility)
- [solidificationMeltingSource.H, OpenFOAM-6](https://github.com/OpenFOAM/OpenFOAM-6/blob/master/src/fvOptions/sources/derived/solidificationMeltingSource/solidificationMeltingSource.H)
- [OpenFOAM-Solidification repository](https://github.com/OpenFOAM/OpenFOAM-Solidification)
- [interPhaseChangeFoam source](https://cpp.openfoam.org/v8/interPhaseChangeFoam_8C_source.html)
- [solidificationInterFoam Chalmers course report](https://www.tfd.chalmers.se/~hani/kurser/OS_CFD_2024/PariaKhosravifar/solidificationInterFoam_Paria_Khosravifar.pdf)
- [OpenFOAM Wikipedia entry](https://en.wikipedia.org/wiki/OpenFOAM)

**"OpenCast" and OCast**
- [aluru.mechse.illinois.edu/Journals/AMM19.pdf](https://aluru.mechse.illinois.edu/Journals/AMM19.pdf)
- [Finite Volume Simulation Framework for Die Casting with Uncertainty Quantification (arXiv)](https://arxiv.org/pdf/1810.08572)
- [Simulations of Die Casting With Uncertainty Quantification (ASME)](https://asmedigitalcollection.asme.org/manufacturingscience/article/141/4/041003/474938/Simulations-of-Die-Casting-With-Uncertainty)
- [ocast.org](https://www.ocast.org/)

**Elmer FEM**
- [github.com/ElmerCSC/elmerfem](https://github.com/ElmerCSC/elmerfem)
- [elmerfem.org license page](https://www.elmerfem.org/blog/license/)
- [Elmer Models Manual](https://www.nic.funet.fi/index/elmer/doc/ElmerModelsManual.pdf)
- [Elmer forum, phase change comparison with Comsol](https://www.elmerfem.org/forum/viewtopic.php?t=3846)

**Code_Saturne and SU2**
- [code-saturne.org](https://code-saturne.org/en)
- [github.com/code-saturne/code_saturne](https://github.com/code-saturne/code_saturne)
- [Code Saturne Wikipedia entry](https://en.wikipedia.org/wiki/Code_Saturne)

**Closed free tools**
- [PoligonSoft FREE](https://www.poligoncast.com/poligonsoftfreeversion)
- [PoligonSoft main site](https://www.poligoncast.com/)
- [SOLIDCast / castingsimulation.com](https://www.castingsimulation.com/)
- [Finite Solutions SOLIDCast product page](https://finite.solutions/en/Products/SOLIDCast)

**Chvorinov's rule and modulus method**
- [Wikipedia, Chvorinov's rule](https://en.wikipedia.org/wiki/Chvorinov%27s_rule)
- [NPTEL lecture 22 PDF](http://elearn.psgcas.ac.in/nptel/courses/video/112107215/lec22.pdf)
- [giessereilexikon.com, Chvorinov's rule entry](https://www.giessereilexikon.com/en/foundry-lexicon/Encyclopedia/show/chvorinovs-rule-4627/?cHash=2d0751fb7828d4549443fd324577ef4f)

**Caine's method**
- [scribd.com, Riser Design Caine's Method](https://www.scribd.com/doc/86657573/Riser-Design-Caines-Method-Compatibility-Mode)
- [EduRev worked Caine's method problem](https://edurev.in/question/2677636/Chvorinov-and-Caine-gave-rules-for-solidification-time-and-freezing-ratio-for-a-riser--A-cylindrical)

**Gating: Bernoulli/Torricelli fill time and gating ratios**
- [Bangladesh University of Professionals, gating system dimensions lecture](https://www.studocu.com/row/document/bangladesh-university-of-professionals/math/lec-13-calculation-of-gating-system-dimensions/23195849)
- [Design of Gating and Feeding Systems (foundrygate.com PDF)](https://old.foundrygate.com/upload/artigos/K3ePtMbZRtz4gRWOTfVVjj3gbKc7.pdf)
- [foundrymax.com, gating system components](https://foundrymax.com/what-is-gating-system-5-key-components-in-metal-casting/)
- [giessereilexikon.com, downsprue-runner gating ratio](https://www.giessereilexikon.com/en/foundry-lexicon/Encyclopedia/show/downsprue-runner-gating-ratio-4352/?cHash=10bcbe12790f5d259161715e62c53f9b)

**Pouring temperatures**
- [zhycasting.com, effect of pouring temperature on sand castings](https://www.zhycasting.com/effect-of-pouring-temperature-on-the-quality-of-sand-castings/)
- [China Foundry, A356 pouring/cooling temperature effects](https://link.springer.com/article/10.1007/s41230-019-9068-8)
- [vietnamcastiron.com, aluminum casting temperature](https://vietnamcastiron.com/aluminum-casting-temperature/)
- [International Journal of Metalcasting, AZ91D mold/pouring temperature and hot tearing](https://link.springer.com/article/10.1007/BF03355421)

**Shrinkage allowance**
- [casting-china.org, 5 Types of Pattern Allowances in Casting](https://casting-china.org/5-types-of-pattern-allowances-in-casting/)
- [langhe-industry.com, Metal Shrinkage in Castings](https://langhe-industry.com/metal-shrinkage-in-castings/)

**Material property values**
- [Canadian Metallurgical Quarterly 44(1), 2005, latent heat of grey cast iron](https://www.tandfonline.com/doi/abs/10.1179/cmq.2005.44.1.1)
- [vcalc.com, density of gray cast iron](https://www.vcalc.com/wiki/density-of-gray-cast-iron)
- [matweb.com, grey cast iron datasheet](https://www.matweb.com/search/datasheet.aspx?MatGUID=f3cd25980ab24fdaa5893252cd2bc192)
- [azom.com, grey cast iron properties](https://www.azom.com/properties.aspx?ArticleID=783)
- [MakeItFrom, A356.0-F cast aluminum](https://www.makeitfrom.com/material-properties/A356.0-F-Cast-Aluminum)
- [sunrise-metal.com, density of aluminum and aluminum alloys](https://www.sunrise-metal.com/density-of-aluminum-and-aluminum-alloys/)
- [diva-portal.org, AZ91D doctoral thesis](https://www.diva-portal.org/smash/get/diva2:1165186/FULLTEXT01.pdf)
