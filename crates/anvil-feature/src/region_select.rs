//! Choosing which closed regions of a sketch a feature uses.
//!
//! The regions are the bounded faces of the planar arrangement of the
//! sketch's closed profiles: every crossing, touch, or shared stretch of
//! two profiles splits the curves there, so two circles that overlap like a
//! Venn diagram give three regions (left only, the lens, right only). Each
//! region is one outer loop plus the loops nested directly inside it as
//! holes, and every point inside the profiles lies in exactly one region.
//!
//! The default regions keep the even nesting rule: a region is used by
//! default when its inside lies within a profile at even nesting depth and
//! outside that profile's direct holes. A plate with holes stays a plate
//! with holes; an island inside a hole, or the inside of a hole, is only
//! used when picked. Features store the choice as text: empty means the
//! default regions, otherwise region indices separated by commas, for
//! example "0,2". Picked regions that share an edge become one region for
//! the feature, so the lens and one side of a Venn diagram extrude as one
//! body.

use crate::features::emboss::{point_in_poly, polygon_area};
use crate::RegenError;
use anvil_math::DVec2;
use std::collections::{HashMap, HashSet};

/// Outer loop and the loops directly inside it, in sketch plane
/// coordinates.
#[derive(Clone, Debug)]
pub struct Region {
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    /// Used when the feature picks no regions explicitly.
    pub default: bool,
}

/// The regions of a sketch's closed profiles: the default ones first, then
/// the others, each group ordered by an interior point (x, then y) and
/// then by area, so the same sketch always gives the same indices.
pub fn regions(profiles: &[anvil_sketch::Profile]) -> Vec<Region> {
    let arr = Arrangement::build(profiles);
    arr.faces
        .iter()
        .map(|f| Region {
            outer: arr.polygon(&f.cycles[0]),
            holes: f.cycles[1..].iter().map(|c| arr.polygon(c)).collect(),
            default: f.default,
        })
        .collect()
}

/// The even nesting rule of the profiles as drawn: a point is covered when
/// it lies inside a profile at even depth and outside that profile's
/// direct holes.
struct Nesting<'a> {
    loops: &'a [Vec<DVec2>],
    depth: Vec<usize>,
    parent: Vec<Option<usize>>,
}

impl<'a> Nesting<'a> {
    fn new(loops: &'a [Vec<DVec2>]) -> Self {
        let n = loops.len();
        // A loop counts as inside another when most of its sample vertices
        // are, so a hole that touches the outer loop at one vertex still nests.
        let inside = |i: usize, j: usize| -> bool {
            let li = &loops[i];
            let step = (li.len() / 5).max(1);
            let samples: Vec<DVec2> = li.iter().step_by(step).take(5).copied().collect();
            let hits = samples.iter().filter(|&&p| point_in_poly(p, &loops[j])).count();
            hits * 2 > samples.len()
        };
        let depth: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| j != i && inside(i, j)).count()).collect();
        // Parent: the smallest loop one level up that contains the loop.
        let parent: Vec<Option<usize>> = (0..n)
            .map(|i| {
                (0..n)
                    .filter(|&j| j != i && depth[j] + 1 == depth[i] && inside(i, j))
                    .min_by(|&a, &b| polygon_area(&loops[a]).abs().total_cmp(&polygon_area(&loops[b]).abs()))
            })
            .collect();
        Nesting { loops, depth, parent }
    }

    fn covers(&self, p: DVec2) -> bool {
        (0..self.loops.len()).any(|i| {
            self.depth[i].is_multiple_of(2)
                && point_in_poly(p, &self.loops[i])
                && !(0..self.loops.len()).any(|k| self.parent[k] == Some(i) && point_in_poly(p, &self.loops[k]))
        })
    }
}

/// One bounded face of the arrangement.
struct Face {
    /// Half-edge cycles with the face on their left: the outer loop first
    /// (counterclockwise), then the holes (clockwise).
    cycles: Vec<Vec<usize>>,
    area: f64,
    sample: DVec2,
    default: bool,
}

