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
        let n = outer.len();
        for i in 0..n {
            let a = outer[i];
            let b = outer[(i + 1) % n];
            if !self.edges.values().any(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a)) {
                self.edges.insert(Edge { a, b });
            }
        }
        self.faces.insert(Face { outer, inner: Vec::new(), surface })
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
            let n = f.outer.len();
            for i in 0..n {
                let a = f.outer[i];
                let b = f.outer[(i + 1) % n];
                let key = if a < b { (a, b) } else { (b, a) };
                adj.entry(key).or_default().push(fid);
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
            let n = f.outer.len();
            for i in 0..n {
                let a = f.outer[i];
                let b = f.outer[(i + 1) % n];
                let key = if a < b { (a, b) } else { (b, a) };
                by_edge.entry(key).or_default().push(fid);
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
                let loop_a = self.faces[fid].outer.clone();
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
                        let lb = &self.faces[nb].outer;
                        let m = lb.len();
                        let same_dir = (0..m).any(|k| lb[k] == a && lb[(k + 1) % m] == b);
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
