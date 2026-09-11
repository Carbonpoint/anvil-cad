//! Fillet and chamfer on straight edges between flat faces.
//!
//! Method: for each edge, find the two faces that meet there, build a prism
//! whose cross-section is the material between the corner and the rounded
//! (or bevelled) surface, and subtract it (outside corners) or add it
//! (inside corners) with the CSG boolean. This covers prismatic parts,
//! which is most of what fillets and chamfers are used for. Edges on curved
//! facets are rejected with a clear error.

use crate::topology::Solid;
use crate::{csg, BooleanOp, KernelError, KernelResult};
use anvil_math::{DVec2, DVec3, Plane};

/// Arc segments used for a 90 degree fillet.
const ARC_SEGMENTS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendKind {
    Fillet,
    Chamfer,
}

/// Normals of the flat faces that contain the segment `a`-`b` along their
/// boundary. Coplanar fragments are merged by normal.
pub fn edge_face_normals(s: &Solid, a: DVec3, b: DVec3) -> Vec<DVec3> {
    let e = b - a;
    let len = e.length();
    if len < 1e-9 {
        return Vec::new();
    }
    let dir = e / len;
    let tol = 1e-5 * (1.0 + len);
    let mut normals: Vec<DVec3> = Vec::new();
    for f in s.faces.values() {
        let n = f.outer.len();
        let mut touches = false;
        for i in 0..n {
            let p = s.pos(f.outer[i]);
            let q = s.pos(f.outer[(i + 1) % n]);
            // Collinear with the edge line and overlapping its span.
            let off = |x: DVec3| (x - a) - dir * (x - a).dot(dir);
            if off(p).length() > tol || off(q).length() > tol {
                continue;
            }
            let (tp, tq) = ((p - a).dot(dir), (q - a).dot(dir));
            let (lo, hi) = (tp.min(tq), tp.max(tq));
            if hi - lo > tol && hi > tol && lo < len - tol {
                touches = true;
                break;
            }
        }
        if touches {
            let nrm = s.face_normal(f);
            if nrm.length_squared() > 0.5 && !normals.iter().any(|m| m.dot(nrm) > 0.9999) {
                normals.push(nrm);
            }
        }
    }
    normals
}