/// Planar arrangement of a sketch's profiles as a half-edge graph. Edge
/// `e` gives half-edges `2e` and `2e + 1`, twins of each other, so the twin
/// of `h` is `h ^ 1`.
struct Arrangement {
    loops: Vec<Vec<DVec2>>,
    pts: Vec<DVec2>,
    /// The profile vertex (loop, index) that created each vertex, if any.
    orig: Vec<Option<(usize, usize)>>,
    eps: f64,
    /// Start vertex of each half-edge; it ends at `from[h ^ 1]`.
    from: Vec<usize>,
    /// Outgoing half-edges of each vertex, counterclockwise by angle.
    out: Vec<Vec<usize>>,
    /// Position of each half-edge in `out[from[h]]`.
    slot: Vec<usize>,
    faces: Vec<Face>,
    /// Face on the left of each half-edge; `None` outside every profile.
    face_of: Vec<Option<usize>>,
}

/// Vertices merged within a tolerance through a hash grid.
struct Snap {
    cell: f64,
    grid: HashMap<(i64, i64), Vec<usize>>,
    pts: Vec<DVec2>,
    orig: Vec<Option<(usize, usize)>>,
}

impl Snap {
    fn id(&mut self, p: DVec2, orig: Option<(usize, usize)>) -> usize {
        let (cx, cy) = ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.grid.get(&(cx + dx, cy + dy)) {
                    if let Some(&i) = v.iter().find(|&&i| self.pts[i].distance(p) <= self.cell) {
                        return i;
                    }
                }
            }
        }
        let i = self.pts.len();
        self.pts.push(p);
        self.orig.push(orig);
        self.grid.entry((cx, cy)).or_default().push(i);
        i
    }
}

fn find(uf: &mut [usize], mut i: usize) -> usize {
    while uf[i] != i {
        uf[i] = uf[uf[i]];
        i = uf[i];
    }
    i
}

fn union(uf: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(uf, a), find(uf, b));
    if ra != rb {
        uf[ra.max(rb)] = ra.min(rb);
    }
}

/// Parameter of `q` along segment `a`-`b` when `q` lies on the segment
/// within `eps`, away from both ends.
fn on_segment(q: DVec2, a: DVec2, b: DVec2, eps: f64) -> Option<f64> {
    let ab = b - a;
    let t = (q - a).dot(ab) / ab.length_squared();
    if t <= 0.0 || t >= 1.0 || (a + ab * t).distance(q) > eps || q.distance(a) <= eps || q.distance(b) <= eps {
        return None;
    }
    Some(t)
}

/// Where segments `a`-`b` and `c`-`d` touch or cross: split points for
/// each of the two.
fn touch(a: DVec2, b: DVec2, c: DVec2, d: DVec2, eps: f64, si: &mut Vec<(f64, DVec2)>, sj: &mut Vec<(f64, DVec2)>) {
    // An end of one segment on the other: a T junction or a shared stretch.
    for q in [c, d] {
        if let Some(t) = on_segment(q, a, b, eps) {
            si.push((t, q));
        }
    }
    for q in [a, b] {
        if let Some(t) = on_segment(q, c, d, eps) {
            sj.push((t, q));
        }
    }
    // A proper crossing, with every end clearly off the other line.
    let (ab, cd) = (b - a, d - c);
    let (la, lc) = (ab.length(), cd.length());
    let o1 = ab.perp_dot(c - a) / la;
    let o2 = ab.perp_dot(d - a) / la;
    let o3 = cd.perp_dot(a - c) / lc;
    let o4 = cd.perp_dot(b - c) / lc;
    let apart = |x: f64, y: f64| (x > eps && y < -eps) || (x < -eps && y > eps);
    if apart(o1, o2) && apart(o3, o4) {
        let s = o3 / (o3 - o4);
        let t = o1 / (o1 - o2);
        let p = a + ab * s;
        si.push((s, p));
        sj.push((t, p));
    }
}

