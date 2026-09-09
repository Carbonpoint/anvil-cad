//! Triangle mesh output and polygon triangulation.

use crate::topology::{Face, FaceId, Solid};
use anvil_math::{DVec2, DVec3};
use rayon::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TriMesh {
    pub positions: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    /// Triangle vertex indices, three per triangle.
    pub indices: Vec<u32>,
    /// Face id that produced each triangle, for picking.
    pub face_of_tri: Vec<FaceId>,
}

impl TriMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn signed_volume(&self) -> f64 {
        let mut v = 0.0;
        for t in self.indices.as_chunks::<3>().0 {
            let a = self.positions[t[0] as usize];
            let b = self.positions[t[1] as usize];
            let c = self.positions[t[2] as usize];
            v += a.dot(b.cross(c));
        }
        v / 6.0
    }

    pub fn append(&mut self, other: TriMesh) {
        let base = self.positions.len() as u32;
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.indices.extend(other.indices.into_iter().map(|i| i + base));
        self.face_of_tri.extend(other.face_of_tri);
    }
}

/// Tessellate every face in parallel and merge the results.
pub fn tessellate(solid: &Solid) -> TriMesh {
    let faces: Vec<(FaceId, &Face)> = solid.faces.iter().collect();
    let parts: Vec<TriMesh> = faces.par_iter().map(|(id, f)| tessellate_face(solid, *id, f)).collect();
    let mut out = TriMesh::default();
    for p in parts {
        out.append(p);
    }
    out
}

fn tessellate_face(solid: &Solid, id: FaceId, f: &Face) -> TriMesh {
    let n = solid.face_normal(f);
    // Build a 2D frame on the face plane.
    let helper = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = n.cross(helper).normalize();
    let v = n.cross(u);
    let origin = solid.pos(f.outer[0]);
    let to2d = |p: DVec3| DVec2::new((p - origin).dot(u), (p - origin).dot(v));

    let pts3: Vec<DVec3> = f.outer.iter().map(|&vid| solid.pos(vid)).collect();
    let pts2: Vec<DVec2> = pts3.iter().map(|&p| to2d(p)).collect();
    let tris = ear_clip(&pts2);

    let face_of_tri = vec![id; tris.len() / 3];
    TriMesh {
        normals: vec![n; f.outer.len()],
        positions: pts3,
        indices: tris.iter().map(|&i| i as u32).collect(),
        face_of_tri,
    }
}

/// Ear clipping for a simple polygon. Input is any winding; output triangles
/// are counter-clockwise in the given 2D frame. O(n^2), fine for CAD facets.
pub fn ear_clip(poly: &[DVec2]) -> Vec<usize> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum();
    let mut idx: Vec<usize> = if area >= 0.0 { (0..n).collect() } else { (0..n).rev().collect() };
    let mut out = Vec::with_capacity((n - 2) * 3);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let ia = idx[(i + m - 1) % m];
            let ib = idx[i];
            let ic = idx[(i + 1) % m];
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if (b - a).perp_dot(c - b) <= 1e-14 {
                continue; // reflex or degenerate
            }
            let inside = idx.iter().any(|&j| j != ia && j != ib && j != ic && point_in_tri(poly[j], a, b, c));
            if inside {
                continue;
            }
            out.extend_from_slice(&[ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break; // degenerate polygon; emit a fan for the remainder
        }
    }
    if idx.len() == 3 {
        out.extend_from_slice(&[idx[0], idx[1], idx[2]]);
    } else {
        for i in 1..idx.len().saturating_sub(1) {
            out.extend_from_slice(&[idx[0], idx[i], idx[i + 1]]);
        }
    }
    out
}

fn point_in_tri(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let s1 = (b - a).perp_dot(p - a);
    let s2 = (c - b).perp_dot(p - b);
    let s3 = (a - c).perp_dot(p - c);
    s1 > 1e-14 && s2 > 1e-14 && s3 > 1e-14
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ear_clip_square() {
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(1.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(0.0, 1.0)];
        assert_eq!(ear_clip(&sq).len(), 6);
    }
    #[test]
    fn ear_clip_concave_l_shape() {
        let l = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 1.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(1.0, 2.0),
            DVec2::new(0.0, 2.0),
        ];
        assert_eq!(ear_clip(&l).len(), 12);
    }
}
