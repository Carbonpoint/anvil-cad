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