/// Cross-section and placement of the tool prism for one edge.
fn edge_tool(s: &Solid, a: DVec3, b: DVec3, size: f64, kind: BlendKind) -> KernelResult<(Solid, BooleanOp)> {
    let normals = edge_face_normals(s, a, b);
    if normals.len() != 2 {
        return Err(KernelError::InvalidInput(format!(
            "edge is not between exactly two flat faces (found {}); fillets work on straight edges of flat faces",
            normals.len()
        )));
    }
    let (n1, n2) = (normals[0], normals[1]);
    let e = (b - a).normalize();
    if n1.dot(e).abs() > 1e-6 || n2.dot(e).abs() > 1e-6 {
        return Err(KernelError::InvalidInput("face normals are not perpendicular to the edge".into()));
    }
    let cos_phi = n1.dot(n2).clamp(-1.0, 1.0);
    if cos_phi > 0.9999 {
        return Err(KernelError::InvalidInput("the two faces are tangent; nothing to round".into()));
    }
    // In-face directions pointing away from the edge, into each face.
    let u1 = (e.cross(n1)).normalize();
    let u2 = (e.cross(n2)).normalize();
    // Orient each u into its face: the face lies on the side away from the
    // other face's outward normal for a convex edge. Test convexity with
    // a probe point: a small step into face 1 must be behind face 2's plane.
    let u1 = if u1.dot(n2) > 0.0 { -u1 } else { u1 };
    let u2 = if u2.dot(n1) > 0.0 { -u2 } else { u2 };
    // A point just in front of face 1 and just behind face 2 is outside the
    // solid at an outside (convex) corner and inside it at an inside
    // (concave) corner, where the material wraps 270 degrees.
    let mid = (a + b) * 0.5;
    let probe = mid + (n1 - n2) * (size * 1e-3).max(1e-6);
    let convex = !point_inside(s, probe);
    // Tangent length along each face.
    let phi = cos_phi.acos();
    let l = match kind {
        BlendKind::Fillet => size * (phi / 2.0).tan(),
        BlendKind::Chamfer => size,
    };
    let edge_len = (b - a).length();
    // An outside-corner cutter may run past the edge ends (it only cuts
    // air there). An inside-corner filler must stop at the edge ends or it
    // would add material outside the part.
    let margin = if convex { size.max(edge_len * 0.01) } else { 0.0 };
    // Local 2D frame: x = n1, y = e x n1, extrude along e.
    let x = n1;
    let y = e.cross(n1);
    let origin = a - e * margin;
    let plane = Plane { origin, x_axis: x, y_axis: y };
    let to2 = |p: DVec3| DVec2::new((p - origin).dot(x), (p - origin).dot(y));
    let eps = (size * 0.02).max(1e-4);
    let t1 = a + u1 * l;
    let t2 = a + u2 * l;
    let mut pts: Vec<DVec3> = Vec::new();
    if convex {
        // Material between the corner and the blend, pushed slightly out.
        let out = (n1 + n2).normalize_or_zero();
        pts.push(a + out * eps);
        pts.push(t2 + n2 * eps);
        pts.push(t2);
        match kind {
            BlendKind::Chamfer => {}
            BlendKind::Fillet => {
                let c = t1 - n1 * size;
                arc_points(c, t2 - c, t1 - c, &mut pts);
            }
        }
        pts.push(t1);
        pts.push(t1 + n1 * eps);
    } else {
        // Inside corner: add material between the corner and the blend.
        // Faces point into the notch; the wedge is on the normal side.
        let t1 = a - u1 * l;
        let t2 = a - u2 * l;
        let into = -(n1 + n2).normalize_or_zero();
        pts.push(a + into * eps);
        pts.push(t1 - n1 * eps);
        pts.push(t1);
        match kind {
            BlendKind::Chamfer => {}
            BlendKind::Fillet => {
                let c = t1 + n1 * size;
                arc_points(c, t1 - c, t2 - c, &mut pts);
            }
        }
        pts.push(t2);
        pts.push(t2 - n2 * eps);
    }
    let prof: Vec<DVec2> = pts.iter().map(|&p| to2(p)).collect();
    let tool = crate::ops::extrude(&plane, &prof, edge_len + 2.0 * margin)?;
    Ok((tool, if convex { BooleanOp::Subtract } else { BooleanOp::Union }))
}

/// Points strictly between `from` and `to` (vectors from the centre) along
/// the shorter arc.
fn arc_points(c: DVec3, from: DVec3, to: DVec3, out: &mut Vec<DVec3>) {
    let r = from.length();
    let (f, t) = (from.normalize(), to.normalize());
    let ang = f.dot(t).clamp(-1.0, 1.0).acos();
    let n = ((ang / std::f64::consts::FRAC_PI_2) * ARC_SEGMENTS as f64).ceil().max(2.0) as usize;
    let axis = f.cross(t).normalize_or_zero();
    for i in 1..n {
        let q = anvil_math::DQuat::from_axis_angle(axis, ang * i as f64 / n as f64);
        out.push(c + (q * f) * r);
    }
}

