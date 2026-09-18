# Mathematics of implicit solid modelling for anvil-implicit

Written 2026-09-17 as background for the `anvil-implicit` crate.
Standard results carry numbered sources (list at the end); Anvil
recommendations are marked `REC`. `p` is the query point, `d` a signed
distance (negative inside), `|v|` a vector length.

## 1. Signed distance functions

An SDF `f` satisfies `|f(p)| = dist(p, surface)` and so `|grad f| = 1`
almost everywhere. Many useful functions are only *bounds*:
`|f(p)| <= dist(p, surface)` with the correct sign. Bounds are safe for
sphere tracing and interval pruning but give wrong offsets, shells, and
wall thicknesses where they under-estimate [1][2].

### Exact primitives (Quilez [1])

```
sphere    d = |p| - r
box       q = |p| - b (componentwise);  d = |max(q,0)| + min(max(q.x,q.y,q.z), 0)
round box q = |p| - b + r;  d = |max(q,0)| + min(max(q.x,q.y,q.z), 0) - r
capsule   pa = p - a, ba = b - a, h = clamp(dot(pa,ba)/dot(ba,ba), 0, 1)
          d = |pa - ba*h| - r
cylinder  w = (|p.xz|, p.y) - (r, h);  d = min(max(w.x,w.y),0) + |max(w,0)|
torus     q = (|p.xz| - R, p.y);  d = |q| - r
cone      exact form: distance to the 2D triangle (0,0),(r,-h) in (|p.xz|, p.y)
          with sign from the side test; see sdCone in [1]
```

The exact cone is the 2D polygon distance of the generating triangle in
the meridian plane; any solid of revolution is exact if its generator is.

**Ellipsoid**: no closed-form exact distance. The usual bound is
`k0 = |p/r|, k1 = |p/(r*r)|, d = k0*(k0-1)/k1` [1], which is
`f/|grad f|` for `f = |p/r| - 1` (a first-order Taubin distance),
accurate near the surface and a lower bound elsewhere.

**2D polygon** (exact, [3]): minimum squared distance over the edges,
with a sign flag flipped each time the horizontal ray from `p` crosses
an edge. `O(N)` per query; use a BVH over edges for large polygons.

**Extrusion and revolution of a 2D SDF** [1]:

```
extrude   d2 = sdf2(p.xy);  w = (d2, |p.z| - h)
          d = min(max(w.x,w.y),0) + |max(w,0)|        (exact if sdf2 is exact)
revolve   q = (|p.xz| - o, p.y);  d = sdf2(q)          (exact if sdf2 is exact)
```

### Which operations keep the distance property

| Operation | Formula | Status |
| --- | --- | --- |
| union | `min(a,b)` | exact outside, bound inside near the overlap [1][2] |
| intersection | `max(a,b)` | bound (wrong outside the concave corner) [1] |
| subtraction | `max(a,-b)` | bound [1] |
| offset | `a - r` | exact outward; inward exact only where the medial axis is farther than `r` [1] |
| shell (onion) | `abs(a) - t/2` | exact if `a` exact [1] |
| uniform scale | `a(p/s)*s` | exact [1] |
| non-uniform scale | `a(p/s)` | not a distance; divide by `max(s)` for a bound [1] |
| rigid transform | `a(R^T (p - t))` | exact |
| smooth minimum | section 2 | bound, `|grad| <= 1` [4] |
| warp `a(m(p))` (twist, bend) | | bound after dividing by the Lipschitz constant of `m` [1] |
| interpolation `(1-t)a + t b` | | bound after dividing by `1 + |a-b|*|grad t|` (section 3) |

For a domain warp `q = m(p)`, `grad_p [a(m(p))] = J_m^T grad_q a`, so
`|grad| <= ||J_m||_2`. Twisting by angle `k*y` has `||J|| <= sqrt(1 + (k*rho)^2)`
at radius `rho` from the axis, unbounded as `rho` grows. Quilez notes the
factor can be computed from the Jacobian but it is "usually just easier"
to reduce the step size by hand [1]. `REC`: expose a `lipschitz()` hint on
warp nodes (default 1, analytic where known) and divide by it before
meshing or sphere tracing.

## 2. Smooth blends, fillets, chamfers

Quilez's normalised family, where `k` is the blend width in distance
units and equals the maximum inflation of the union [4]:

