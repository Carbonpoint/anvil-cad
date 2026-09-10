//! The renderable scene built from a document: one merged mesh plus the
//! bookkeeping needed for picking and highlighting.

use anvil_feature::Document;
use anvil_kernel::{FaceId, Solid, TriMesh};
use anvil_math::{Aabb, DVec3, Plane};

/// Feature edges sharper than this many radians are drawn.
const EDGE_ANGLE: f64 = 0.35;

#[derive(Default)]
pub struct Scene {
    pub mesh: TriMesh,
    /// For each triangle: index into `bodies`.
    pub tri_body: Vec<u32>,
    /// (feature index, body index within that feature).
    pub bodies: Vec<(usize, usize)>,
    /// Edge segments per body: (body index, a, b).
    pub edges: Vec<(u32, [DVec3; 2])>,
    /// Optional colour per body index.
    pub colors: Vec<Option<[u8; 3]>>,
    pub bounds: Aabb,
}

impl Scene {
    pub fn build(doc: &Document) -> Scene {
        let mut sc = Scene { bounds: Aabb::empty(), ..Default::default() };
        let mut per_feature_count: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for (fi, body) in doc.visible_bodies() {
            let bi = per_feature_count.entry(fi).or_insert(0);
            let body_index = sc.bodies.len() as u32;
            sc.bodies.push((fi, *bi));
            sc.colors.push(doc.appearance.get(&fi).copied());
            *bi += 1;
            let m = anvil_kernel::mesh::tessellate(body);
            sc.tri_body.extend(std::iter::repeat_n(body_index, m.triangle_count()));
            for e in body.feature_edges(EDGE_ANGLE) {
                sc.edges.push((body_index, e));
            }
            for p in &m.positions {
                sc.bounds.include(*p);
            }
            sc.mesh.append(m);
        }
        sc
    }

    pub fn body_of_tri(&self, tri: usize) -> Option<(usize, usize)> {
        self.tri_body.get(tri).map(|&b| self.bodies[b as usize])
    }

    pub fn face_of_tri(&self, tri: usize) -> Option<FaceId> {
        self.mesh.face_of_tri.get(tri).copied()
    }

    /// Solid behind a triangle, looked up in the document.
    pub fn solid_of_tri<'a>(&self, doc: &'a Document, tri: usize) -> Option<&'a Solid> {
        let (fi, bi) = self.body_of_tri(tri)?;
        doc.features.get(fi)?.output.as_ref()?.bodies.get(bi)
    }

    /// Outer loop and holes of a face in the coordinates of `face_plane`.
    pub fn face_loops(
        &self,
        doc: &Document,
        tri: usize,
    ) -> Option<(Plane, Vec<anvil_math::DVec2>, Vec<Vec<anvil_math::DVec2>>)> {
        let plane = self.face_plane(doc, tri)?;
        let solid = self.solid_of_tri(doc, tri)?;
        let face = &solid.faces[self.face_of_tri(tri)?];
        let outer = face.outer.iter().map(|&v| plane.to_local(solid.pos(v))).collect();
        let holes = face.inner.iter().map(|l| l.iter().map(|&v| plane.to_local(solid.pos(v))).collect()).collect();
        Some((plane, outer, holes))
    }

    /// A sketch plane on the picked face: origin at the face centroid,
    /// normal along the face normal, x axis aligned with world X where possible.
    pub fn face_plane(&self, doc: &Document, tri: usize) -> Option<Plane> {
        let solid = self.solid_of_tri(doc, tri)?;
        let face = &solid.faces[self.face_of_tri(tri)?];
        let n = solid.face_normal(face);
        let centroid = face.outer.iter().map(|&v| solid.pos(v)).sum::<DVec3>() / face.outer.len() as f64;
        let helper = if n.dot(DVec3::X).abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let x = (helper - n * helper.dot(n)).normalize();
        let y = n.cross(x);
        Some(Plane { origin: centroid, x_axis: x, y_axis: y })
    }
}