/// Ray-parity inside test against the tessellated solid.
pub fn point_inside(s: &Solid, p: DVec3) -> bool {
    let m = crate::mesh::tessellate(s);
    // Slightly skewed direction avoids hitting edges exactly.
    let d = DVec3::new(0.577_2, 0.311_7, 0.759_3).normalize();
    let mut hits = 0;
    for t in m.indices.as_chunks::<3>().0 {
        let (v0, v1, v2) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let h = d.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-14 {
            continue;
        }
        let inv = 1.0 / det;
        let sv = p - v0;
        let u = sv.dot(h) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = sv.cross(e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        if e2.dot(q) * inv > 1e-12 {
            hits += 1;
        }
    }
    hits % 2 == 1
}

/// Blend each edge in turn. Edges are given as world-space end points.
pub fn blend_edges(s: &Solid, edges: &[[DVec3; 2]], size: f64, kind: BlendKind) -> KernelResult<Solid> {
    if size <= 0.0 {
        return Err(KernelError::InvalidInput("size must be positive".into()));
    }
    if edges.is_empty() {
        return Err(KernelError::InvalidInput("select one or more edges".into()));
    }
    // Build every tool against the original solid so later edges still find
    // their faces, then apply them one by one.
    let mut tools = Vec::new();
    for [a, b] in edges {
        tools.push(edge_tool(s, *a, *b, size, kind)?);
    }
    let mut acc = s.clone();
    for (tool, op) in tools {
        acc = csg::boolean(&acc, &tool, op);
    }
    if acc.faces.is_empty() {
        return Err(KernelError::BooleanFailed("blend removed the whole body".into()));
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::box_solid;

    #[test]
    fn fillet_one_box_edge_removes_the_corner_sliver() {
        let b = box_solid(DVec3::ZERO, DVec3::new(20.0, 10.0, 10.0)).unwrap();
        let edge = [DVec3::new(0.0, 0.0, 10.0), DVec3::new(20.0, 0.0, 10.0)];
        let r = blend_edges(&b, &[edge], 2.0, BlendKind::Fillet).unwrap();
        let removed = 20.0 * 4.0 * (1.0 - std::f64::consts::FRAC_PI_4);
        let got = 2000.0 - r.volume();
        assert!((got - removed).abs() / removed < 0.02, "{got} vs {removed}");
        assert_eq!(r.open_edge_report(), (0, 0));
    }

    #[test]
    fn chamfer_one_box_edge() {
        let b = box_solid(DVec3::ZERO, DVec3::new(20.0, 10.0, 10.0)).unwrap();
        let edge = [DVec3::new(0.0, 0.0, 10.0), DVec3::new(20.0, 0.0, 10.0)];
        let r = blend_edges(&b, &[edge], 3.0, BlendKind::Chamfer).unwrap();
        let got = 2000.0 - r.volume();
        assert!((got - 90.0).abs() < 1e-6, "{got}");
    }

    #[test]
    fn inside_corner_fillet_adds_material() {
        // L-shape: union of two boxes gives a concave edge at x = 5, z = 5.
        let a = box_solid(DVec3::ZERO, DVec3::new(20.0, 10.0, 5.0)).unwrap();
        let b = box_solid(DVec3::ZERO, DVec3::new(5.0, 10.0, 20.0)).unwrap();
        let l = csg::boolean(&a, &b, BooleanOp::Union);
        let v0 = l.volume();
        let edge = [DVec3::new(5.0, 0.0, 5.0), DVec3::new(5.0, 10.0, 5.0)];
        let r = blend_edges(&l, &[edge], 3.0, BlendKind::Fillet).unwrap();
        let added = 10.0 * 9.0 * (1.0 - std::f64::consts::FRAC_PI_4);
        assert!(((r.volume() - v0) - added).abs() / added < 0.03, "{} vs {added}", r.volume() - v0);
    }

    #[test]
    fn fillet_four_top_edges() {
        let b = box_solid(DVec3::ZERO, DVec3::new(20.0, 20.0, 10.0)).unwrap();
        let z = 10.0;
        let edges = [
            [DVec3::new(0.0, 0.0, z), DVec3::new(20.0, 0.0, z)],
            [DVec3::new(20.0, 0.0, z), DVec3::new(20.0, 20.0, z)],
            [DVec3::new(20.0, 20.0, z), DVec3::new(0.0, 20.0, z)],
            [DVec3::new(0.0, 20.0, z), DVec3::new(0.0, 0.0, z)],
        ];
        let r = blend_edges(&b, &edges, 2.0, BlendKind::Fillet).unwrap();
        assert!(r.volume() < 4000.0 && r.volume() > 4000.0 - 4.0 * 20.0 * 4.0);
    }
}