/// Twice the signed area of a closed polyline.
fn area2(p: &[DVec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum()
}

/// A point strictly inside the area bounded by `loops` under the even-odd
/// rule: the middle of the widest span on the horizontal line through the
/// widest gap between vertex heights.
fn interior_point(loops: &[Vec<DVec2>]) -> DVec2 {
    let mut ys: Vec<f64> = loops.iter().flatten().map(|p| p.y).collect();
    ys.sort_by(f64::total_cmp);
    let fallback = || {
        let o = &loops[0];
        o.iter().copied().sum::<DVec2>() / o.len().max(1) as f64
    };
    let Some(w) = ys.windows(2).max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0]))) else { return fallback() };
    if w[1] <= w[0] {
        return fallback();
    }
    let y0 = 0.5 * (w[0] + w[1]);
    let mut xs = Vec::new();
    for l in loops {
        for k in 0..l.len() {
            let (a, b) = (l[k], l[(k + 1) % l.len()]);
            if (a.y > y0) != (b.y > y0) {
                xs.push(a.x + (y0 - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>()
        .0
        .iter()
        .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        .map_or_else(fallback, |s| DVec2::new(0.5 * (s[0] + s[1]), y0))
}

impl Arrangement {
    fn build(profiles: &[anvil_sketch::Profile]) -> Self {
        let loops: Vec<Vec<DVec2>> =
            profiles.iter().filter(|p| p.points.len() >= 3).map(|p| p.points.clone()).collect();
        let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
        for p in loops.iter().flatten() {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let diag = if loops.is_empty() { 0.0 } else { (hi - lo).length() };
        // Points closer than this are one point. It follows the sketch size
        // but never drops below the kernel's linear tolerance.
        let eps = anvil_math::LINEAR_TOL.max(diag * 1e-9);
        let mut arr = Arrangement {
            loops: Vec::new(),
            pts: Vec::new(),
            orig: Vec::new(),
            eps,
            from: Vec::new(),
            out: Vec::new(),
            slot: Vec::new(),
            faces: Vec::new(),
            face_of: Vec::new(),
        };
        if !diag.is_finite() || diag <= eps {
            return arr;
        }

        // Vertices: the profile points first, so they keep their origin.
        let mut snap = Snap { cell: eps, grid: HashMap::new(), pts: Vec::new(), orig: Vec::new() };
        let mut segs: Vec<(usize, usize)> = Vec::new();
        for (li, l) in loops.iter().enumerate() {
            let ids: Vec<usize> = l.iter().enumerate().map(|(k, &p)| snap.id(p, Some((li, k)))).collect();
            for k in 0..ids.len() {
                let (u, v) = (ids[k], ids[(k + 1) % ids.len()]);
                if u != v {
                    segs.push((u, v));
                }
            }
        }

        // Every touch and crossing, found by a sweep along x.
        let seg_pts = |s: (usize, usize)| (snap.pts[s.0], snap.pts[s.1]);
        let m = segs.len();
        let bounds: Vec<(DVec2, DVec2)> = segs
            .iter()
            .map(|&s| {
                let (a, b) = seg_pts(s);
                (a.min(b), a.max(b))
            })
            .collect();
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|&i, &j| bounds[i].0.x.total_cmp(&bounds[j].0.x).then(i.cmp(&j)));
        let mut splits: Vec<Vec<(f64, DVec2)>> = vec![Vec::new(); m];
        for (oi, &i) in order.iter().enumerate() {
            let (bi_lo, bi_hi) = bounds[i];
            for &j in &order[oi + 1..] {
                let (bj_lo, bj_hi) = bounds[j];
                if bj_lo.x > bi_hi.x + eps {
                    break;
                }
                if bj_lo.y > bi_hi.y + eps || bi_lo.y > bj_hi.y + eps {
                    continue;
                }
                let ((a, b), (c, d)) = (seg_pts(segs[i]), seg_pts(segs[j]));
                let (mut si, mut sj) = (Vec::new(), Vec::new());
                touch(a, b, c, d, eps, &mut si, &mut sj);
                splits[i].extend(si);
                splits[j].extend(sj);
            }
        }

        // Split segments into edges; a shared stretch gives one edge.
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for (i, &(u0, v0)) in segs.iter().enumerate() {
            let mut sp = std::mem::take(&mut splits[i]);
            sp.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut chain = vec![u0];
            chain.extend(sp.iter().map(|&(_, p)| snap.id(p, None)));
            chain.push(v0);
            for w in chain.windows(2) {
                let (u, v) = (w[0], w[1]);
                if u != v && seen.insert((u.min(v), u.max(v))) {
                    edges.push((u, v));
                }
            }
        }
        let Snap { pts, orig, .. } = snap;
        let nv = pts.len();

        // Drop dangling edges: they bound no area.
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nv];
        for (e, &(u, v)) in edges.iter().enumerate() {
            adj[u].push(e);
            adj[v].push(e);
        }
        let mut deg: Vec<usize> = adj.iter().map(Vec::len).collect();
        let mut alive = vec![true; edges.len()];
        let mut stack: Vec<usize> = (0..nv).filter(|&v| deg[v] == 1).collect();
        while let Some(v) = stack.pop() {
            if deg[v] != 1 {
                continue;
            }
            let Some(&e) = adj[v].iter().find(|&&e| alive[e]) else { continue };
            alive[e] = false;
            let (a, b) = edges[e];
            for w in [a, b] {
                deg[w] -= 1;
                if deg[w] == 1 {
                    stack.push(w);
                }
            }
        }
        let edges: Vec<(usize, usize)> = edges.into_iter().zip(alive).filter(|(_, a)| *a).map(|(e, _)| e).collect();

        // Half-edges, sorted around each vertex.
        let mut from = Vec::with_capacity(edges.len() * 2);
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); nv];
        for (e, &(u, v)) in edges.iter().enumerate() {
            from.push(u);
            from.push(v);
            out[u].push(2 * e);
            out[v].push(2 * e + 1);
        }
        let mut slot = vec![0; from.len()];
        for (v, o) in out.iter_mut().enumerate() {
            let ang = |h: usize| {
                let d = pts[from[h ^ 1]] - pts[v];
                d.y.atan2(d.x)
            };
            o.sort_by(|&a, &b| ang(a).total_cmp(&ang(b)).then(a.cmp(&b)));
            for (k, &h) in o.iter().enumerate() {
                slot[h] = k;
            }
        }
        arr.loops = loops;
        arr.pts = pts;
        arr.orig = orig;
        arr.from = from;
        arr.out = out;
        arr.slot = slot;

        // Walk every cycle with its area on the left.
        let nh = arr.from.len();
        let mut visited = vec![false; nh];
        let mut cycles: Vec<(Vec<usize>, f64)> = Vec::new();
        for h0 in 0..nh {
            if visited[h0] {
                continue;
            }
            let mut cyc = Vec::new();
            let mut h = h0;
            while !visited[h] {
                visited[h] = true;
                cyc.push(h);
                h = arr.next(h, |_| true);
            }
            let a = 0.5 * area2(&arr.cycle_points(&cyc));
            cycles.push((cyc, a));
        }

        // Connected pieces of the drawing.
        let mut uf: Vec<usize> = (0..nv).collect();
        for &(u, v) in &edges {
            union(&mut uf, u, v);
        }
        let area_eps = eps * diag;
        let mut faces: Vec<(Vec<Vec<usize>>, usize)> = Vec::new();
        let mut outlines: Vec<(Vec<usize>, usize)> = Vec::new();
        for (cyc, a) in cycles {
            let comp = find(&mut uf, arr.from[cyc[0]]);
            if a > area_eps {
                faces.push((vec![cyc], comp));
            } else if a < -area_eps {
                outlines.push((cyc, comp));
            }
        }
        // The outline of a piece is a hole of the smallest face of another
        // piece around it; with no face around, it borders the outside.
        let outer_pts: Vec<Vec<DVec2>> = faces.iter().map(|(c, _)| arr.cycle_points(&c[0])).collect();
        let outer_box: Vec<(DVec2, DVec2)> =
            outer_pts.iter().map(|p| p.iter().fold((p[0], p[0]), |(lo, hi), &q| (lo.min(q), hi.max(q)))).collect();
        let outer_area: Vec<f64> = outer_pts.iter().map(|p| 0.5 * area2(p)).collect();
        for (cyc, comp) in outlines {
            let q = arr.pts[arr.from[cyc[0]]];
            let host = (0..faces.len())
                .filter(|&f| faces[f].1 != comp)
                .filter(|&f| {
                    let (lo, hi) = outer_box[f];
                    q.x >= lo.x && q.x <= hi.x && q.y >= lo.y && q.y <= hi.y && point_in_poly(q, &outer_pts[f])
                })
                .min_by(|&a, &b| outer_area[a].total_cmp(&outer_area[b]).then(a.cmp(&b)));
            if let Some(f) = host {
                faces[f].0.push(cyc);
            }
        }

        let nesting = Nesting::new(&arr.loops);
        let mut faces: Vec<Face> = faces
            .into_iter()
            .map(|(cycles, _)| {
                let polys: Vec<Vec<DVec2>> = cycles.iter().map(|c| arr.cycle_points(c)).collect();
                let area = 0.5 * polys.iter().map(|p| area2(p)).sum::<f64>();
                let sample = interior_point(&polys);
                let default = nesting.covers(sample);
                Face { cycles, area, sample, default }
            })
            .collect();
        faces.sort_by(|a, b| {
            b.default
                .cmp(&a.default)
                .then(a.sample.x.total_cmp(&b.sample.x))
                .then(a.sample.y.total_cmp(&b.sample.y))
                .then(a.area.total_cmp(&b.area))
        });
        let mut face_of = vec![None; nh];
        for (f, face) in faces.iter().enumerate() {
            for &h in face.cycles.iter().flatten() {
                face_of[h] = Some(f);
            }
        }
        arr.faces = faces;
        arr.face_of = face_of;
        arr
    }

    /// The half-edge after `h` around the area on its left, skipping
    /// half-edges that `keep` rejects: the first one clockwise from the
    /// way back.
    fn next(&self, h: usize, keep: impl Fn(usize) -> bool) -> usize {
        let o = &self.out[self.from[h ^ 1]];
        let (i, d) = (self.slot[h ^ 1], o.len());
        (1..=d).map(|k| o[(i + d - k) % d]).find(|&c| keep(c)).unwrap_or(h ^ 1)
    }

    fn cycle_points(&self, cyc: &[usize]) -> Vec<DVec2> {
        cyc.iter().map(|&h| self.pts[self.from[h]]).collect()
    }

    /// The points of a cycle. A cycle that runs exactly along one profile
    /// gives that profile's points as drawn; any other drops the points
    /// that lie on a straight run.
    fn polygon(&self, cyc: &[usize]) -> Vec<DVec2> {
        let vs: Vec<usize> = cyc.iter().map(|&h| self.from[h]).collect();
        if let Some((li, _)) = self.orig[vs[0]] {
            let n = self.loops[li].len();
            let idx = |v: usize| self.orig[v].filter(|o| o.0 == li).map(|o| o.1);
            let whole = vs.len() == n
                && (0..n).all(|k| match (idx(vs[k]), idx(vs[(k + 1) % n])) {
                    (Some(a), Some(b)) => (a + 1) % n == b || (b + 1) % n == a,
                    _ => false,
                });
            if whole {
                return self.loops[li].clone();
            }
        }
        let mut p: Vec<DVec2> = vs.iter().map(|&v| self.pts[v]).collect();
        let straight = |a: DVec2, b: DVec2, c: DVec2| {
            let ac = c - a;
            let l = ac.length();
            l > 0.0 && (b - a).perp_dot(ac).abs() / l <= self.eps && (b - a).dot(c - b) > 0.0
        };
        loop {
            let n = p.len();
            if n <= 3 {
                break;
            }
            let Some(k) = (0..n).find(|&k| straight(p[(k + n - 1) % n], p[k], p[(k + 1) % n])) else { break };
            // Remove every straight point in one pass, then look again.
            let keep: Vec<bool> =
                (0..n).map(|j| j != k && !(j > k && straight(p[j - 1], p[j], p[(j + 1) % n]))).collect();
            p = p.into_iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| q).collect();
        }
        p
    }

    /// The picked faces as (outer, holes) pairs. Faces that share an edge
    /// merge into one; faces that do not stay apart.
    fn merged(&self, picked: &[usize]) -> Vec<OuterAndHoles> {
        let nf = self.faces.len();
        let mut on = vec![false; nf];
        for &f in picked {
            on[f] = true;
        }
        let mut uf: Vec<usize> = (0..nf).collect();
        for f in (0..nf).filter(|&f| on[f]) {
            for &h in self.faces[f].cycles.iter().flatten() {
                if let Some(g) = self.face_of[h ^ 1].filter(|&g| on[g]) {
                    union(&mut uf, f, g);
                }
            }
        }
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_of: Vec<Option<usize>> = vec![None; nf];
        let mut root_group: HashMap<usize, usize> = HashMap::new();
        for f in (0..nf).filter(|&f| on[f]) {
            let r = find(&mut uf, f);
            let g = *root_group.entry(r).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[g].push(f);
            group_of[f] = Some(g);
        }
        let group_at = |h: usize| self.face_of[h].and_then(|f| group_of[f]);
        let mut result = Vec::new();
        for (g, faces) in groups.iter().enumerate() {
            let cycles: Vec<Vec<usize>> = if let [f] = faces[..] {
                self.faces[f].cycles.clone()
            } else {
                // The boundary: half-edges of the group whose twin is not.
                let edge = |h: usize| group_at(h) == Some(g) && group_at(h ^ 1) != Some(g);
                let mut done = HashSet::new();
                let mut cycles = Vec::new();
                for &h0 in faces.iter().flat_map(|&f| self.faces[f].cycles.iter().flatten()) {
                    if !edge(h0) || done.contains(&h0) {
                        continue;
                    }
                    let mut cyc = Vec::new();
                    let mut h = h0;
                    while done.insert(h) {
                        cyc.push(h);
                        h = self.next(h, edge);
                    }
                    cycles.push(cyc);
                }
                cycles
            };
            let mut outers: Vec<(f64, Vec<DVec2>)> = Vec::new();
            let mut holes: Vec<Vec<DVec2>> = Vec::new();
            for c in &cycles {
                let a = area2(&self.cycle_points(c));
                if a > 0.0 {
                    outers.push((a, self.polygon(c)));
                } else if a < 0.0 {
                    holes.push(self.polygon(c));
                }
            }
            outers.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut outers = outers.into_iter().map(|(_, p)| p);
            if let Some(o) = outers.next() {
                result.push((o, holes));
            }
            // A group only pinches into more than one outer loop in odd
            // corner cases; those loops still become regions of their own.
            result.extend(outers.map(|o| (o, Vec::new())));
        }
        result
    }
}