```
h = max(k - |a - b|, 0) / k
quadratic   smin = min(a,b) - h*h*k/4
cubic       smin = min(a,b) - h*h*h*k/6
quartic     smin = min(a,b) - h^3*(4 - h)*k/16
circular    smin = min(a,b) - k*0.5*(1 + h - sqrt(1 - h*(h - 2)))
exponential smin = -k * log2(2^(-a/k) + 2^(-b/k))
```

Anvil's current `SmoothUnion` (`h = clamp(0.5 + 0.5(b-a)/k, 0, 1);
mix(b,a,h) - k*h*(1-h)`) is algebraically the quadratic form with the
same `k`, so `k` already means blend width. All of these are bounds:
`|grad smin|^2 <= 1` and the shortfall is non-local [4]. Smooth
intersection and subtraction follow by sign flips: `smax(a,b) = -smin(-a,-b)`.

**Rounded (circular) union**, a true constant-radius fillet where the
inputs are exact and their normals are near perpendicular (hg_sdf [5]):

```
fOpUnionRound(a,b,r):        u = max((r - a, r - b), 0);  d = max(r, min(a,b)) - |u|
fOpIntersectionRound(a,b,r): u = max((r + a, r + b), 0);  d = min(-r, max(a,b)) + |u|
```

This inserts a circle of radius `r` tangent to both surfaces in the
`(a,b)` plane and acts only where both `a < r` and `b < r`, unlike the
polynomial `smin`, which perturbs everything within `k` of either surface.

**Chamfer** (hg_sdf [5]): `fOpUnionChamfer(a,b,r) = min(min(a,b), (a - r + b)*sqrt(0.5))`,
a 45 degree cut at distance `r`; `fOpIntersectionChamfer(a,b,r) = max(max(a,b), (a + r + b)*sqrt(0.5))`.

`REC`: keep the polynomial `smin` for organic blends, add the rounded
union as the "fillet" operator because its `r` is an actual radius, and
add chamfer. Blends see only `a` and `b`, so a fillet between two faces
of the *same* body is not expressible this way; that stays in the B-rep
kernel.

## 3. Sweeps and lofts as implicit functions

nTop lofts by interpolating two implicit sections with a parameter
mapped along an axis, `mix(R, W, t) = (1-t)*R + t*W`, plus an optional
bulge `- a*sin(pi*t)` that vanishes at both ends [6]. This blends
*functions*, so it is not a distance:

```
f = (1 - t(p)) A(p) + t(p) B(p)
grad f = (1-t) grad A + t grad B + (B - A) grad t
|grad f| <= 1 + |B - A| * |grad t|
```

Divide by that constant for a conservative bound; where `|B - A|` is
small near the surface the field stays close to a distance.

**A wing with varying chord, twist and sweep** (assembled from [1] and
the rule above):

```
s      = clamp(p.y / L, 0, 1)                 span parameter
c(s), theta(s), x_le(s), z_le(s)              chord, twist, leading edge (sweep, dihedral)
q      = R(-theta(s)) * (p.x - x_le(s), p.z - z_le(s))    into the section frame
d2     = c(s) * airfoil_unit(q / c(s))         2D section SDF scaled to chord
w      = (d2, |p.y - L/2| - L/2)
d      = min(max(w.x, w.y), 0) + |max(w, 0)|   extrusion combination
```

`airfoil_unit` is the exact polygon SDF of the section at chord 1, or an
interpolation between two section polygons by `s`. The scale
`c(s) * f(q/c(s))` keeps `d2` a distance in each plane; across planes the
Lipschitz factor is about
`1 + |q|*(|c'(s)|/c + |theta'(s)|) + |x_le'(s)| + |z_le'(s)|` per unit
span, small for real planforms.

**Closed tips.** Three standard options:

1. Flat tip: the slab term cuts the sweep at `y = 0` and `y = L`. Exact,
   and what most CAD lofts do.
2. Rounded tip: smooth-intersect (section 2) the sweep with the slab, or
   union a capsule or ellipsoid sized to the tip section.
3. Planform tip: intersect the sweep with the *planform* extruded in `z`,
   a 2D polygon with a rounded or elliptical tip. This keeps the section
   shape as the chord shrinks and avoids the degenerate `c(s) -> 0` scale.

`REC`: implement `Loft` as a `Field` over a `Sections` trait (a 2D SDF
plus per-station parameters interpolated cubically along the path), with
option 3 as the default tip. Never let `c(s)` reach zero inside the sweep.

## 4. Periodic structures

### TPMS level-set functions

With `x = 2*pi*X/L` for cell size `L` (nodal approximations [7][8][9]):

```
Schwarz P       cos x + cos y + cos z
Schwarz D       sin x sin y sin z + sin x cos y cos z + cos x sin y cos z + cos x cos y sin z
Gyroid          sin x cos y + sin y cos z + sin z cos x
Neovius         3 (cos x + cos y + cos z) + 4 cos x cos y cos z
IWP             2 (cos x cos y + cos y cos z + cos z cos x) - (cos 2x + cos 2y + cos 2z)
Lidinoid        0.5 (sin 2x cos y sin z + sin 2y cos z sin x + sin 2z cos x sin y)
                - 0.5 (cos 2x cos 2y + cos 2y cos 2z + cos 2z cos 2x) + 0.15
