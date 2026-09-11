//! B-rep topology.

use anvil_math::{Aabb, DVec3};
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct VertexId;
    pub struct EdgeId;
    pub struct FaceId;
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub pos: DVec3,
}

/// A straight edge between two vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub a: VertexId,
    pub b: VertexId,
}

/// The analytic surface a face belongs to. Used for selection, fillets, and
/// smooth shading. Facets of one curved surface share the same tag.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Surface {
    Plane,
    /// Cylinder-like sweep of a line segment. `id` groups the facets.
    Cylindrical {
        id: u32,
    },
    /// Surface of revolution. `id` groups the facets.
    Revolved {
        id: u32,
    },
}

/// A planar face. `outer` and each of `inner` list vertex ids in order.
/// The outer loop is counter-clockwise when viewed from outside the solid.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Face {
    pub outer: Vec<VertexId>,
    pub inner: Vec<Vec<VertexId>>,
    pub surface: Surface,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Solid {
    pub vertices: SlotMap<VertexId, Vertex>,
    pub edges: SlotMap<EdgeId, Edge>,
    pub faces: SlotMap<FaceId, Face>,
}

impl Solid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_vertex(&mut self, pos: DVec3) -> VertexId {
        self.vertices.insert(Vertex { pos })
    }

    pub fn add_face(&mut self, outer: Vec<VertexId>, surface: Surface) -> FaceId {
        self.add_face_with_holes(outer, Vec::new(), surface)
    }

    pub fn add_face_with_holes(&mut self, outer: Vec<VertexId>, inner: Vec<Vec<VertexId>>, surface: Surface) -> FaceId {
        for lp in std::iter::once(&outer).chain(inner.iter()) {
            let n = lp.len();
            for i in 0..n {
                let a = lp[i];
                let b = lp[(i + 1) % n];
                if !self.edges.values().any(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a)) {
                    self.edges.insert(Edge { a, b });
                }
            }
        }
        self.faces.insert(Face { outer, inner, surface })
    }

    pub fn pos(&self, v: VertexId) -> DVec3 {
        self.vertices[v].pos
    }

    pub fn face_normal(&self, f: &Face) -> DVec3 {
        // Newell's method: robust for any planar polygon.
        let mut n = DVec3::ZERO;
        let k = f.outer.len();
        for i in 0..k {
            let p = self.pos(f.outer[i]);
            let q = self.pos(f.outer[(i + 1) % k]);
            n.x += (p.y - q.y) * (p.z + q.z);
            n.y += (p.z - q.z) * (p.x + q.x);
            n.z += (p.x - q.x) * (p.y + q.y);
        }
        n.normalize_or_zero()
    }

    pub fn bounds(&self) -> Aabb {
        let mut b = Aabb::empty();
        for v in self.vertices.values() {
            b.include(v.pos);
        }
        b
    }

    /// Signed volume by the divergence theorem over triangulated faces.
    /// Positive means outward-facing normals, which is the Anvil convention.
    pub fn volume(&self) -> f64 {
        let m = crate::mesh::tessellate(self);
        m.signed_volume()
    }

    /// Euler characteristic check: V - E + F == 2 for a genus-0 closed solid.
    pub fn euler_characteristic(&self) -> isize {
        self.vertices.len() as isize - self.edges.len() as isize + self.faces.len() as isize
    }
}

impl Solid {
    /// Apply a point transform to every vertex. If `flips_orientation` is
    /// true (mirror or negative scale) every face loop is reversed so normals
    /// still point outward.
    pub fn transformed(&self, f: impl Fn(DVec3) -> DVec3, flips_orientation: bool) -> Solid {
        let mut s = self.clone();
        for v in s.vertices.values_mut() {
            v.pos = f(v.pos);
        }
        if flips_orientation {
            for face in s.faces.values_mut() {
                face.outer.reverse();
                for l in &mut face.inner {
                    l.reverse();
                }
            }
        }
        s
    }

    /// Flip every face so the signed volume becomes positive.
    pub fn orient_outward(&mut self) {
        if self.volume() < 0.0 {
            for face in self.faces.values_mut() {
                face.outer.reverse();
                for l in &mut face.inner {
                    l.reverse();
                }
            }
        }
    }

