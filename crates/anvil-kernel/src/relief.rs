//! Relief: refine one surface of revolution and move its vertices along the
//! surface normal by a height field. This is how raised dot patterns
//! (hobnail, tortoiseshell) go onto a kettle body: one fine watertight mesh
//! instead of a boolean per dot.
//!
//! The facets of the chosen surface that are still whole revolve cells
//! (four vertices on the revolve lattice) are refined into a fine grid.
//! Facets that a boolean has cut are left as they are, and every vertex
//! they touch keeps a zero displacement, so the result stays watertight.

use crate::topology::{Face, Solid, Surface, SurfaceGeom, VertexId};
use crate::{KernelError, KernelResult};
use anvil_math::{Axis, DVec2, DVec3};
use std::collections::{HashMap, HashSet};

/// Parameters `(s, theta)` on a surface of revolution: `s` is the arc
/// length along the profile run in mm, `theta` the angle in radians.
pub struct RevolvedParam {
    pub axis: Axis,
    pub x_axis: DVec3,
    pub y_axis: DVec3,
    /// Profile run as `(r, z)`.
    pub run: Vec<DVec2>,
    /// Arc length at each run vertex.
    pub cum: Vec<f64>,
    /// Outward unit normal `(n_r, n_z)` at each run vertex.
    pub normals: Vec<DVec2>,
    pub segments: usize,
}

impl RevolvedParam {
    /// Build the parametrisation. `outward` is a point on one facet of the
    /// surface and that facet's world normal; it fixes which side the run
    /// normals point to.
    pub fn new(geom: &SurfaceGeom, outward: Option<(DVec3, DVec3)>) -> Option<Self> {
        let SurfaceGeom::Revolved { axis, x_axis, run, segments } = geom;
        if run.len() < 2 {
            return None;
        }
        let y_axis = axis.dir.cross(*x_axis).normalize_or_zero();
        let mut cum = Vec::with_capacity(run.len());
        let mut acc = 0.0;
        for i in 0..run.len() {
            if i > 0 {
                acc += (run[i] - run[i - 1]).length();
            }
            cum.push(acc);
        }
        // Segment normals (perpendicular in the r-z plane), then averaged.
        let seg_n: Vec<DVec2> = (0..run.len() - 1)
            .map(|i| {
                let d = (run[i + 1] - run[i]).normalize_or_zero();
                DVec2::new(d.y, -d.x)
            })
            .collect();
        let mut normals = Vec::with_capacity(run.len());
        for i in 0..run.len() {
            let n = if i == 0 {
                seg_n[0]
            } else if i == run.len() - 1 {
                seg_n[i - 1]
            } else {
                (seg_n[i - 1] + seg_n[i]).normalize_or_zero()
            };
            normals.push(n);
        }
        let mut p =
            RevolvedParam { axis: *axis, x_axis: *x_axis, y_axis, run: run.clone(), cum, normals, segments: *segments };
        if let Some((at, n)) = outward {
            // Compare against the run normal at the facet's own position.
            let (rz, theta) = p.frame(at);
            let (i, _) = p.nearest_run_vertex(rz);
            let world_n = p.normal(p.cum[i], theta);
            if world_n.dot(n) < 0.0 {
                for m in &mut p.normals {
                    *m = -*m;
                }
            }
        }
        Some(p)
    }

    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }

    /// Run vertex index and fraction for an arc length.
    fn locate_s(&self, s: f64) -> (usize, f64) {
        let last = self.run.len() - 1;
        if s <= 0.0 {
            return (0, 0.0);
        }
        let i = match self.cum.binary_search_by(|c| c.total_cmp(&s)) {
            Ok(i) => i.min(last - 1),
            Err(i) => i.saturating_sub(1).min(last - 1),
        };
        let len = self.cum[i + 1] - self.cum[i];
        let f = if len > 1e-12 { ((s - self.cum[i]) / len).clamp(0.0, 1.0) } else { 0.0 };
        (i, f)
    }

    /// `(r, z)` at arc length `s`.
    pub fn rz(&self, s: f64) -> DVec2 {
        let (i, f) = self.locate_s(s);
        self.run[i].lerp(self.run[i + 1], f)
    }

    pub fn radius(&self, s: f64) -> f64 {
        self.rz(s).x
    }

    /// Outward normal `(n_r, n_z)` at arc length `s`.
    pub fn normal_rz(&self, s: f64) -> DVec2 {
        let (i, f) = self.locate_s(s);
        // Inside a segment the normal is the segment normal; at a vertex
        // the average. Blend near vertices for a smooth field.
        let d = (self.run[i + 1] - self.run[i]).normalize_or_zero();
        let seg = DVec2::new(d.y, -d.x);
        let sign = if seg.dot(self.normals[i] + self.normals[i + 1]) < 0.0 { -1.0 } else { 1.0 };
        let seg = seg * sign;
        let t = if f < 0.5 { f * 2.0 } else { (1.0 - f) * 2.0 };
        let v = if f < 0.5 { self.normals[i] } else { self.normals[i + 1] };
        (seg * t + v * (1.0 - t)).normalize_or_zero()
    }

    pub fn world_rz(&self, rz: DVec2, theta: f64) -> DVec3 {
        let (sn, cs) = theta.sin_cos();
        self.axis.origin + self.x_axis * (rz.x * cs) + self.y_axis * (rz.x * sn) + self.axis.dir * rz.y
    }

    pub fn point(&self, s: f64, theta: f64) -> DVec3 {
        self.world_rz(self.rz(s), theta)
    }

    pub fn normal(&self, s: f64, theta: f64) -> DVec3 {
        let n = self.normal_rz(s);
        let (sn, cs) = theta.sin_cos();
        (self.x_axis * (n.x * cs) + self.y_axis * (n.x * sn) + self.axis.dir * n.y).normalize_or_zero()
    }

    /// `(r, z, theta)` of a world point in the axis frame.
    pub fn frame(&self, p: DVec3) -> (DVec2, f64) {
        let d = p - self.axis.origin;
        let z = d.dot(self.axis.dir);
        let x = d.dot(self.x_axis);
        let y = d.dot(self.y_axis);
        let r = (x * x + y * y).sqrt();
        let mut theta = y.atan2(x);
        if theta < 0.0 {
            theta += std::f64::consts::TAU;
        }
        (DVec2::new(r, z), theta)
    }

    /// Nearest run vertex index to a point in `(r, z)`, with its distance.
    fn nearest_run_vertex(&self, rz: DVec2) -> (usize, f64) {
        let mut best = (0usize, f64::INFINITY);
        for (i, v) in self.run.iter().enumerate() {
            let d = (*v - rz).length();
            if d < best.1 {
                best = (i, d);
            }
        }
        best
    }
}