Fischer-Koch S  cos 2x sin y cos z + cos 2y sin z cos x + cos 2z sin x cos y
```

These are Fourier truncations, exact only near `g = 0`; the Gyroid's
mean curvature is not constant away from the zero level [9]. Some
sources write Neovius with `cos 2x`, which halves the period; pick one
convention.

**Sheet versus network.** Network (solid) form: `g(p) - c <= 0` fills
one labyrinth, `c` setting the volume fraction. Sheet form:
`|g(p)| - c <= 0`, equivalently `max(g - c, -g - c)`. Volume fraction
versus `c` is nonlinear and tabulated per family [7]; too large `|c|`
disconnects the surface.

### Approximating a true distance

`g` is not a distance: `|grad g|` varies over the cell (Gyroid: roughly 1
to 1.5 in scaled coordinates, zero at critical points). The first-order
correction is the Taubin distance [10]:

```
d(p) ~= g(p) / |grad g(p)|      (then divide by 2*pi/L for real units)
```

nTop exposes this as the "normalized field", a first-order approximation
of the implicit function [11]. Anvil currently divides by a constant
(1.2 for Gyroid, 0.9 for Diamond), which is why wall thickness drifts
across the cell. `REC`: compute the analytic gradient (the `sin_cos`
values are already at hand) and use `g / sqrt(|grad g|^2 + eps^2)`,
`eps` about 0.05, so critical points do not blow up. The wall
`abs(d) - wall/2` then has a thickness error of second order in `wall/L`.

### Beam lattices

A beam lattice is the distance to a segment set,
`d = min_i dist(p, seg_i) - r`, exact for the infinite lattice: wrap `p`
into the cell and test its segments plus neighbours within `r + L/2`, as
`BeamLattice` does. A graded radius `r(p)` gives a bound with factor
`1 + |grad r|`.

### Conformal lattices by domain mapping

Evaluate the periodic function in mapped coordinates `u = m(p)`: a
cylindrical or spherical map, a face UV map plus offset along the normal,
or a map between two surfaces; nTop's cell maps do this [12]. Real-space
thickness becomes `t_real ~= t_lattice / ||J_m||` along the thickness
direction, so a `Warp` distorts walls by the Jacobian [13]. Hong,
Antolin, Elber and Kim compute the offset in Euclidean space and correct
by the gradient magnitude so graded walls stay "fully controlled" across
tiles [14]. `REC`: apply the normalisation above to the composed
`g(m(p))` with the chain rule `grad = J_m^T grad g`; that one change fixes
both the TPMS gradient and the map distortion.

### Variable cell size

The naive `g(2*pi*p/L(p))` is wrong: the local frequency is
`d/dx [2*pi*x/L(x)] = 2*pi/L - 2*pi*x*L'/L^2`, so cells drift and alias
far from the origin. Three standard ways to grade without breaking
periodicity:

1. **Phase integral** (grading along one axis, exact): use
   `phi(x) = integral_0^x 2*pi/L(s) ds` as the argument, so the local
   period is `L(x)` everywhere; one integral per axis when `L` depends on
   that axis only.
2. **Blend between scales**: mix `g(2*pi*p/L_1)` and `g(2*pi*p/L_2)` by a
   smooth weight `w(p)` in a transition zone [15]. Both sides stay
   exactly periodic; choose `L_2 = L_1/2` so the lattices are
   commensurate and the transition reads as a subdivision.