    /// Edges worth drawing: boundary edges plus edges where the two adjacent
    /// faces meet at more than `min_angle` radians. Facet seams inside one
    /// smooth surface are skipped.
    pub fn feature_edges(&self, min_angle: f64) -> Vec<[DVec3; 2]> {
        use std::collections::HashMap;
        let mut adj: HashMap<(VertexId, VertexId), Vec<FaceId>> = HashMap::new();
        for (fid, f) in &self.faces {
            for lp in std::iter::once(&f.outer).chain(f.inner.iter()) {
                let n = lp.len();
                for i in 0..n {
                    let a = lp[i];
                    let b = lp[(i + 1) % n];
                    let key = if a < b { (a, b) } else { (b, a) };
                    adj.entry(key).or_default().push(fid);
                }
            }
        }
        let cos_min = min_angle.cos();
        let mut out = Vec::new();
        for ((a, b), faces) in adj {
            let draw = match faces.as_slice() {
                [f1, f2] => {
                    let fa = &self.faces[*f1];
                    let fb = &self.faces[*f2];
                    let same_surface = fa.surface != Surface::Plane && fa.surface == fb.surface;
                    let n1 = self.face_normal(fa);
                    let n2 = self.face_normal(fb);
                    !(same_surface && n1.dot(n2) > cos_min) && n1.dot(n2) < cos_min
                }
                _ => true,
            };
            if draw {
                out.push([self.pos(a), self.pos(b)]);
            }
        }
        out
    }
}

impl Solid {
    /// Make every face loop consistent with its neighbours: two faces that
    /// share an edge must traverse it in opposite directions. Then flip the
    /// whole solid so the signed volume is positive. Works per connected
    /// shell.
    pub fn make_consistent(&mut self) {
        use std::collections::{HashMap, HashSet, VecDeque};
        let ids: Vec<FaceId> = self.faces.keys().collect();
        // Directed edge -> face.
        let mut by_edge: HashMap<(VertexId, VertexId), Vec<FaceId>> = HashMap::new();
        for &fid in &ids {
            let f = &self.faces[fid];
            for lp in std::iter::once(&f.outer).chain(f.inner.iter()) {
                let n = lp.len();
                for i in 0..n {
                    let a = lp[i];
                    let b = lp[(i + 1) % n];
                    let key = if a < b { (a, b) } else { (b, a) };
                    by_edge.entry(key).or_default().push(fid);
                }
            }
        }
        let mut visited: HashSet<FaceId> = HashSet::new();
        for &seed in &ids {
            if visited.contains(&seed) {
                continue;
            }
            visited.insert(seed);
            let mut queue = VecDeque::from([seed]);
            while let Some(fid) = queue.pop_front() {
                let face_a = self.faces[fid].clone();
                let loops_a: Vec<&Vec<VertexId>> = std::iter::once(&face_a.outer).chain(face_a.inner.iter()).collect();
                for loop_a in loops_a {
                    let n = loop_a.len();
                    for i in 0..n {
                        let a = loop_a[i];
                        let b = loop_a[(i + 1) % n];
                        let key = if a < b { (a, b) } else { (b, a) };
                        for &nb in &by_edge[&key] {
                            if nb == fid || visited.contains(&nb) {
                                continue;
                            }
                            // Neighbour must traverse b -> a. If it goes a -> b, flip it.
                            let fb = &self.faces[nb];
                            let same_dir = std::iter::once(&fb.outer).chain(fb.inner.iter()).any(|lb| {
                                let m = lb.len();
                                (0..m).any(|k| lb[k] == a && lb[(k + 1) % m] == b)
                            });
                            if same_dir {
                                let face = &mut self.faces[nb];
                                face.outer.reverse();
                                for l in &mut face.inner {
                                    l.reverse();
                                }
                            }
                            visited.insert(nb);
                            queue.push_back(nb);
                        }
                    }
                }
            }
        }
        self.orient_outward();
    }
}