/// A height field over a surface of revolution in `(s, theta)`.
pub trait HeightField {
    /// Height along the normal at arc length `s` and angle `theta`.
    fn height(&self, s: f64, theta: f64) -> f64;
    /// Called once before displacement with the parameters of every
    /// vertex that will not move: the boundary of the refined region and
    /// the edges of facets a cut has touched. A field made of discrete
    /// bumps can drop every bump that reaches one, so no bump is cut in
    /// half. The default keeps the field as it is.
    fn exclude_near(&mut self, _fixed: &[(f64, f64)], _param: &RevolvedParam) {}
}

impl<F: Fn(f64, f64) -> f64> HeightField for F {
    fn height(&self, s: f64, theta: f64) -> f64 {
        self(s, theta)
    }
}

/// Refine the facets tagged `Surface::Revolved { id: surface }` to about
/// `step` mm and move each vertex outward by the field's height.
pub fn relief(solid: &Solid, surface: u32, step: f64, field: &mut dyn HeightField) -> KernelResult<Solid> {
    let tag = Surface::Revolved { id: surface };
    let Some(geom) = solid.surfaces.get(&surface) else {
        return Err(KernelError::InvalidInput("the picked face is not a surface of revolution".into()));
    };
    let tagged: Vec<(crate::FaceId, &Face)> = solid.faces.iter().filter(|(_, f)| f.surface == tag).collect();
    let Some((_, first)) = tagged.first() else {
        return Err(KernelError::InvalidInput("no facets on that surface".into()));
    };
    let first_at = first.outer.iter().map(|&v| solid.pos(v)).sum::<DVec3>() / first.outer.len() as f64;
    let param = RevolvedParam::new(geom, Some((first_at, solid.face_normal(first))))
        .ok_or_else(|| KernelError::InvalidInput("surface run is too short".into()))?;
    let step = step.max(0.05);
    let segs = param.segments;
    let dtheta = std::f64::consts::TAU / segs as f64;
    let scale = solid.bounds().diagonal().max(1.0);
    let tol = 1e-6 * scale;

    // Lattice index of every vertex of the tagged facets.
    let mut lattice_of: HashMap<VertexId, (usize, usize)> = HashMap::new();
    let mut vertex_at: HashMap<(usize, usize), VertexId> = HashMap::new();
    let mut index_of = |v: VertexId, p: DVec3| -> Option<(usize, usize)> {
        if let Some(k) = lattice_of.get(&v) {
            return Some(*k);
        }
        let (rz, theta) = param.frame(p);
        let (i, d) = param.nearest_run_vertex(rz);
        if d > tol || rz.x < tol {
            return None;
        }
        let jf = theta / dtheta;
        let j = jf.round() as usize % segs;
        if (jf - jf.round()).abs() * dtheta * rz.x > tol {
            return None;
        }
        lattice_of.insert(v, (i, j));
        vertex_at.insert((i, j), v);
        Some((i, j))
    };

    // Whole revolve cells: (i, j) is the cell between run vertices i, i+1
    // and angle steps j, j+1. A cell may carry extra vertices on its edges
    // (split points a boolean sewed in); they must lie on the straight
    // edge between two corners and are kept in the fine mesh.
    let mut cells: HashMap<(usize, usize), (crate::FaceId, bool)> = HashMap::new();
    let mut extras: HashMap<(VertexId, VertexId), Vec<(f64, VertexId)>> = HashMap::new();
    let etol = 2e-6 * (1.0 + scale);
    for (fid, f) in &tagged {
        if f.outer.len() < 4 || !f.inner.is_empty() {
            continue;
        }
        let idx: Vec<Option<(usize, usize)>> = f.outer.iter().map(|&v| index_of(v, solid.pos(v))).collect();
        let corners: Vec<(usize, (usize, usize))> =
            idx.iter().enumerate().filter_map(|(k, x)| x.map(|c| (k, c))).collect();
        if corners.len() != 4 {
            continue;
        }
        let cidx: Vec<(usize, usize)> = corners.iter().map(|c| c.1).collect();
        let i0 = cidx.iter().map(|k| k.0).min().unwrap();
        let i1 = cidx.iter().map(|k| k.0).max().unwrap();
        if i1 != i0 + 1 {
            continue;
        }
        let js: HashSet<usize> = cidx.iter().map(|k| k.1).collect();
        if js.len() != 2 {
            continue;
        }
        let jmin = *js.iter().min().unwrap();
        let jmax = *js.iter().max().unwrap();
        let j0 = if jmax == jmin + 1 {
            jmin
        } else if jmin == 0 && jmax == segs - 1 {
            jmax
        } else {
            continue;
        };
        // Every other vertex must sit on the edge between its two corners.
        let n = f.outer.len();
        let mut ok = true;
        let mut local: Vec<((VertexId, VertexId), f64, VertexId)> = Vec::new();
        for (ci, &(k, _)) in corners.iter().enumerate() {
            let (k_next, _) = corners[(ci + 1) % 4];
            let (a_id, b_id) = (f.outer[k], f.outer[k_next]);
            let (a, b) = (solid.pos(a_id), solid.pos(b_id));
            let ab = b - a;
            let l2 = ab.length_squared();
            let mut m = (k + 1) % n;
            while m != k_next {
                let v = f.outer[m];
                let pv = solid.pos(v);
                let t = (pv - a).dot(ab) / l2;
                if !(t > 0.0 && t < 1.0) || (pv - (a + ab * t)).length() > etol {
                    ok = false;
                    break;
                }
                let (key, tt) = if a_id < b_id { ((a_id, b_id), t) } else { ((b_id, a_id), 1.0 - t) };
                local.push((key, tt, v));
                m = (m + 1) % n;
            }
            if !ok {
                break;
            }
        }
        if !ok {
            continue;
        }
        // Orientation: does (i0+1, j0) follow (i0, j0) in the corner cycle?
        let pos = |k: (usize, usize)| cidx.iter().position(|x| *x == k).unwrap();
        let a = pos((i0, j0));
        let b = pos((i0 + 1, j0));
        let forward = (a + 1) % 4 == b;
        cells.insert((i0, j0), (*fid, forward));
        for (key, t, v) in local {
            extras.entry(key).or_default().push((t, v));
        }
    }
    // An extra vertex that only refined cells use is a split point a
    // boolean plane left on a face it did not cut. Both cells are rebuilt,
    // so the vertex can go; keeping it would pin the surface there.
    {
        let cell_ids: HashSet<crate::FaceId> = cells.values().map(|c| c.0).collect();
        let mut uses: HashMap<VertexId, usize> = HashMap::new();
        for f in solid.faces.values() {
            for &v in f.outer.iter().chain(f.inner.iter().flatten()) {
                *uses.entry(v).or_default() += 1;
            }
        }
        let mut cell_uses: HashMap<VertexId, usize> = HashMap::new();
        for fid in &cell_ids {
            for &v in &solid.faces[*fid].outer {
                *cell_uses.entry(v).or_default() += 1;
            }
        }
        for list in extras.values_mut() {
            list.retain(|x| uses.get(&x.1).copied().unwrap_or(0) > cell_uses.get(&x.1).copied().unwrap_or(0));
        }
        extras.retain(|_, list| !list.is_empty());
    }
    if std::env::var("ANVIL_RELIEF_DEBUG").is_ok() {
        let cell_ids: HashSet<crate::FaceId> = cells.values().map(|c| c.0).collect();
        eprintln!("relief: {} tagged facets, {} cells, {} edges with extras", tagged.len(), cells.len(), extras.len());
        for i in 0..param.run.len().saturating_sub(1) {
            let n = (0..segs).filter(|&j| cells.contains_key(&(i, j))).count();
            if n != segs {
                eprintln!(
                    "  row {i} (s {:.1} to {:.1}, r {:.1}): {n} of {segs} cells",
                    param.cum[i],
                    param.cum[i + 1],
                    param.run[i].x
                );
            }
        }
        for (fid, f) in &tagged {
            if cell_ids.contains(fid) {
                continue;
            }
            let c = f.outer.iter().map(|&v| solid.pos(v)).sum::<DVec3>() / f.outer.len() as f64;
            let (rz, theta) = param.frame(c);
            let lattice = f.outer.iter().filter(|&&v| lattice_of.contains_key(&v)).count();
            eprintln!(
                "  refused: {} verts ({} on lattice) at theta {:.1} deg, z {:.1}, r {:.1}",
                f.outer.len(),
                lattice,
                theta.to_degrees(),
                rz.y,
                rz.x
            );
        }
    }
    if cells.is_empty() {
        return Err(KernelError::InvalidInput(
            "no whole revolve cells on that surface; put the pattern before cuts that touch the surface".into(),
        ));
    }

    // Fine lattice sizes.
    let r_max = param.run.iter().map(|p| p.x).fold(0.0, f64::max);
    let m_t = ((std::f64::consts::TAU * r_max / segs as f64) / step).ceil().max(1.0) as usize;
    let m_s: Vec<usize> =
        (0..param.run.len() - 1).map(|i| ((param.cum[i + 1] - param.cum[i]) / step).ceil().max(1.0) as usize).collect();
    let s_base: Vec<usize> = m_s
        .iter()
        .scan(0usize, |acc, m| {
            let b = *acc;
            *acc += m;
            Some(b)
        })
        .collect();
    let fine_t = segs * m_t;

    let is_cell = |i: isize, j: isize| -> bool {
        if i < 0 {
            return false;
        }
        cells.contains_key(&(i as usize, j.rem_euclid(segs as isize) as usize))
    };
    // Frozen coarse vertices: any adjacent cell missing.
    let coarse_free = |i: usize, j: usize| -> bool {
        let (i, j) = (i as isize, j as isize);
        is_cell(i, j) && is_cell(i - 1, j) && is_cell(i, j - 1) && is_cell(i - 1, j - 1)
    };

    let mut out = solid.clone();
    for (fid, _) in cells.values() {
        out.faces.remove(*fid);
    }
    let mut fine: HashMap<(usize, usize), VertexId> = HashMap::new();
    let mut frozen: HashSet<VertexId> = HashSet::new();
    // Fine vertices created on a coarse edge whose other side is not
    // refined: (edge, fraction along it, vertex). They are sewn into the
    // neighbouring face loops at the end.
    let mut seam: HashMap<(VertexId, VertexId), Vec<(f64, VertexId)>> = HashMap::new();
    let mut sew = |a: VertexId, b: VertexId, t: f64, v: VertexId| {
        let (key, t) = if a < b { ((a, b), t) } else { ((b, a), 1.0 - t) };
        seam.entry(key).or_default().push((t, v));
    };
    let mut new_faces: Vec<(Vec<VertexId>, bool)> = Vec::with_capacity(cells.len() * m_t * 2);

    let mut cell_keys: Vec<(usize, usize)> = cells.keys().copied().collect();
    cell_keys.sort_unstable();
    for (i, j) in cell_keys {
        let forward = cells[&(i, j)].1;
        let ms = m_s[i];
        let ring_lo_free = is_cell(i as isize - 1, j as isize);
        let ring_hi_free = is_cell(i as isize + 1, j as isize);
        let col_lo_free = is_cell(i as isize, j as isize - 1);
        let col_hi_free = is_cell(i as isize, j as isize + 1);
        let jn = (j + 1) % segs;
        let corner = |ii: usize, jj: usize| vertex_at[&(ii, jj)];
        let c00 = out.pos(corner(i, j));
        let c01 = out.pos(corner(i, jn));
        let c10 = out.pos(corner(i + 1, j));
        let c11 = out.pos(corner(i + 1, jn));
        let mut grid: Vec<VertexId> = Vec::with_capacity((ms + 1) * (m_t + 1));
        for a in 0..=ms {
            for b in 0..=m_t {
                let key = (s_base[i] + a, (j * m_t + b) % fine_t);
                let on_lo = a == 0;
                let on_hi = a == ms;
                let on_cl = b == 0;
                let on_ch = b == m_t;
                let id = if (on_lo || on_hi) && (on_cl || on_ch) {
                    // Coarse corner: reuse the existing vertex.
                    let v = corner(if on_lo { i } else { i + 1 }, if on_cl { j } else { jn });
                    if !coarse_free(if on_lo { i } else { i + 1 }, if on_cl { j } else { jn }) {
                        frozen.insert(v);
                    }
                    v
                } else if let Some(&v) = fine.get(&key) {
                    v
                } else if let Some(v) = {
                    // A coarse edge whose other side was refined earlier
                    // already carries split vertices at this step. Reuse
                    // one that sits where this fine vertex would go, or
                    // the two sides end up with twin vertices and a seam
                    // of open edges.
                    let fa = a as f64 / ms as f64;
                    let fb = b as f64 / m_t as f64;
                    let want = if on_lo {
                        Some((corner(i, j), corner(i, jn), fb))
                    } else if on_hi {
                        Some((corner(i + 1, j), corner(i + 1, jn), fb))
                    } else if on_cl {
                        Some((corner(i, j), corner(i + 1, j), fa))
                    } else if on_ch {
                        Some((corner(i, jn), corner(i + 1, jn), fa))
                    } else {
                        None
                    };
                    want.and_then(|(ea, eb, t)| {
                        let (ekey, tt) = if ea < eb { ((ea, eb), t) } else { ((eb, ea), 1.0 - t) };
                        let list = extras.get_mut(&ekey)?;
                        let k = list.iter().position(|x| (x.0 - tt).abs() < 1e-6)?;
                        Some(list.remove(k).1)
                    })
                } {
                    fine.insert(key, v);
                    frozen.insert(v);
                    v
                } else {
                    let fa = a as f64 / ms as f64;
                    let fb = b as f64 / m_t as f64;
                    let p = if on_lo {
                        c00.lerp(c01, fb)
                    } else if on_hi {
                        c10.lerp(c11, fb)
                    } else {
                        let s = param.cum[i] + fa * (param.cum[i + 1] - param.cum[i]);
                        let theta = (j as f64 + fb) * dtheta;
                        param.point(s, theta)
                    };
                    let v = out.add_vertex(p);
                    fine.insert(key, v);
                    let f = (on_lo && !ring_lo_free)
                        || (on_hi && !ring_hi_free)
                        || (on_cl && !col_lo_free)
                        || (on_ch && !col_hi_free);
                    if f {
                        frozen.insert(v);
                        if on_lo {
                            sew(corner(i, j), corner(i, jn), fb, v);
                        } else if on_hi {
                            sew(corner(i + 1, j), corner(i + 1, jn), fb, v);
                        } else if on_cl {
                            sew(corner(i, j), corner(i + 1, j), fa, v);
                        } else {
                            sew(corner(i, jn), corner(i + 1, jn), fa, v);
                        }
                    }
                    v
                };
                grid.push(id);
            }
        }
        let at = |a: usize, b: usize| grid[a * (m_t + 1) + b];
        for a in 0..ms {
            for b in 0..m_t {
                let q = [at(a, b), at(a + 1, b), at(a + 1, b + 1), at(a, b + 1)];
                new_faces.push((vec![q[0], q[1], q[2]], forward));
                new_faces.push((vec![q[0], q[2], q[3]], forward));
            }
        }
    }

    // Surface parameters of a vertex: arc length by projecting onto the
    // nearer of the two run segments at the nearest run vertex.
    let params_of = |p: DVec3| -> (f64, f64) {
        let (rz, theta) = param.frame(p);
        let (i, _) = param.nearest_run_vertex(rz);
        let mut best = (param.cum[i], f64::INFINITY);
        for k in [i.saturating_sub(1), i.min(param.run.len() - 2)] {
            let a = param.run[k];
            let d = param.run[k + 1] - a;
            let l2 = d.length_squared();
            if l2 < 1e-18 {
                continue;
            }
            let t = ((rz - a).dot(d) / l2).clamp(0.0, 1.0);
            let dist = (a + d * t - rz).length();
            if dist < best.1 {
                best = (param.cum[k] + t * l2.sqrt(), dist);
            }
        }
        (best.0, theta)
    };
    // Tell the field where the surface stays fixed: frozen fine vertices
    // and the extra vertices on cut edges.
    let mut fixed: Vec<(f64, f64)> = frozen.iter().map(|&v| params_of(out.pos(v))).collect();
    for list in extras.values() {
        fixed.extend(list.iter().map(|x| params_of(out.pos(x.1))));
    }
    field.exclude_near(&fixed, &param);

    // Displace every free vertex of the refined cells along the normal.
    let mut moved: HashSet<VertexId> = HashSet::new();
    for (loop_ids, _) in &new_faces {
        for &v in loop_ids {
            if frozen.contains(&v) || !moved.insert(v) {
                continue;
            }
            let p = out.pos(v);
            let (s, theta) = params_of(p);
            let h = field.height(s, theta);
            if h.abs() > 1e-12 {
                let n = param.normal(s, theta);
                out.vertices[v].pos = p + n * h;
            }
        }
    }
    for (mut loop_ids, forward) in new_faces {
        if !forward {
            loop_ids.reverse();
        }
        out.push_face(loop_ids, tag);
    }
    // Sew the fine edge vertices into the faces that were not refined, and
    // the extra vertices those faces already had into the fine faces. Both
    // use the straight coarse edge as the common ruler: every vertex on it
    // has a position t from 0 at one corner to 1 at the other.
    for list in seam.values_mut() {
        list.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    /// Coarse edge (sorted corner pair) and position along it.
    type EdgePos = ((VertexId, VertexId), f64);
    let mut on_edge: HashMap<VertexId, Vec<EdgePos>> = HashMap::new();
    let mut sub_extra: HashMap<(VertexId, VertexId), Vec<(f64, VertexId)>> = HashMap::new();
    let edge_keys: HashSet<(VertexId, VertexId)> = seam.keys().chain(extras.keys()).copied().collect();
    for key in &edge_keys {
        on_edge.entry(key.0).or_default().push((*key, 0.0));
        on_edge.entry(key.1).or_default().push((*key, 1.0));
        let mut seq: Vec<(f64, VertexId)> = vec![(0.0, key.0)];
        if let Some(list) = seam.get(key) {
            seq.extend(list.iter().copied());
        }
        seq.push((1.0, key.1));
        if let Some(ex) = extras.get(key) {
            for &(t, v) in ex {
                on_edge.entry(v).or_default().push((*key, t));
                // The fine sub-edge this extra falls into.
                let k = seq.iter().rposition(|x| x.0 < t).unwrap_or(0).min(seq.len() - 2);
                let (p, q) = (seq[k].1, seq[k + 1].1);
                let span = (seq[k + 1].0 - seq[k].0).max(1e-12);
                let local = (t - seq[k].0) / span;
                let (sk, lt) = if p < q { ((p, q), local) } else { ((q, p), 1.0 - local) };
                sub_extra.entry(sk).or_default().push((lt, v));
            }
        }
    }
    for list in sub_extra.values_mut() {
        list.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    if !edge_keys.is_empty() {
        let ids: Vec<crate::FaceId> = out.faces.keys().collect();
        for fid in ids {
            let face = &out.faces[fid];
            let mut loops: Vec<Vec<VertexId>> =
                std::iter::once(face.outer.clone()).chain(face.inner.iter().cloned()).collect();
            let mut changed = false;
            for lp in &mut loops {
                let n = lp.len();
                let mut new_lp = Vec::with_capacity(n);
                for k in 0..n {
                    let (a, b) = (lp[k], lp[(k + 1) % n]);
                    new_lp.push(a);
                    let key = if a < b { (a, b) } else { (b, a) };
                    if let Some(list) = sub_extra.get(&key) {
                        changed = true;
                        if a < b {
                            new_lp.extend(list.iter().map(|x| x.1));
                        } else {
                            new_lp.extend(list.iter().rev().map(|x| x.1));
                        }
                        continue;
                    }
                    let (Some(ea), Some(eb)) = (on_edge.get(&a), on_edge.get(&b)) else { continue };
                    let Some((ckey, ta, tb)) = ea.iter().find_map(|(ka, ta)| {
                        eb.iter().find(|(kb, tb)| kb == ka && (tb - ta).abs() > 1e-12).map(|(_, tb)| (*ka, *ta, *tb))
                    }) else {
                        continue;
                    };
                    let Some(list) = seam.get(&ckey) else { continue };
                    let (lo, hi) = (ta.min(tb), ta.max(tb));
                    let between: Vec<VertexId> = list.iter().filter(|x| x.0 > lo && x.0 < hi).map(|x| x.1).collect();
                    if between.is_empty() {
                        continue;
                    }
                    changed = true;
                    if ta < tb {
                        new_lp.extend(between);
                    } else {
                        new_lp.extend(between.into_iter().rev());
                    }
                }
                *lp = new_lp;
            }
            if changed {
                let f = &mut out.faces[fid];
                f.outer = loops.remove(0);
                f.inner = loops;
            }
        }
    }
    // Drop vertices no face uses any more (discarded extras).
    let used: HashSet<VertexId> =
        out.faces.values().flat_map(|f| f.outer.iter().chain(f.inner.iter().flatten()).copied()).collect();
    out.vertices.retain(|k, _| used.contains(&k));
    out.rebuild_edges();
    out.make_consistent();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{revolve_n, sphere};
    use anvil_math::Plane;

    #[test]
    fn sphere_relief_is_watertight_and_grows() {
        let s = sphere(DVec3::ZERO, 10.0).unwrap();
        assert!(s.surfaces.contains_key(&0));
        let v0 = s.volume();
        // One bump of height 1 everywhere: the sphere grows to radius 11.
        let r = relief(&s, 0, 1.0, &mut |_, _| 1.0).unwrap();
        let v1 = r.volume();
        let expect = v0 * (11.0f64 / 10.0).powi(3);
        assert!((v1 - expect).abs() / expect < 0.03, "{v1} vs {expect}");
        assert_eq!(r.open_edge_report(), (0, 0), "watertight");
    }

    #[test]
    fn cup_relief_only_touches_the_outer_wall() {
        // A cup: outer wall run and inner wall run are separate surfaces.
        let prof = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 30.0),
            DVec2::new(17.0, 30.0),
            DVec2::new(17.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        let plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::X, y_axis: DVec3::Z };
        let axis = Axis::new(DVec3::ZERO, DVec3::Z);
        let cup = revolve_n(&plane, &prof, &axis, std::f64::consts::TAU, 48).unwrap();
        let ids: HashSet<u32> = cup
            .faces
            .values()
            .filter_map(|f| match f.surface {
                Surface::Revolved { id } => Some(id),
                _ => None,
            })
            .collect();
        assert!(ids.len() >= 4, "bottom, outer wall, rim, inner wall, inner floor: {ids:?}");
        // Find the outer wall: the run whose radius is 20 everywhere.
        let outer = *cup
            .surfaces
            .iter()
            .find(|(_, g)| {
                let SurfaceGeom::Revolved { run, .. } = g;
                run.iter().all(|p| (p.x - 20.0).abs() < 1e-9) && run.len() == 2
            })
            .map(|(k, _)| k)
            .expect("outer wall run");
        let v0 = cup.volume();
        let r = relief(&cup, outer, 1.0, &mut |_, _| 0.5).unwrap();
        assert_eq!(r.open_edge_report(), (0, 0), "watertight");
        // The outer wall moved out by 0.5 except at the two frozen rings
        // (29 of 30 rows), and the refined facets sit on the true circle
        // instead of the 48-gon chords, which adds the polygon deficit.
        let shell = std::f64::consts::PI * (20.5f64.powi(2) - 20.0f64.powi(2)) * 30.0 * 29.0 / 30.0;
        let deficit = (std::f64::consts::PI * 400.0 - 48.0 * 0.5 * 400.0 * (std::f64::consts::TAU / 48.0).sin()) * 30.0;
        let expect = shell + deficit;
        let grown = r.volume() - v0;
        assert!((grown - expect).abs() / expect < 0.05, "grew {grown}, expected {expect}");
        // Inner wall vertices did not move: radius 17 still present.
        let inner_ok =
            r.vertices.values().any(|v| ((v.pos.x * v.pos.x + v.pos.y * v.pos.y).sqrt() - 17.0).abs() < 1e-9);
        assert!(inner_ok);
    }

    #[test]
    fn relief_on_the_run_next_to_a_refined_run_stays_watertight() {
        // A drum with a 45 degree underside: two runs that share a circle.
        let profile = [
            DVec2::new(0.0, 0.0),
            DVec2::new(9.0, 0.0),
            DVec2::new(22.0, 13.0),
            DVec2::new(22.0, 38.0),
            DVec2::new(0.0, 38.0),
        ];
        let plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::X, y_axis: DVec3::Z };
        let axis = Axis::new(DVec3::ZERO, DVec3::Z);
        let drum = revolve_n(&plane, &profile, &axis, std::f64::consts::TAU, 48).unwrap();
        let run_id = |s: &Solid, pick: &dyn Fn(&[DVec2]) -> bool| {
            *s.surfaces
                .iter()
                .find(|(_, g)| {
                    let SurfaceGeom::Revolved { run, .. } = g;
                    pick(run)
                })
                .map(|(k, _)| k)
                .expect("run")
        };
        let wall = run_id(&drum, &|run| run.iter().all(|p| (p.x - 22.0).abs() < 1e-9));
        let under = run_id(&drum, &|run| {
            run.iter().any(|p| (p.x - 9.0).abs() < 1e-9) && run.iter().any(|p| (p.x - 22.0).abs() < 1e-9)
        });
        let v0 = drum.volume();
        let mut bumps = |s: f64, theta: f64| 0.4 * (s * 1.3).sin().abs() * (theta * 5.0).cos().abs();
        let first = relief(&drum, wall, 0.5, &mut bumps).unwrap();
        assert_eq!(first.open_edge_report().0, 0, "first relief is watertight");
        let mut bumps2 = |s: f64, theta: f64| 0.3 * (s * 0.9).cos().abs() * (theta * 7.0).sin().abs();
        let second = relief(&first, under, 0.5, &mut bumps2).unwrap();
        let (open, _) = second.open_edge_report();
        assert_eq!(open, 0, "second relief next to the first is watertight");
        assert!(second.volume() > first.volume() && first.volume() > v0);
    }

    #[test]
    fn relief_after_a_cut_refines_cells_with_split_vertices() {
        use crate::csg::boolean;
        use crate::ops::cylinder;
        use crate::BooleanOp;
        let prof = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 30.0),
            DVec2::new(17.0, 30.0),
            DVec2::new(17.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        let plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::X, y_axis: DVec3::Z };
        let axis = Axis::new(DVec3::ZERO, DVec3::Z);
        let cup = revolve_n(&plane, &prof, &axis, std::f64::consts::TAU, 48).unwrap();
        // A hole through the outer wall.
        let side = Plane { origin: DVec3::new(30.0, 0.0, 15.0), x_axis: DVec3::Y, y_axis: DVec3::Z };
        let tool = cylinder(&side, DVec2::ZERO, 3.0, -20.0).unwrap();
        let cut = boolean(&cup, &tool, BooleanOp::Subtract);
        let outer = *cut
            .surfaces
            .iter()
            .find(|(_, g)| {
                let SurfaceGeom::Revolved { run, .. } = g;
                run.iter().all(|p| (p.x - 20.0).abs() < 1e-9) && run.len() == 2
            })
            .map(|(k, _)| k)
            .expect("outer wall run");
        let before = cut.open_edge_report();
        let tag = Surface::Revolved { id: outer };
        let with_extras = cut.faces.values().filter(|f| f.surface == tag && f.outer.len() > 4).count();
        assert!(with_extras > 0, "the cut leaves split vertices on neighbouring cells");
        let r = relief(&cut, outer, 1.0, &mut |_, _| 0.5).unwrap();
        assert_eq!(r.open_edge_report(), before, "no new seam defects");
        // Nearly every cell is refined: only the merged fragments around
        // the hole stay coarse, and they have many vertices.
        let cells_before = cut.faces.values().filter(|f| f.surface == tag).count();
        let coarse_left = r.faces.values().filter(|f| f.surface == tag && f.outer.len() > 6).count();
        assert!(coarse_left < cells_before / 4, "{coarse_left} of {cells_before} cells were not refined");
        let fine = r.faces.values().filter(|f| f.surface == tag).count();
        assert!(fine > cells_before * 20, "only {fine} faces after refinement");
    }
}