3. **Map from a de-homogenisation field**: solve for a coordinate map
   whose Jacobian has the target cell size with minimum distortion, then
   evaluate `g(m(p))` [16]. Needs a PDE solve.

`REC`: implement 1 (`Ramp` in cell size along an axis) and 2 (`Blend`
between two `Lattice` fields by any scalar field) now; 3 later.

## 5. Discretisation

**Dense grids** cost `O((D/h)^3)`; a 200 mm part at 0.2 mm is 10^9
voxels. **Sparse grids** (OpenVDB [17]) use a 4-level tree (root,
internal 32^3, internal 16^3, leaf 8^3) with tile values for uniform
regions and a narrow band of active voxels "normally three voxels wide on
either side of the surface". **Octrees with interval arithmetic**
(libfive, Fidget [18][19]) evaluate `f` over each cell's interval box,
cull it if the result excludes zero, else subdivide. Keeter's tape
pruning records which `min`/`max` branch the interval decided and drops
the other for that subtree, cutting expression complexity "by two orders
of magnitude" on complex models [19]. `fidget_mesh::Settings { depth,
bounds, threads }` builds such an octree and runs manifold dual
contouring [20].

**Gradients.** Central differences need 6 evaluations; Quilez's
tetrahedron trick needs 4 [21]: `n = normalize(sum_i k_i * f(p + h*k_i))`
over `k = (1,-1,-1), (-1,-1,1), (-1,1,-1), (1,1,1)`. Forward-mode
automatic differentiation (dual numbers) gives the exact gradient in one
pass, as Fidget's `grad` evaluator does [18]. `REC`: add
`fn grad(&self, p) -> DVec3` to `Field` (finite-difference default,
analytic overrides for primitives, TPMS, warps) and
`fn range(&self, lo, hi) -> (f64, f64)` defaulting to `(-inf, inf)` so an
octree can prune wherever nodes implement it.

## 6. Meshing

**Marching cubes** (Lorensen and Cline 1987 [22]): vertices at edge
crossings, triangles from a 256-case table. The original table has
ambiguous faces that can leave holes; use the asymptotic decider
(Nielson and Hamann 1991) or Lewiner's 33-case table. No sharp edges,
many slivers.

**Surface nets** (Gibson 1998 [23]; Lysenko's naive version [24]): one
vertex per cell at the mean of its edge crossings, one quad per crossing
edge joining the four adjacent cells. About 4x fewer primitives than
marching cubes, trivial to code, closed by construction, but can produce
non-manifold vertices and bevels sharp edges [24]. This is what Anvil has.

**Dual contouring** (Ju, Losasso, Schaefer, Warren 2002 [25]): surface
net connectivity, but each vertex minimises a quadratic error function
over the Hermite data (crossing points `p_i`, normals `n_i`):

```
E(x) = sum_i ( n_i . (x - p_i) )^2
```

solved by SVD with pseudo-inverse truncation and a pull toward the mass
point so planar cells stay stable. Vertices land on edges and corners,
reproducing sharp features. On an octree, polygons are emitted only
across *minimal edges* (not subdivided by any neighbour), which makes
adaptive DC crack-free without patching [25]. Plain DC can still create
non-manifold edges and self-intersections.

**Manifold dual contouring** (Schaefer, Ju, Warren 2007 [26]) adds a
vertex-clustering rule that guarantees manifold output even after
simplification. Fidget and libfive implement it and state that meshes
are manifold, watertight and sharp, but "may contain self-intersections,
and are not guaranteed to catch thin features (below the sampling grid
resolution)" [20]. Keeter's 2026 note proposes a Delaunay
tetrahedralisation extraction to remove both failures; unimplemented [27].

**Post-processing.** Taubin `lambda | mu` smoothing shrinks less than
plain Laplacian smoothing; project vertices back to `f = 0` after each
pass. Decimate by quadric error metrics (Garland and Heckbert 1997),
the same QEF machinery as DC. `REC` for F4: keep uniform-grid surface
nets, add QEF vertex placement (this alone recovers sharp edges on CSG
of primitives), then an octree with interval pruning and the manifold
clustering rule; Fidget as a back end (F5) covers the expression-tree
subset of fields.

## 7. Mesh to SDF

Unsigned distance: closest point on the triangle set via a BVH,
`O(log N)` per query. Sign:

* **Angle-weighted pseudonormal** (Baerentzen and Aanaes 2005 [28]): at
  the closest feature (face, edge, or vertex) use the angle-weighted
  average of incident face normals; the sign of `(p - c) . n_pseudo` is
  correct whichever feature is closest. Needs a closed, consistently
  oriented mesh.
* **Generalised winding number** (Jacobson et al. 2013; fast tree version
  Barill et al. 2018 [29]): `w(p) = sum of triangle solid angles / (4*pi)`,
  with a dipole expansion per BVH node far away. Robust to holes,
  self-intersections and flipped faces; inside where `w > 0.5`.
* **Ray parity** (Anvil's `Sampled`): cheap, but fails on non-watertight
  input and on rays through edges.

Distance transforms on a binary voxelisation (Meijster 2000,
Felzenszwalb and Huttenlocher 2012) are `O(N)` and separable but measure
to the *voxel* boundary: up to one voxel of error and a staircased surface.

**Redistancing.** Given exact distances in a narrow band, extend them by
solving `|grad phi| = 1` outward: fast marching (Sethian 1996 [30]) is a
Dijkstra-like upwind scheme, `O(N log N)`; fast sweeping (Zhao 2004
[31]) is Gauss-Seidel with alternating sweep directions, `O(N)` and
parallel (8 sweeps in 3D). OpenVDB's `meshToLevelSet` rasterises exact
triangle distances into the band, then signed-flood-fills the tiles
[17][32]. `REC`: use the fast winding number for sign, keep the exact
narrow-band triangle distance, and fill the rest by fast sweeping rather
than a full-grid distance transform.

## 8. Practical numerics

**Voxel size versus feature size.** A sampled level set resolves
features larger than about `2h` (the grid's Nyquist limit). Surface nets
and marching cubes need a sign change on an edge to see a wall, so a
wall thinner than `h` vanishes or breaks into islands depending on its
phase against the grid. Rule of thumb: `h <= wall/3` for TPMS sheets and
`h <= r_beam` for struts; a 1 mm wall at `h = 0.3 mm` is fine for a
0.4 mm nozzle, below that use an octree. Interval pruning does not help:
it decides where the surface *might* be, not whether the grid resolves
it [20].

**Lipschitz bounds.** Sphere tracing (Hart 1996 [33]) steps
`t <- t + f(p(t)) / lambda`, `lambda` a Lipschitz bound of `f` (1 for an
exact SDF), and never crosses the surface. The same bound prunes cells:
a cell of half-diagonal `rho` centred at `c` is empty if
`|f(c)| > lambda * rho`. Interval arithmetic is tighter for fields with
no useful `lambda` (graded TPMS, strong twist), which is why libfive and
Fidget use it. Keep a per-node `lambda`: 1 for primitives, rigid moves,
`min`/`max` and polynomial `smin`; `max(s)` for scale; `||J||` for warps;
`1 + |A-B|*|grad t|` for interpolations.

**Thin walls and aliasing.** Evaluate sheets as `abs(d) - wall/2` on a
*normalised* field (section 4); never evaluate `wall` in lattice
coordinates on a warped field; and when the wall approaches `h`, refine
locally (octree, driven by `|f| < 2h` and `|grad f|` far from 1) or clamp
the wall to `2h` and report it, rather than let the mesher drop it
silently.

## Sources

1. Quilez, "Distance functions" https://iquilezles.org/articles/distfunctions/
2. Quilez, "SDF bounding volumes" https://iquilezles.org/articles/sdfbounding/
3. Quilez, "2D distance functions" https://iquilezles.org/articles/distfunctions2d/
4. Quilez, "Smooth minimum" https://iquilezles.org/articles/smin/
5. Mercury, hg_sdf library (fOpUnionRound, fOpUnionChamfer) https://mercury.sexy/hg_sdf/
6. nTop, "Interpolating with implicit modeling" https://resources.ntop.com/resources/blog/interpolating-with-implicit-modeling/
7. Wohlgemuth, Yufa, Hoffman, Thomas, "Triply periodic bicontinuous cubic microdomain morphologies by symmetries", Macromolecules 34 (2001) 6083, https://pubs.acs.org/doi/10.1021/ma0019499
8. Al-Ketan, Abu Al-Rub, "Multifunctional mechanical metamaterials based on TPMS lattices", Adv. Eng. Mater. 21 (2019) 1900524, https://advanced.onlinelibrary.wiley.com/doi/abs/10.1002/adem.201900524
9. Wikipedia, "Gyroid" https://en.wikipedia.org/wiki/Gyroid ; nodal-approximation accuracy discussed in https://utw10945.utweb.utexas.edu/sites/default/files/2022/Using%20Mean%20Curvature%20of%20Implicitly%20Defined%20Minimal.pdf
10. Taubin, "Distance approximations for rasterizing implicit curves", ACM TOG 13 (1994) 3
11. nTop support, "Why isn't the TPMS/Lattice thickness constant when I distort it?" https://support.ntop.com/hc/en-us/articles/360061475314
12. nTop support, "Guide to Conformal Latticing" https://support.ntop.com/hc/en-us/articles/29651196870035-Guide-to-Conformal-Latticing
13. nTop support, "How to create a periodic lattice" https://support.ntop.com/hc/en-us/articles/360050828294-How-to-create-a-periodic-lattice
14. Hong, Antolin, Elber, Kim, "Variable offsets and processing of implicit forms toward the adaptive synthesis and analysis of heterogeneous conforming microstructure", https://arxiv.org/abs/2408.14068
15. Functionally graded TPMS with programmable pore size (period varied continuously), https://pmc.ncbi.nlm.nih.gov/articles/PMC7664887/
16. "Distortion-minimized de-homogenization for optimization of cell-size distribution in TPMS structures", https://arxiv.org/pdf/2605.06011
17. OpenVDB overview (tree, tiles, narrow band) https://www.openvdb.org/documentation/doxygen/overview.html
18. Fidget https://github.com/mkeeter/fidget
19. Keeter, "Massively Parallel Rendering of Complex Closed-Form Implicit Surfaces", ACM TOG 39 (2020) 4, https://www.mattkeeter.com/research/mpr/
20. fidget_mesh docs https://docs.rs/fidget-mesh/latest/fidget_mesh/
21. Quilez, "Normals for an SDF" https://iquilezles.org/articles/normalsSDF/
22. Lorensen, Cline, "Marching Cubes", SIGGRAPH 1987, https://dl.acm.org/doi/10.1145/37401.37422
23. Gibson, "Constrained elastic surface nets", MICCAI 1998, https://link.springer.com/chapter/10.1007/BFb0056277
24. Lysenko, "Smooth voxel terrain, part 2" https://0fps.net/2012/07/12/smooth-voxel-terrain-part-2/
25. Ju, Losasso, Schaefer, Warren, "Dual contouring of Hermite data", SIGGRAPH 2002, https://www.cs.rice.edu/~jwarren/papers/dualcontour.pdf
26. Schaefer, Ju, Warren, "Manifold dual contouring", IEEE TVCG 13 (2007) 610, https://www.cs.wustl.edu/~taoju/research/dualsimp_tvcg.pdf
27. Keeter, "Please steal my meshing algorithm idea" (2026) https://www.mattkeeter.com/blog/2026-07-03-meshing/
28. Baerentzen, Aanaes, "Signed distance computation using the angle weighted pseudonormal", IEEE TVCG 11 (2005) 243, https://backend.orbit.dtu.dk/ws/files/3977815/B%C3%A6rentzen.pdf
29. Barill, Dickson, Schmidt, Levin, Jacobson, "Fast winding numbers for soups and clouds", SIGGRAPH 2018, https://www.dgp.toronto.edu/projects/fast-winding-numbers/
30. Sethian, "A fast marching level set method for monotonically advancing fronts", PNAS 93 (1996) 1591, https://www.pnas.org/doi/10.1073/pnas.93.4.1591
31. Zhao, "A fast sweeping method for Eikonal equations", Math. Comp. 74 (2005) 603, https://www.math.uci.edu/~zhao/homepage/research_files/FSM.pdf
32. OpenVDB MeshToVolume.h https://academysoftwarefoundation.github.io/openvdb/MeshToVolume_8h_source.html
33. Hart, "Sphere tracing", The Visual Computer 12 (1996) 527, http://graphics.stanford.edu/courses/cs348b-18-spring-content/uploads/hart.pdf