impl Solid {
    /// Centre of mass assuming uniform density, by the divergence theorem.
    pub fn centroid(&self) -> DVec3 {
        let m = crate::mesh::tessellate(self);
        let mut vol = 0.0;
        let mut c = DVec3::ZERO;
        for t in m.indices.as_chunks::<3>().0 {
            let a = m.positions[t[0] as usize];
            let b = m.positions[t[1] as usize];
            let d = m.positions[t[2] as usize];
            let v = a.dot(b.cross(d)) / 6.0;
            vol += v;
            c += (a + b + d) / 4.0 * v;
        }
        if vol.abs() < 1e-300 {
            DVec3::ZERO
        } else {
            c / vol
        }
    }
}

impl Solid {
    /// Remove T-junctions: where a vertex lies on the interior of another
    /// face's edge, insert it into that face loop. CSG output needs this so
    /// adjacency, feature edges, and the Euler characteristic are right.
    pub fn fix_t_junctions(&mut self) {
        let verts: Vec<(VertexId, DVec3)> = self.vertices.iter().map(|(id, v)| (id, v.pos)).collect();
        if verts.len() < 4 {
            return;
        }
        // Coarse grid for candidate lookup.
        let b = self.bounds();
        let cell = (b.diagonal() / 40.0).max(1e-6);
        let key = |p: DVec3| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64, (p.z / cell).floor() as i64);
        let mut grid: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = std::collections::HashMap::new();
        for (k, (_, p)) in verts.iter().enumerate() {
            grid.entry(key(*p)).or_default().push(k);
        }
        let tol = 1e-6 * (1.0 + b.diagonal());
        let ids: Vec<FaceId> = self.faces.keys().collect();
        for fid in ids {
            let face = self.faces[fid].clone();
            let mut loops: Vec<Vec<VertexId>> =
                std::iter::once(face.outer.clone()).chain(face.inner.iter().cloned()).collect();
            let mut changed = false;
            for lp in &mut loops {
                let mut out: Vec<VertexId> = Vec::with_capacity(lp.len() + 4);
                let n = lp.len();
                for i in 0..n {
                    let a_id = lp[i];
                    let b_id = lp[(i + 1) % n];
                    let a = self.pos(a_id);
                    let bb = self.pos(b_id);
                    out.push(a_id);
                    let ab = bb - a;
                    let len2 = ab.length_squared();
                    if len2 < 1e-18 {
                        continue;
                    }
                    let lo = key(a.min(bb));
                    let hi = key(a.max(bb));
                    let mut inserts: Vec<(f64, VertexId)> = Vec::new();
                    for x in lo.0 - 1..=hi.0 + 1 {
                        for y in lo.1 - 1..=hi.1 + 1 {
                            for z in lo.2 - 1..=hi.2 + 1 {
                                let Some(cands) = grid.get(&(x, y, z)) else { continue };
                                for &k in cands {
                                    let (vid, p) = verts[k];
                                    if vid == a_id || vid == b_id {
                                        continue;
                                    }
                                    let t = (p - a).dot(ab) / len2;
                                    if t <= 1e-9 || t >= 1.0 - 1e-9 {
                                        continue;
                                    }
                                    if (p - (a + ab * t)).length() < tol {
                                        inserts.push((t, vid));
                                    }
                                }
                            }
                        }
                    }
                    if !inserts.is_empty() {
                        inserts.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                        inserts.dedup_by_key(|x| x.1);
                        out.extend(inserts.into_iter().map(|x| x.1));
                        changed = true;
                    }
                }
                *lp = out;
            }
            if changed {
                let f = &mut self.faces[fid];
                f.outer = loops.remove(0);
                f.inner = loops;
            }
        }
        // Rebuild edges.
        self.edges.clear();
        let faces: Vec<Vec<VertexId>> =
            self.faces.values().flat_map(|f| std::iter::once(f.outer.clone()).chain(f.inner.iter().cloned())).collect();
        let mut seen = std::collections::HashSet::new();
        for f in faces {
            for i in 0..f.len() {
                let a = f[i];
                let b = f[(i + 1) % f.len()];
                let k = if a < b { (a, b) } else { (b, a) };
                if seen.insert(k) {
                    self.edges.insert(Edge { a, b });
                }
            }
        }
    }
}

