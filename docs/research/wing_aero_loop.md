# Parametric wing modeller with an aerodynamic feedback loop

Written 2026-09-17. Research for a demo on `anvil-implicit`: model an
aircraft quickly, change airfoil and planform parametrically, get an
aerodynamic estimate in milliseconds, and let an optimiser drive the
parameters. Each section is a 2D SDF and the wing is a loft of sections.

## 1. Airfoil parametrisations

**NACA 4-digit** `MPTT`: `m = M/100` max camber, `p = P/10` its
position, `t = TT/100` thickness, fractions of chord, `x` in `[0, 1]`.
Half-thickness ([Wikipedia](https://en.wikipedia.org/wiki/NACA_airfoil),
[PDAS](https://www.pdas.com/naca456thick4.html)):

`y_t = 5 t (0.2969 sqrt(x) - 0.1260 x - 0.3516 x^2 + 0.2843 x^3 - 0.1015 x^4)`

At `x = 1` this leaves a gap of `0.0021 t`; replacing the last
coefficient by `-0.1036` makes the coefficients sum to zero and closes
the trailing edge with the least change of shape. Camber line:

`y_c = (m/p^2) (2 p x - x^2)` for `x <= p`;
`y_c = (m/(1-p)^2) ((1 - 2p) + 2 p x - x^2)` for `x > p`.

Surface points sit normal to the camber line with
`theta = atan(dy_c/dx)`: `x_u = x - y_t sin(theta)`,
`y_u = y_c + y_t cos(theta)`, `x_l = x + y_t sin(theta)`,
`y_l = y_c - y_t cos(theta)`. Use cosine spacing in `x` to resolve the
nose.

**NACA 5-digit** `LPSTT`: design lift `C_L = 0.15 L`, max camber at
`P/20`, `S = 0` simple or `1` reflexed, `TT` thickness. Same thickness
polynomial; simple camber line
`y_c = (k1/6) (x^3 - 3 r x^2 + r^2 (3 - r) x)` for `x <= r`,
`y_c = (k1 r^3 / 6)(1 - x)` aft, with `(p, r, k1)` tabulated:
210 `(0.05, 0.0580, 361.40)`, 220 `(0.10, 0.126, 51.640)`,
230 `(0.15, 0.2025, 15.957)`, 240 `(0.20, 0.290, 6.643)`,
250 `(0.25, 0.391, 3.230)` ([Wikipedia](https://en.wikipedia.org/wiki/NACA_airfoil)).
Worth having because 23012 and 23015 are common.

**CST (Kulfan)** ([Kulfan 2008, J. Aircraft 45(1)](https://doi.org/10.2514/1.29958);
formulas also in [Grey and Constantine](https://arxiv.org/pdf/1702.02909)):
each surface is `z(x) = C(x) S(x) + x z_TE`, class function
`C(x) = x^N1 (1 - x)^N2` with `N1 = 0.5`, `N2 = 1.0` (round nose, sharp
tail), shape function a Bernstein sum

`S(x) = sum_{i=0}^{n} A_i K_i x^i (1 - x)^(n-i)`, `K_i = n! / (i! (n-i)!)`.

`A_0 = sqrt(2 R_LE / c)` sets the nose radius and `A_n` the trailing
edge angle. Typical orders are `n = 5` to `8` per surface, 12 to 18
coefficients in total. Any point set can be least-squares fitted to CST,
which makes it the standard design-variable set: smooth, compact, linear
in the coefficients.

**PARSEC** (Sobieczky, [H182](https://sobieczky.at/aero/literature/H182.pdf)):
each surface is `z = sum_{n=1}^{6} a_n x^(n - 1/2)`, the 12 `a_n` solved
from a linear system built from 11 geometric parameters: `r_LE`, upper
crest `(x_up, z_up, z_xx_up)`, lower crest `(x_lo, z_lo, z_xx_lo)`,
trailing edge `z_TE`, thickness `dz_TE`, direction `alpha_TE`, wedge
angle `beta_TE`. Intuitive sliders, but odd inputs give crossed
surfaces, so it needs validity checks.

**UIUC database, Selig format** ([UIUC](https://m-selig.ae.illinois.edu/ads/coord_database.html)):
about 1650 `.dat` files. Line one is the name, then `x y` pairs from the
upper trailing edge, round the nose, to the lower trailing edge; files
may contain `#` comment lines and CRLF endings. The Lednicer variant
lists upper then lower surface from the nose after a point-count line;
detect it by a first pair greater than 1. XFOIL's `LOAD` accepts Selig
ordering in either direction.

**Recommendation.** NACA 4-digit first (five lines, exercises the
pipeline), then the Selig loader (real airfoils), then CST with a fitter
from any point set (optimiser variables). Skip PARSEC unless a slider UI
is wanted. Store each section as a closed polyline (200 to 400
cosine-spaced points) with an exact 2D polygon distance and winding
sign; the polyline is what XFOIL and AVL consume anyway.

## 2. Planform and loft

Parameters: span `b`, root chord `c_r`, tip chord `c_t`, taper
`lambda = c_t / c_r`, sweep `Lambda` (state the line: leading edge,
quarter chord or half chord), dihedral, twist (washout, tip incidence 2
to 4 deg below root), root incidence, and a section per station. Sweep
at chord fraction `n` follows from sweep at fraction `m` by
`tan(Lambda_n) = tan(Lambda_m) - (4/AR) (n - m) (1 - lambda)/(1 + lambda)`
(Raymer, *Aircraft Design*, ch. 4; Etkin,
[appendix C](http://aero.us.es/adesign/Slides/Extra/Stability/Mean%20Aerodynamic%20Chord,%20&%20Mean%20Aerodynami%20Center%20-%20Appendix%20C%20(Etkin).pdf)).

Closed forms for a trapezoidal wing
([NACA Report 751](https://ntrs.nasa.gov/api/citations/19930091829/downloads/19930091829.pdf)):

`S = b (c_r + c_t) / 2`, `AR = b^2 / S`,
`MAC = (2/3) c_r (1 + lambda + lambda^2) / (1 + lambda)`,
`y_MAC = (b/6) (1 + 2 lambda) / (1 + lambda)`.

For a multi-panel wing (kinks, winglets) integrate `c(y)` and `c(y)^2`
per panel. A winglet is a panel with dihedral near 90 deg; lifting-line
cannot model it, VLM can.

**Loft.** A station is `(y, x_le, z_le, chord, twist, section)`. The
surface is the section polyline scaled by `chord`, rotated by `twist`
about its quarter chord, translated to `(x_le, z_le)`, swept over `y`.
As an SDF: from the query point find the span parameter `s` (linear in
`y` for straight-edged panels), interpolate the two bounding station
transforms and section polylines (equal point counts, matched by index),
take the 2D polygon distance in the local section plane and combine
with the span bound by the box-extrusion formula. Not the exact
Euclidean distance for swept or twisted panels, but a bound with the
right zero set, which is all the mesher needs. Blend section shapes by
interpolating CST coefficients rather than points.

## 3. Fast aerodynamic estimates

Standard number used throughout: section lift slope `a_0 = 2 pi` per
radian from thin-airfoil theory; XFOIL gives about 6.0 to 6.3 for real
sections.

**Thin-airfoil theory** (Anderson, *Fundamentals of Aerodynamics*,
ch. 4; [Cadence](https://resources.system-analysis.cadence.com/blog/2023-an-overview-of-thin-airfoil-theory)).
With `x = (c/2)(1 - cos(theta))` and camber slope `dz/dx`:

`c_l = 2 pi (alpha - alpha_L0)`,
`alpha_L0 = -(1/pi) int_0^pi (dz/dx)(cos(theta) - 1) dtheta`,
`c_m,c/4 = (pi/4)(A_2 - A_1)`, `A_n = (2/pi) int_0^pi (dz/dx) cos(n theta) dtheta`.

One quadrature over the camber line, microseconds; thickness has no
effect. This is the section input lifting-line needs.

**Prandtl lifting-line** (Anderson ch. 5;
[ERAU](https://eaglepubs.erau.edu/introductiontoaerospaceflightvehicles/chapter/lifting-line-theory/)).
Map span to `y = -(b/2) cos(theta)`, `theta` in `[0, pi]`, and expand
`Gamma(theta) = 2 b V sum_{n=1}^{N} A_n sin(n theta)`. The monoplane
equation at each station `theta_k`:

`alpha(theta_k) - alpha_L0(theta_k) = (4b / (a_0 c(theta_k))) sum A_n sin(n theta_k) + sum n A_n sin(n theta_k)/sin(theta_k)`

Pick `N` stations (`theta_k = k pi/(N+1)`, odd `n` only for a symmetric
wing), solve the `N x N` system for `A_n`, then

`C_L = pi AR A_1`, `C_Di = pi AR sum n A_n^2 = C_L^2 (1 + delta)/(pi AR)`,
`delta = sum_{n>=2} n (A_n/A_1)^2`, `e = 1/(1 + delta)`.

Geometric twist enters through `alpha(theta)`, aerodynamic twist through
`alpha_L0(theta)`, taper through `c(theta)`. An elliptic wing gives
`delta = 0`; an untwisted tapered wing has `delta` about 0.01 to 0.02
near `lambda = 0.3 to 0.4`
([USU](https://digitalcommons.usu.edu/cgi/viewcontent.cgi?article=1010&context=mae_stures)).
Finite-wing lift slope `a = a_0 / (1 + a_0 (1 + tau)/(pi AR))`, `tau`
0.05 to 0.25. `N = 20 to 50` costs well under a millisecond. Valid for
straight wings with `AR > 4`; sweep and dihedral are outside the theory.

**Vortex lattice** (Katz and Plotkin, *Low-Speed Aerodynamics*, ch. 12;
[Wikipedia](https://en.wikipedia.org/wiki/Vortex_lattice_method),
[PyTornado theory](https://pytornado.readthedocs.io/en/latest/theory/index.html)).
Divide the camber surface into `m x n` panels. Each carries a horseshoe
vortex: bound leg on the panel quarter-chord line, trailing legs from
its ends downstream to infinity along the freestream. The control point
is at the panel three-quarter chord (the "1/4 - 3/4 rule", which
recovers `2 pi` for a flat plate). Induced velocity at `P` from a
straight segment `A -> B`, with `r_1 = P - A`, `r_2 = P - B`,
`r_0 = B - A`:

`v = (Gamma / (4 pi)) (r_1 x r_2) / |r_1 x r_2|^2 * r_0 . (r_1/|r_1| - r_2/|r_2|)`

with a core cutoff when `|r_1 x r_2|` is tiny; a semi-infinite leg from
`A` along unit `u` gives `v = (Gamma / (4 pi)) (u x r_1) / (|r_1| (|r_1| - u . r_1))`.
Assemble `a_ij = w_ij . n_i` (velocity at control point `i` from a
unit horseshoe `j`, dotted with the panel normal), solve
`a Gamma = -V_inf . n`, then per panel
`F_i = rho Gamma_i (V_inf + v_i) x l_i` with `l_i` the bound leg and
`v_i` the velocity induced by all other vortices at its midpoint (this
form yields induced drag directly). Sum forces and moments about the
reference point, divide by `q S` and `q S MAC`.

VLM predicts `C_L`, `C_Di`, `C_m`, spanwise loading, the effects of
sweep, dihedral, twist, winglets, tails and control deflections, and
stability derivatives by finite differences. It cannot predict thickness
effects, profile drag, stall, or transonic effects beyond
Prandtl-Glauert scaling. A `10 x 40` lattice per wing is a 400 x 400
dense solve, 5 to 20 ms in Rust; a whole aircraft is a few thousand
unknowns and around a second.

**2D panel method (Hess-Smith)**
([Stanford AA200b](http://aero-comlab.stanford.edu/aa200b/lect_notes/lect3-4.pdf),
[Reusser](https://rreusser.github.io/notebooks/hess-smith-panel-solver/)).
`N` flat panels, constant source `q_j` per panel plus one shared vortex
strength; `N` tangency equations at panel midpoints and one Kutta
equation (equal tangential velocity on the two trailing-edge panels)
give an `(N+1) x (N+1)` system. Output `C_p = 1 - (V/V_inf)^2` and
`c_l = 2 Gamma / (V_inf c)`. Inviscid, zero drag, but real thickness
effects; 100 to 200 panels in under a millisecond.

**Profile drag build-up** (Raymer ch. 12;
[MIT page](https://computationaldesignlab.github.io/aircraft-design/aerodynamics/drag_buildup.html)):
`C_D0 = sum_c C_f,c FF_c Q_c S_wet,c / S_ref`, laminar
`C_f = 1.328 / sqrt(Re)`, turbulent
`C_f = 0.455 / (log10(Re))^2.58 / (1 + 0.144 M^2)^0.65`, wing form factor
`FF = (1 + 0.6/(x/c)_m (t/c) + 100 (t/c)^4)(1.34 M^0.18 cos(Lambda_m)^0.28)`
with `(x/c)_m` the max-thickness position, fuselage
`FF = 1 + 60/f^3 + f/400` for fineness `f = l/d`, `Q = 1.0` for wing and
body, 1.04 for tails, `S_wet ~ 2 S_exposed (1 + 0.25 t/c)`. Then
`C_D = C_D0 + C_L^2 / (pi e AR)`. Trivial cost, and the only estimate
that knows about Reynolds number.

**Implementation order.** (1) NACA and Selig sections plus thin-airfoil
`alpha_L0`. (2) Lifting-line with twist and taper: `C_L`, `C_Di`, `e`
and a live loading plot. (3) Drag build-up: `L/D`. (4) VLM, as soon as
sweep, dihedral, a tail or winglets appear. (5) Hess-Smith, only for an
on-screen `C_p` plot without XFOIL. All are pure functions of the
parameter vector and cost less than meshing the SDF.

## 4. External tools

| Tool | Physics | Input | Output | Effort and run time |
| --- | --- | --- | --- | --- |
| [XFOIL](https://web.mit.edu/drela/Public/web/xfoil/) | 2D panel plus integral boundary layer, viscous, transition | Selig point file, stdin commands | polar text file | trivial to drive; 0.1 to 2 s per polar |
| [AVL](https://web.mit.edu/drela/Public/web/avl/) | 3D VLM plus slender body, stability | `.avl` text file | `ft`, `st` text dumps | small; under 1 s |
| [OpenVSP / VSPAERO](https://openvsp.org/wiki/doku.php?id=vspaerotutorial) | VLM or panel method, actuator disks | OpenVSP model (DegenGeom for VLM, `.tri` for panels) | `.polar`, `.lod`, `.history` | Python API; seconds to minutes |
| [SU2](https://su2code.github.io/) | Euler / RANS, continuous and discrete adjoint | `.su2` or CGNS mesh, `.cfg` | forces, surface sensitivities | mesh via Gmsh; 2D airfoil minutes, 3D RANS wing hours |
| [OpenFOAM](https://openfoamwiki.net/index.php/SnappyHexMesh) | RANS (simpleFoam), snappyHexMesh from STL | STL plus case directory | forceCoeffs log | most setup; 1 to 10 M cells, 0.5 to several hours |

Only OpenFOAM (snappyHexMesh) and VSPAERO's panel mode accept a
triangulated surface directly; XFOIL and AVL take 2D point files; SU2
needs a volume mesh.

**XFOIL** reads commands from stdin
([doc](https://web.mit.edu/drela/Public/web/xfoil/xfoil_doc.txt),
[Tiftikci](https://hakantiftikci.wordpress.com/2010/12/21/using-xfoil-and-automating-via-python-subprocess-module/)):

```
LOAD foil.dat
PANE
OPER
VISC 500000
MACH 0.1
ITER 200
PACC
polar.txt

ASEQ -4 14 1

QUIT
```

The polar has header lines then columns
`alpha CL CD CDp CM Top_Xtr Bot_Xtr`. Non-converged points are skipped;
use a timeout and kill on a hang. `NACA 2412` needs no file.
[NeuralFoil](https://github.com/peterdsharpe/NeuralFoil) is a surrogate
trained on XFOIL, about 5 ms per call, always converges, weights public,
so a Rust port is feasible.

**AVL** file ([doc](https://web.mit.edu/drela/Public/web/avl/avl_doc.txt)):
header (title, Mach, `iYsym iZsym Zsym`, `Sref Cref Bref`,
`Xref Yref Zref`), then per surface

```
SURFACE
Wing
12 1.0  30 -2.0       ! Nchord Cspace Nspan Sspace
YDUPLICATE 0.0
ANGLE 2.0
SECTION
0.0 0.0 0.0  1.5 0.0  ! Xle Yle Zle Chord Ainc
AFILE root.dat
SECTION
0.8 5.0 0.3  0.6 -2.0
NACA 2412
```

plus `CDCL` for a three-point section drag polar (from XFOIL), `CONTROL`
for hinges, `BODY` with `BFILE` for a fuselage profile. Run `avl plane.avl`,
then `OPER`, `a c 0.5` (alpha for `C_L = 0.5`), `x`, `ft`, `st`. The
station list is the loft's own, so the writer is a few dozen lines, and
AVL is the natural check for the in-house VLM.

**OpenVSP** exports STL, `.tri`, STEP and IGES and has its own
`AIRFOIL FILE` format ([wiki](https://openvsp.org/wiki/doku.php?id=files));
a reference tool, not a dependency.

**SU2** ([airfoil tutorial](https://su2code.github.io/tutorials/Inviscid_2D_Unconstrained_NACA0012/),
[mesh docs](https://su2code.github.io/docs/Mesh-File/)) needs a volume
mesh (`.su2` ASCII or CGNS, from Gmsh or Pointwise).
`shape_optimization.py -g CONTINUOUS_ADJOINT -o SLSQP -f case.cfg` runs
`SU2_CFD` (flow and adjoint), `SU2_DOT` (sensitivities onto design
variables: Hicks-Henne bumps in 2D, an FFD box in 3D) and `SU2_DEF`
(mesh deformation) under SciPy SLSQP; `DRAG` with a `LIFT` constraint is
the standard case. 2D inviscid: under a minute per iteration; 3D RANS
wing with adjoint: hours.

**OpenFOAM** ([external aero setup](https://dev.to/spondon_saha_102/how-to-set-up-an-external-aerodynamics-case-in-openfoam-23hc),
[motorBike](https://github.com/OpenFOAM/OpenFOAM-6/blob/master/tutorials/mesh/snappyHexMesh/motorBike))
takes the `surface_nets` STL directly. Minimal case:
`constant/triSurface/wing.stl`, `system/blockMeshDict` (box about 5
chords upstream, 10 downstream, 3 to each side), `surfaceFeatureExtract`,
`snappyHexMeshDict` (castellate, snap, `addLayers` with 5 layers,
`locationInMesh`, a refinement box), `0/{U,p,k,omega,nut}` with
`kOmegaSST`, `controlDict` with a `forceCoeffs` function object,
`simpleFoam` for 500 to 2000 iterations. All text, so Anvil can write
the case. SU2 and OpenFOAM are validation tools, not loop members.

## 5. Optimisation loops

Gradient-free methods fit the fast estimators: Nelder-Mead for 2 to 6
parameters and a few hundred evaluations; CMA-ES for 10 to 30 CST plus
planform parameters, thousands of evaluations, robust to noise; Bayesian
optimisation (Gaussian process, expected improvement) when an evaluation
costs seconds or more (XFOIL, VSPAERO), 50 to 300 evaluations. Rust has
[argmin](https://argmin-rs.org/) (Nelder-Mead, CMA-ES, L-BFGS) and
[egobox](https://github.com/relf/egobox) (EGO with Kriging). Adjoint
gradients (SU2) cost one extra solve per objective regardless of
variable count, so they win with hundreds of variables and expensive
solves; a [2025 study](https://arxiv.org/pdf/2505.09088) found
derivative-free methods competitive up to moderate dimension. The
in-house estimators are cheap enough for finite differences, so
L-BFGS-B is also an option.

Typical objectives: maximise `L/D` at a design `C_L` (trim by solving
for `alpha`); minimise `C_Di` at fixed `C_L` and span (the answer is
elliptic loading via twist and taper); minimise total drag with the
build-up in the loop; minimise a structural proxy such as root bending
moment `M = int y L'(y) dy`. Constraints: minimum `t/c` (spar depth),
`C_m` bounds, stall margin from section `c_l,max` (XFOIL), fixed span
or wetted area, minimum tip chord. A drag against bending-moment Pareto
front gives the demo something to plot.

nTop's [aircraft pages](https://www.ntop.com/industries/aerospace-and-defense/conceptual-aircraft-design/)
promise "near-realtime performance feedback" with parametric changes that
"automatically update the mesh and boundary conditions" for its built-in
FE; its [wing video](https://resources.ntop.com/resources/videos/designing-a-parametric-wing-with-field-driven-airfoil-and-internal-structure/)
has camber and thickness as spanwise fields with ribs and spars
regenerating. Their loop is FE, not CFD; the same structure works here
with the estimators above as the inner solver.

**The loop, step by step.**

1. Parameter vector `p`: CST coefficients per station (or NACA digits),
   `b, c_r, lambda, Lambda, twist`, incidence, tail volume; bounds and
   linear constraints on `p`.
2. Geometry: build stations, compute `S, AR, MAC`, build the SDF (for
   display and export only; the estimators use the stations).
3. Sections: `alpha_L0`, lift slope, `c_d(c_l)` per station from
   thin-airfoil theory and the drag build-up, or a cached XFOIL or
   NeuralFoil polar when seconds per step are allowed.
4. 3D: lifting-line or VLM, trimmed to the design `C_L` by one Newton
   step on `alpha` (linear, so exact), giving `C_Di`, `e`, `L'(y)`, `C_m`.
5. Objective and constraints from 3 and 4; log every evaluation.
6. Optimiser proposes new `p`; repeat to tolerance, showing the loading
   and `L/D` change live, since each iteration costs milliseconds.
7. Validate the optimum: run XFOIL polars per station at the design
   Reynolds number (checks section drag and `c_l,max`), write the `.avl`
   file and compare `C_L`, `C_Di`, `C_m` (checks the VLM), and for the
   final figure export an STL to OpenFOAM or a Gmsh mesh to SU2 for one
   RANS run. If the section polars differ, swap step 3 for the XFOIL
   data and rerun: a two-fidelity loop.

Known checks: an `AR = 8`, `lambda = 0.4`, untwisted wing gives `e`
near 0.98 from lifting-line; NACA 2412 `alpha_L0` is about -2.0 deg
from thin-airfoil theory and -2.1 deg from XFOIL at `Re = 3e6`; with
twist free at fixed span and `C_L` the optimiser must reach elliptic
loading, `delta -> 0`.

## 6. The rest of the aircraft

* **Fuselage** as a body of revolution: distance to the axis minus
  `r(x)`, corrected for slope. The Sears-Haack law
  `r(x) = R_max (4 x (1 - x))^(3/4)` ([Wikipedia](https://en.wikipedia.org/wiki/Sears%E2%80%93Haack_body))
  is a good default; a lofted fuselage interpolates superelliptic cross
  sections like wing stations. Drag build-up needs only `l/d` and
  wetted area.
* **Tail surfaces**: the same wing type with a symmetric section, sized
  by tail volume coefficients `V_H = S_H l_H / (S MAC)` (0.5 to 0.8) and
  `V_V = S_V l_V / (S b)` (0.02 to 0.05); static margin from `dC_m/dC_L`
  becomes one more loop output.
* **Wing-body junction**: the implicit smooth union
  (`h = max(k - |a - b|, 0)/k`, `min(a, b) - h^2 k/4`) gives a fillet
  that never fails whatever the incidence or dihedral, and `k` can be a
  field: larger at the trailing edge where the junction separates first.
  The same operator adds nacelles, a canopy and winglet blends, exactly
  where a B-rep loft would fail.
* **Internal structure** (nTop's showpiece): spars as boxes intersected
  with the wing SDF, ribs as slabs at stations, a shell for skin; the
  root bending moment drives the spar depth.

Everything exports through `surface_nets` as one closed STL, which is
what snappyHexMesh wants.

## Sources not linked inline

* Anderson, *Fundamentals of Aerodynamics*, ch. 4 and 5.
* Katz and Plotkin, *Low-Speed Aerodynamics*, 2nd ed., ch. 10 and 12.
* Raymer, *Aircraft Design: A Conceptual Approach*, ch. 4 and 12.
* AVL primer: https://web.mit.edu/drela/Public/web/avl/AVL_User_Primer.pdf
* SU2 3D wing tutorial: https://su2code.github.io/tutorials/Inviscid_3D_Constrained_ONERAM6/
* NeuralFoil paper: https://arxiv.org/pdf/2503.16323 ; AeroSandbox: https://peterdsharpe.github.io/AeroSandbox/
* nTop simulation page: https://www.ntop.com/software/capabilities/simulation/