/// Region indices named by `spec`, or `None` for the default regions.
pub fn parse(spec: &str) -> Option<Vec<usize>> {
    let t = spec.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("all") {
        return None;
    }
    let mut v: Vec<usize> = t.split([',', ' ']).filter_map(|s| s.trim().parse().ok()).collect();
    v.sort_unstable();
    v.dedup();
    Some(v)
}

pub fn format(sel: &[usize]) -> String {
    sel.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

/// True when region `i` is used under `spec`.
pub fn is_selected(regions: &[Region], spec: &str, i: usize) -> bool {
    match parse(spec) {
        None => regions.get(i).is_some_and(|r| r.default),
        Some(v) => v.contains(&i),
    }
}

/// The spec after a click on region `i`. A click on a used region drops
/// it; a click on an unused one adds it. Ending on the default regions
/// gives back the empty spec.
pub fn toggle(regions: &[Region], spec: &str, i: usize) -> String {
    let n = regions.len();
    let defaults: Vec<usize> = (0..n).filter(|&k| regions[k].default).collect();
    let mut v = parse(spec).unwrap_or_else(|| defaults.clone());
    v.retain(|&k| k < n);
    match v.iter().position(|&k| k == i) {
        Some(p) => {
            v.remove(p);
        }
        None => {
            v.push(i);
            v.sort_unstable();
        }
    }
    if v == defaults {
        String::new()
    } else {
        format(&v)
    }
}

/// An outer profile and its holes, as the kernel takes them.
pub type OuterAndHoles = (Vec<DVec2>, Vec<Vec<DVec2>>);

/// The regions that `spec` picks from a sketch's profiles. Picked regions
/// that share an edge come back merged into one.
pub fn select(profiles: &[anvil_sketch::Profile], spec: &str) -> Result<Vec<OuterAndHoles>, RegenError> {
    let arr = Arrangement::build(profiles);
    let n = arr.faces.len();
    let picked: Vec<usize> = match parse(spec) {
        None => (0..n).filter(|&i| arr.faces[i].default).collect(),
        Some(sel) => {
            if sel.is_empty() {
                return Err(RegenError::Other("no region selected; click a region of the sketch".into()));
            }
            if let Some(bad) = sel.iter().find(|&&i| i >= n) {
                return Err(RegenError::Other(format!(
                    "region {bad} no longer exists (the sketch has {n} regions); pick the regions again"
                )));
            }
            sel
        }
    };
    Ok(arr.merged(&picked))
}

/// The region that contains `p` (inside the outer loop and outside its
/// holes). Regions do not overlap, so there is at most one.
pub fn region_at(regions: &[Region], p: DVec2) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.outer.len() >= 3 && point_in_poly(p, &r.outer) && !r.holes.iter().any(|l| point_in_poly(p, l))
        })
        .min_by(|a, b| polygon_area(&a.1.outer).abs().total_cmp(&polygon_area(&b.1.outer).abs()))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_sketch::Profile;

    fn square(c: DVec2, h: f64) -> Profile {
        Profile {
            points: vec![c + DVec2::new(-h, -h), c + DVec2::new(h, -h), c + DVec2::new(h, h), c + DVec2::new(-h, h)],
        }
    }

    #[test]
    fn toggle_from_default_and_back() {
        let r =
            regions(&[square(DVec2::ZERO, 1.0), square(DVec2::new(5.0, 0.0), 1.0), square(DVec2::new(9.0, 0.0), 1.0)]);
        assert_eq!(toggle(&r, "", 1), "0,2");
        assert_eq!(toggle(&r, "0,2", 1), "");
        assert_eq!(toggle(&r, "0", 0), "");
        assert!(is_selected(&r, "", 2));
        assert!(!is_selected(&r, "0,2", 1));
    }

    #[test]
    fn select_and_pick_nested_regions() {
        // Big square with a hole, an island in the hole, and a separate square.
        let profs = vec![
            square(DVec2::ZERO, 10.0),
            square(DVec2::ZERO, 5.0),
            square(DVec2::ZERO, 2.0),
            square(DVec2::new(30.0, 0.0), 3.0),
        ];
        let r = regions(&profs);
        assert_eq!(r.len(), 4);
        assert_eq!(r.iter().filter(|x| x.default).count(), 3);
        let ring = region_at(&r, DVec2::new(7.0, 0.0)).unwrap();
        let island = region_at(&r, DVec2::ZERO).unwrap();
        let side = region_at(&r, DVec2::new(30.0, 0.0)).unwrap();
        assert_eq!(r[ring].holes.len(), 1);
        assert_ne!(ring, island);
        // Between the hole and the island: the inside of the hole, not a default region.
        let gap = region_at(&r, DVec2::new(3.5, 0.0)).unwrap();
        assert!(!r[gap].default);
        assert_eq!(r[gap].holes.len(), 1);
        let picked = select(&profs, &format(&[side])).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(select(&profs, "7").is_err());
        assert_eq!(select(&profs, " ").unwrap().len(), 3);
    }

    #[test]
    fn circle_inside_a_face_outline_can_be_picked_alone() {
        let profs = vec![square(DVec2::ZERO, 10.0), square(DVec2::new(2.0, 2.0), 1.0)];
        let r = regions(&profs);
        let boss = region_at(&r, DVec2::new(2.0, 2.0)).unwrap();
        let picked = select(&profs, &format(&[boss])).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(picked[0].1.is_empty());
        assert!((polygon_area(&picked[0].0).abs() - 4.0).abs() < 1e-9);
    }

    fn circle(c: DVec2, r: f64, n: usize) -> Profile {
        Profile {
            points: (0..n)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / n as f64;
                    c + DVec2::new(a.cos(), a.sin()) * r
                })
                .collect(),
        }
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Profile {
        Profile { points: vec![DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)] }
    }

    /// Area of an (outer, holes) pair.
    fn area(r: &OuterAndHoles) -> f64 {
        polygon_area(&r.0).abs() - r.1.iter().map(|h| polygon_area(h).abs()).sum::<f64>()
    }

    #[test]
    fn venn_circles_give_three_regions() {
        let n = 256;
        let profs = vec![circle(DVec2::new(-0.5, 0.0), 1.0, n), circle(DVec2::new(0.5, 0.0), 1.0, n)];
        let r = regions(&profs);
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.default));
        let left = region_at(&r, DVec2::new(-1.2, 0.0)).unwrap();
        let lens = region_at(&r, DVec2::ZERO).unwrap();
        let right = region_at(&r, DVec2::new(1.2, 0.0)).unwrap();
        assert!(left != lens && lens != right && left != right);
        // Every point lies in at most one region.
        for i in -30..=30 {
            for j in -15..=15 {
                let p = DVec2::new(i as f64 * 0.05 + 0.001, j as f64 * 0.07 + 0.002);
                let hits = r
                    .iter()
                    .filter(|x| point_in_poly(p, &x.outer) && !x.holes.iter().any(|h| point_in_poly(p, h)))
                    .count();
                assert!(hits <= 1, "{p:?} in {hits} regions");
            }
        }
        // Circle polygons lose a little area against true circles.
        let shrink = (std::f64::consts::TAU / n as f64).sin() / (std::f64::consts::TAU / n as f64);
        let lens_area = (2.0 * 0.5f64.acos() - 0.5 * 3.0f64.sqrt()) * shrink;
        let circle_area = std::f64::consts::PI * shrink;
        let picked = select(&profs, &format(&[lens])).unwrap();
        assert_eq!(picked.len(), 1);
        assert!((area(&picked[0]) - lens_area).abs() < 0.01 * lens_area, "{}", area(&picked[0]));
        // The left side and the lens merge into the whole left circle.
        let mut sel = vec![left, lens];
        sel.sort_unstable();
        let picked = select(&profs, &format(&sel)).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(picked[0].1.is_empty());
        assert!((area(&picked[0]) - circle_area).abs() < 1e-9 * circle_area + 1e-6, "{}", area(&picked[0]));
        // The default is the union of both circles, as one region.
        let picked = select(&profs, "").unwrap();
        assert_eq!(picked.len(), 1);
        assert!(picked[0].1.is_empty());
        let union = 2.0 * circle_area - area(&select(&profs, &format(&[lens])).unwrap()[0]);
        assert!((area(&picked[0]) - union).abs() < 1e-9, "{} vs {union}", area(&picked[0]));
        // Left and right alone touch only at the crossing points: two regions.
        let mut sel = vec![left, right];
        sel.sort_unstable();
        assert_eq!(select(&profs, &format(&sel)).unwrap().len(), 2);
    }

    #[test]
    fn plate_with_a_hole_keeps_its_default() {
        let profs = vec![rect(0.0, 0.0, 10.0, 6.0), circle(DVec2::new(5.0, 3.0), 2.0, 64)];
        let r = regions(&profs);
        assert_eq!(r.len(), 2);
        assert_eq!(r.iter().filter(|x| x.default).count(), 1);
        let plate = region_at(&r, DVec2::new(1.0, 1.0)).unwrap();
        let inside = region_at(&r, DVec2::new(5.0, 3.0)).unwrap();
        assert!(r[plate].default && !r[inside].default);
        assert_eq!(r[plate].holes.len(), 1);
        // Loops that cross nothing come back exactly as drawn.
        assert_eq!(r[plate].outer, profs[0].points);
        let picked = select(&profs, "").unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].1.len(), 1);
        let hole = polygon_area(&profs[1].points).abs();
        assert!((area(&picked[0]) - (60.0 - hole)).abs() < 1e-9);
        // The plate and the inside of the hole merge into the full plate.
        let mut sel = vec![plate, inside];
        sel.sort_unstable();
        let picked = select(&profs, &format(&sel)).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(picked[0].1.is_empty());
        assert!((area(&picked[0]) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn island_in_a_hole_is_its_own_default_region() {
        let profs = vec![
            rect(0.0, 0.0, 20.0, 20.0),
            circle(DVec2::new(10.0, 10.0), 6.0, 96),
            circle(DVec2::new(10.0, 10.0), 2.0, 48),
        ];
        let r = regions(&profs);
        assert_eq!(r.len(), 3);
        let gap = region_at(&r, DVec2::new(14.0, 10.0)).unwrap();
        let island = region_at(&r, DVec2::new(10.0, 10.0)).unwrap();
        assert!(!r[gap].default && r[island].default);
        assert_eq!(r[gap].holes.len(), 1);
        let picked = select(&profs, "").unwrap();
        assert_eq!(picked.len(), 2);
        assert_eq!(picked.iter().map(|p| p.1.len()).sum::<usize>(), 1);
    }

    #[test]
    fn rectangle_and_square_sharing_an_edge() {
        let profs = vec![rect(0.0, 0.0, 2.0, 1.0), rect(2.0, 0.0, 3.0, 1.0)];
        let r = regions(&profs);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|x| x.default));
        let square = region_at(&r, DVec2::new(2.5, 0.5)).unwrap();
        let picked = select(&profs, &format(&[square])).unwrap();
        assert!((area(&picked[0]) - 1.0).abs() < 1e-12);
        let picked = select(&profs, "").unwrap();
        assert_eq!(picked.len(), 1);
        assert!((area(&picked[0]) - 3.0).abs() < 1e-12);
        // The shared edge is gone and so are the corners along the straight run.
        assert_eq!(picked[0].0.len(), 4);

        // A square that shares only part of an edge: a T junction on each side.
        let profs = vec![rect(0.0, 0.0, 2.0, 1.0), rect(2.0, 0.25, 2.5, 0.75)];
        let r = regions(&profs);
        assert_eq!(r.len(), 2);
        let picked = select(&profs, "").unwrap();
        assert_eq!(picked.len(), 1);
        assert!((area(&picked[0]) - 2.25).abs() < 1e-12);
        assert_eq!(picked[0].0.len(), 8);
    }

    #[test]
    fn chain_of_overlapping_circles_is_fast() {
        // 20 circles of 100 segments each, every one overlapping the next.
        let profs: Vec<Profile> = (0..20).map(|k| circle(DVec2::new(k as f64 * 1.5, 0.0), 1.0, 100)).collect();
        let t = std::time::Instant::now();
        let r = regions(&profs);
        let picked = select(&profs, "").unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!("2000 segments: {ms:.1} ms for regions and select");
        assert_eq!(r.len(), 39);
        assert_eq!(picked.len(), 1);
        assert!(ms < 2000.0, "{ms} ms");
    }
}
