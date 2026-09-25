//! Shell: hollow a solid, leaving walls of one thickness.
//!
//! The inner body has the same faces as the solid, each moved inward by
//! the thickness; an open face moves outward instead, so the inner body
//! cuts through it. Each vertex goes where the moved planes of its faces
//! meet, by least squares. On a faceted curved surface the planes around
//! a vertex are almost parallel; damped steps from the vertex moved
//! along its average normal keep that stable. The shell is the solid less
//! the inner body.

use crate::{BooleanOp, FaceId, KernelError, KernelResult, Solid};
use anvil_math::{DMat3, DVec3};
use std::collections::{HashMap, HashSet};

/// Hollow `solid` with walls `thickness` thick, leaving the faces in
/// `open` open.
pub fn shell(solid: &Solid, thickness: f64, open: &[FaceId]) -> KernelResult<Solid> {
    if !(thickness.is_finite() && thickness > 0.0) {
        return Err(KernelError::InvalidInput("the wall thickness must be above 0".into()));
    }
    let open: HashSet<FaceId> = open.iter().copied().collect();
    // Plane of every face: unit normal and offset, moved.
    let mut planes: HashMap<FaceId, (DVec3, f64)> = HashMap::new();
    for (id, f) in &solid.faces {
        let (n, area) = solid.face_normal_area(f);
        if area <= 1e-12 {
            continue;
        }
        let p = solid.pos(f.outer[0]);
        let shift = if open.contains(&id) { thickness } else { -thickness };
        planes.insert(id, (n, n.dot(p) + shift));
    }
    // Faces around each vertex.
    let mut around: HashMap<crate::VertexId, Vec<FaceId>> = HashMap::new();
    for (id, f) in &solid.faces {
        for &v in f.outer.iter().chain(f.inner.iter().flatten()) {
            around.entry(v).or_default().push(id);
        }
    }
    let mut inner = solid.clone();
    for (vid, v) in &solid.vertices {
        let Some(faces) = around.get(&vid) else { continue };
        // Merge planes that are the same up to facet noise.
        let mut seen: Vec<(DVec3, f64)> = Vec::new();
        for f in faces {
            if let Some(&(n, d)) = planes.get(f) {
                if !seen.iter().any(|(m, _)| m.dot(n) > 1.0 - 1e-9) {
                    seen.push((n, d));
                }
            }
        }
        if seen.is_empty() {
            continue;
        }
        let avg = seen.iter().map(|(n, _)| *n).sum::<DVec3>().normalize_or_zero();
        let target = v.pos - avg * thickness;
        // Least squares on the planes. Damped steps from `target` reach the
        // exact meeting point in every direction the planes fix; a
        // direction no plane fixes stays at `target`.
        let lambda = 1e-4;
        let mut a = DMat3::ZERO;
        let mut b = DVec3::ZERO;
        for (n, d) in &seen {
            a += DMat3::from_cols(*n * n.x, *n * n.y, *n * n.z);
            b += *n * *d;
        }
        let damped = a + DMat3::from_diagonal(DVec3::splat(lambda));
        if damped.determinant().abs() < 1e-18 {
            return Err(KernelError::InvalidInput("a vertex has no usable faces".into()));
        }
        let step = damped.inverse();
        let mut p = target;
        for _ in 0..12 {
            p += step * (b - a * p);
        }
        inner.vertices[vid].pos = p;
    }
    // A face that turned over means the wall is thicker than a feature.
    for (id, f) in &solid.faces {
        let (n0, a0) = solid.face_normal_area(f);
        let (n1, a1) = inner.face_normal_area(&inner.faces[id]);
        if a0 > 1e-9 && a1 > 1e-12 && n0.dot(n1) < 0.0 {
            return Err(KernelError::InvalidInput(format!(
                "a {thickness} mm wall is thicker than a feature of the body; use a thinner wall"
            )));
        }
    }
    let out = crate::csg::boolean(solid, &inner, BooleanOp::Subtract);
    if out.faces.is_empty() {
        return Err(KernelError::BooleanFailed("the shell came out empty".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops;

    fn top_face(s: &Solid) -> FaceId {
        s.faces.iter().find(|(_, f)| s.face_normal(f).z > 0.99).map(|(id, _)| id).unwrap()
    }

    #[test]
    fn a_closed_box_shell_keeps_a_hollow_inside() {
        let b = ops::box_solid(DVec3::ZERO, DVec3::new(40.0, 30.0, 20.0)).unwrap();
        let s = shell(&b, 2.0, &[]).unwrap();
        let want = 40.0 * 30.0 * 20.0 - 36.0 * 26.0 * 16.0;
        assert!((s.volume() - want).abs() < 1e-6 * want, "{} vs {want}", s.volume());
    }

    #[test]
    fn an_open_top_makes_a_tray() {
        let b = ops::box_solid(DVec3::ZERO, DVec3::new(40.0, 30.0, 20.0)).unwrap();
        let s = shell(&b, 2.0, &[top_face(&b)]).unwrap();
        let want = 40.0 * 30.0 * 20.0 - 36.0 * 26.0 * 18.0;
        assert!((s.volume() - want).abs() < 1e-6 * want, "{} vs {want}", s.volume());
        assert!(s.open_edges().is_empty());
    }

    #[test]
    fn a_cylinder_cup_has_an_even_wall() {
        let plane = anvil_math::Plane::XY;
        let c = ops::cylinder(&plane, anvil_math::DVec2::ZERO, 20.0, 30.0).unwrap();
        let s = shell(&c, 2.0, &[top_face(&c)]).unwrap();
        // The faceted cylinder's volume, less a faceted cylinder 2 mm in.
        let outer = c.volume();
        let inner_r_ratio = 18.0 / 20.0;
        let want = outer - outer * inner_r_ratio * inner_r_ratio * (28.0 / 30.0);
        assert!((s.volume() - want).abs() < 0.01 * want, "{} vs {want}", s.volume());
    }

    #[test]
    fn a_wall_thicker_than_the_part_is_refused() {
        let b = ops::box_solid(DVec3::ZERO, DVec3::new(40.0, 30.0, 4.0)).unwrap();
        assert!(shell(&b, 3.0, &[]).is_err());
    }
}