impl Solid {
    /// Count undirected edges by how many face-loop uses they have.
    /// A closed 2-manifold has every edge used exactly twice.
    /// Returns (edges used once, edges used more than twice).
    pub fn open_edge_report(&self) -> (usize, usize) {
        let mut uses: std::collections::HashMap<(VertexId, VertexId), usize> = std::collections::HashMap::new();
        for f in self.faces.values() {
            for lp in std::iter::once(&f.outer).chain(f.inner.iter()) {
                let n = lp.len();
                for i in 0..n {
                    let (a, b) = (lp[i], lp[(i + 1) % n]);
                    if a == b {
                        continue;
                    }
                    *uses.entry(if a < b { (a, b) } else { (b, a) }).or_default() += 1;
                }
            }
        }
        (uses.values().filter(|&&u| u == 1).count(), uses.values().filter(|&&u| u > 2).count())
    }
}

/// Surface ids at or above this value are temporary tags that CSG gives to
/// flat input faces; merged faces return to `Surface::Plane`.
pub const CSG_TAG_BASE: u32 = 1_000_000;

impl Solid {
    /// Merge fragments that share a surface tag and a plane into one face
    /// with an outer loop and holes. Booleans split faces into many
    /// triangles; without this, each boolean multiplies the fragment count
    /// of the next one. Requires T-junctions to be fixed first.
    pub fn merge_coplanar_faces(&mut self) {
        use std::collections::BTreeMap as HashMap;
        // Group by tag and quantised plane.
        let mut groups: HashMap<(u64, i64, i64, i64, i64), Vec<FaceId>> = HashMap::new();
        for (fid, f) in &self.faces {
            let n = self.face_normal(f);
            if n.length_squared() < 0.5 {
                continue;
            }
            let d = n.dot(self.pos(f.outer[0]));
            let tag = match f.surface {
                Surface::Plane => 0u64,
                Surface::Cylindrical { id } => 1 + ((id as u64) << 2),
                Surface::Revolved { id } => 2 + ((id as u64) << 2),
            };
            let q = |x: f64| (x * 1e5).round() as i64;
            groups.entry((tag, q(n.x), q(n.y), q(n.z), q(d))).or_default().push(fid);
        }
        for (_, fids) in groups {
            if fids.len() < 2 {
                if let Some(&f) = fids.first() {
                    self.untag(f);
                }
                continue;
            }
            let normal = self.face_normal(&self.faces[fids[0]]);
            let surface = self.faces[fids[0]].surface;
            // Directed boundary edges with interior pairs cancelled.
            let mut edges: HashMap<(VertexId, VertexId), usize> = HashMap::new();
            for &fid in &fids {
                let f = &self.faces[fid];
                for lp in std::iter::once(&f.outer).chain(f.inner.iter()) {
                    let n = lp.len();
                    for i in 0..n {
                        let (a, b) = (lp[i], lp[(i + 1) % n]);
                        if a == b {
                            continue;
                        }
                        if let Some(c) = edges.get_mut(&(b, a)) {
                            *c -= 1;
                            if *c == 0 {
                                edges.remove(&(b, a));
                            }
                        } else {
                            *edges.entry((a, b)).or_default() += 1;
                        }
                    }
                }
            }
            // Chain into loops.
            let mut next: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
            for ((a, b), c) in &edges {
                for _ in 0..*c {
                    next.entry(*a).or_default().push(*b);
                }
            }
            let mut loops: Vec<Vec<VertexId>> = Vec::new();
            let mut ok = true;
            while let Some((&start, _)) = next.iter().find(|(_, v)| !v.is_empty()) {
                let mut lp = vec![start];
                let mut cur = start;
                loop {
                    let Some(list) = next.get_mut(&cur) else {
                        ok = false;
                        break;
                    };
                    let Some(nx) = list.pop() else {
                        ok = false;
                        break;
                    };
                    if nx == start {
                        break;
                    }
                    lp.push(nx);
                    cur = nx;
                    if lp.len() > edges.len() + 2 {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    break;
                }
                next.retain(|_, v| !v.is_empty());
                if lp.len() >= 3 {
                    loops.push(lp);
                }
            }
            if !ok || loops.is_empty() {
                for &f in &fids {
                    self.untag(f);
                }
                continue;
            }
            // Signed areas in the face plane: positive loops are outers.
            let helper = if normal.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
            let u = normal.cross(helper).normalize();
            let v = normal.cross(u);
            let to2 = |p: DVec3| (p.dot(u), p.dot(v));
            let area = |lp: &Vec<VertexId>| -> f64 {
                let n = lp.len();
                (0..n)
                    .map(|i| {
                        let (x0, y0) = to2(self.pos(lp[i]));
                        let (x1, y1) = to2(self.pos(lp[(i + 1) % n]));
                        x0 * y1 - x1 * y0
                    })
                    .sum::<f64>()
                    * 0.5
            };
            let inside = |pt: (f64, f64), lp: &Vec<VertexId>| -> bool {
                let n = lp.len();
                let mut c = false;
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = to2(self.pos(lp[i]));
                    let (xj, yj) = to2(self.pos(lp[j]));
                    if (yi > pt.1) != (yj > pt.1) && pt.0 < (xj - xi) * (pt.1 - yi) / (yj - yi) + xi {
                        c = !c;
                    }
                    j = i;
                }
                c
            };
            let (outers, holes): (Vec<Vec<VertexId>>, Vec<Vec<VertexId>>) =
                loops.into_iter().partition(|lp| area(lp) > 0.0);
            let mut new_faces: Vec<(Vec<VertexId>, Vec<Vec<VertexId>>)> =
                outers.into_iter().map(|o| (o, Vec::new())).collect();
            for h in holes {
                // Probe just inside the hole's first edge, on the solid side.
                let (x0, y0) = to2(self.pos(h[0]));
                let (x1, y1) = to2(self.pos(h[1]));
                let probe = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
                let mut best: Option<(usize, f64)> = None;
                for (k, (o, _)) in new_faces.iter().enumerate() {
                    if inside(probe, o) || o.iter().any(|&vid| h.contains(&vid)) {
                        let a = area(o);
                        if best.is_none_or(|(_, ba)| a < ba) {
                            best = Some((k, a));
                        }
                    }
                }
                match best {
                    Some((k, _)) => new_faces[k].1.push(h),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok || new_faces.is_empty() {
                for &f in &fids {
                    self.untag(f);
                }
                continue;
            }
            for &f in &fids {
                self.faces.remove(f);
            }
            let surface = match surface {
                Surface::Cylindrical { id } | Surface::Revolved { id } if id >= CSG_TAG_BASE => Surface::Plane,
                s => s,
            };
            for (outer, inner) in new_faces {
                self.faces.insert(Face { outer, inner, surface });
            }
        }
        // Drop vertices no face uses and rebuild edges.
        let used: std::collections::HashSet<VertexId> =
            self.faces.values().flat_map(|f| f.outer.iter().chain(f.inner.iter().flatten()).copied()).collect();
        self.vertices.retain(|k, _| used.contains(&k));
        self.edges.clear();
        let mut seen = std::collections::HashSet::new();
        let loops: Vec<Vec<VertexId>> =
            self.faces.values().flat_map(|f| std::iter::once(f.outer.clone()).chain(f.inner.iter().cloned())).collect();
        for lp in loops {
            for i in 0..lp.len() {
                let (a, b) = (lp[i], lp[(i + 1) % lp.len()]);
                let k = if a < b { (a, b) } else { (b, a) };
                if seen.insert(k) {
                    self.edges.insert(Edge { a, b });
                }
            }
        }
    }

    fn untag(&mut self, f: FaceId) {
        let face = &mut self.faces[f];
        if let Surface::Cylindrical { id } | Surface::Revolved { id } = face.surface {
            if id >= CSG_TAG_BASE {
                face.surface = Surface::Plane;
            }
        }
    }
}
