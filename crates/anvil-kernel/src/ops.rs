//! Modeling operations.

use crate::topology::{Solid, Surface, VertexId};
use crate::{BooleanOp, EdgeId, KernelError, KernelResult};
use anvil_math::{Axis, DVec2, Plane};

/// Segments per full revolution when revolving.
pub const REVOLVE_SEGMENTS: usize = 64;

fn ccw(profile: &[DVec2]) -> Vec<DVec2> {
    let n = profile.len();
    let area: f64 = (0..n).map(|i| profile[i].perp_dot(profile[(i + 1) % n])).sum();
    if area < 0.0 {
        profile.iter().rev().cloned().collect()
    } else {
        profile.to_vec()
    }
}

/// Extrude a closed planar profile along the plane normal by `distance`.
/// A negative distance extrudes against the normal.
pub fn extrude(plane: &Plane, profile: &[DVec2], distance: f64) -> KernelResult<Solid> {
    if profile.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if distance.abs() <= anvil_math::LINEAR_TOL {
        return Err(KernelError::InvalidInput("extrude distance is zero".into()));
    }
    let prof = ccw(profile);
    let n = prof.len();
    let normal = plane.normal();
    let offset = normal * distance;

    let mut s = Solid::new();
    let bottom: Vec<VertexId> = prof.iter().map(|&p| s.add_vertex(plane.to_world(p))).collect();
    let top: Vec<VertexId> = prof.iter().map(|&p| s.add_vertex(plane.to_world(p) + offset)).collect();

    // Orientation: if distance > 0 the cap at +normal is "top" and must face +normal.
    let (cap_lo, cap_hi) = if distance > 0.0 { (&bottom, &top) } else { (&top, &bottom) };
    let mut lo = cap_lo.clone();
    lo.reverse();
    s.add_face(lo, Surface::Plane);
    s.add_face(cap_hi.clone(), Surface::Plane);

    for i in 0..n {
        let j = (i + 1) % n;
        let quad = if distance > 0.0 {
            vec![bottom[i], bottom[j], top[j], top[i]]
        } else {
            vec![top[i], top[j], bottom[j], bottom[i]]
        };
        s.add_face(quad, Surface::Plane);
    }
    Ok(s)
}

/// Revolve a closed profile about `axis` (given in world space, lying in the
/// sketch plane) by `angle` radians. A full 2*pi gives a closed ring.
pub fn revolve(plane: &Plane, profile: &[DVec2], axis: &Axis, angle: f64) -> KernelResult<Solid> {
    if profile.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if angle.abs() <= anvil_math::ANGULAR_TOL {
        return Err(KernelError::InvalidInput("revolve angle is zero".into()));
    }
    let angle = angle.clamp(-std::f64::consts::TAU, std::f64::consts::TAU);
    let full = (angle.abs() - std::f64::consts::TAU).abs() < 1e-9;
    let prof = ccw(profile);
    let n = prof.len();
    let steps = ((angle.abs() / std::f64::consts::TAU) * REVOLVE_SEGMENTS as f64).ceil().max(1.0) as usize;
    let rings = if full { steps } else { steps + 1 };

    // Points must all lie on one side of the axis.
    let world: Vec<_> = prof.iter().map(|&p| plane.to_world(p)).collect();
    let side = |p: anvil_math::DVec3| (p - axis.origin).cross(axis.dir).dot(plane.normal());
    let signs: Vec<f64> = world.iter().map(|&p| side(p)).collect();
    if signs.iter().any(|s| *s > 1e-9) && signs.iter().any(|s| *s < -1e-9) {
        return Err(KernelError::InvalidInput("profile crosses the revolve axis".into()));
    }
    // Ensure the sweep produces outward normals: flip if profile is on the
    // "negative" side relative to rotation direction.
    let flip = (signs.iter().sum::<f64>() < 0.0) != (angle < 0.0);

    let mut s = Solid::new();
    let mut ring_ids: Vec<Vec<VertexId>> = Vec::with_capacity(rings);
    for r in 0..rings {
        let t = angle * r as f64 / steps as f64;
        ring_ids.push(world.iter().map(|&p| s.add_vertex(axis.rotate(p, t))).collect());
    }
    let surf_id = 0u32;
    for r in 0..steps {
        let r2 = (r + 1) % rings;
        for i in 0..n {
            let j = (i + 1) % n;
            let mut quad = vec![ring_ids[r][i], ring_ids[r2][i], ring_ids[r2][j], ring_ids[r][j]];
            if flip {
                quad.reverse();
            }
            // Drop degenerate quads where a vertex lies on the axis.
            let pts: Vec<_> = quad.iter().map(|&v| s.pos(v)).collect();
            let degenerate = (pts[0] - pts[1]).length() < 1e-9 && (pts[3] - pts[2]).length() < 1e-9;
            if degenerate {
                continue;
            }
            if (pts[0] - pts[1]).length() < 1e-9 {
                quad.remove(1);
            } else if (pts[3] - pts[2]).length() < 1e-9 {
                quad.remove(2);
            }
            s.add_face(quad, Surface::Revolved { id: surf_id });
        }
    }
    if !full {
        let mut start = ring_ids[0].clone();
        let mut end = ring_ids[rings - 1].clone();
        if !flip {
            start.reverse();
        } else {
            end.reverse();
        }
        s.add_face(start, Surface::Plane);
        s.add_face(end, Surface::Plane);
    }
    Ok(s)
}

/// Boolean operations are not implemented in the native kernel yet.
/// See `docs/adr/0001-kernel-strategy.md` for the plan.
pub fn boolean(_a: &Solid, _b: &Solid, _op: BooleanOp) -> KernelResult<Solid> {
    Err(KernelError::Unsupported("boolean"))
}

/// Fillets are not implemented in the native kernel yet.
pub fn fillet(_solid: &Solid, _edges: &[EdgeId], _radius: f64) -> KernelResult<Solid> {
    Err(KernelError::Unsupported("fillet"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::DVec3;

    fn square(w: f64) -> Vec<DVec2> {
        vec![DVec2::new(0.0, 0.0), DVec2::new(w, 0.0), DVec2::new(w, w), DVec2::new(0.0, w)]
    }

    #[test]
    fn extrude_cube_volume_and_euler() {
        let s = extrude(&Plane::XY, &square(10.0), 10.0).unwrap();
        assert_eq!(s.faces.len(), 6);
        assert_eq!(s.euler_characteristic(), 2);
        assert!((s.volume() - 1000.0).abs() < 1e-9, "volume {}", s.volume());
    }

    #[test]
    fn extrude_negative_direction_still_positive_volume() {
        let s = extrude(&Plane::XY, &square(2.0), -3.0).unwrap();
        assert!((s.volume() - 12.0).abs() < 1e-9, "volume {}", s.volume());
    }

    #[test]
    fn revolve_full_ring_volume() {
        // Square 1x1 whose centroid is at x = 3, revolved about the y axis.
        // Pappus: V = A * 2*pi*R = 1 * 2*pi*3.
        let prof = vec![DVec2::new(2.5, 0.0), DVec2::new(3.5, 0.0), DVec2::new(3.5, 1.0), DVec2::new(2.5, 1.0)];
        let axis = Axis::new(DVec3::ZERO, DVec3::Y);
        let s = revolve(&Plane::XY, &prof, &axis, std::f64::consts::TAU).unwrap();
        let exact = std::f64::consts::TAU * 3.0;
        // Polygonal approximation of a circle underestimates slightly.
        assert!((s.volume() - exact).abs() / exact < 0.01, "volume {} vs {}", s.volume(), exact);
        assert_eq!(s.euler_characteristic(), 0, "torus has genus 1");
    }

    #[test]
    fn revolve_half_turn_has_caps() {
        let prof = vec![DVec2::new(1.0, 0.0), DVec2::new(2.0, 0.0), DVec2::new(2.0, 1.0), DVec2::new(1.0, 1.0)];
        let axis = Axis::new(DVec3::ZERO, DVec3::Y);
        let s = revolve(&Plane::XY, &prof, &axis, std::f64::consts::PI).unwrap();
        let exact = std::f64::consts::PI * 1.5;
        assert!((s.volume() - exact).abs() / exact < 0.01, "volume {} vs {}", s.volume(), exact);
        assert_eq!(s.euler_characteristic(), 2);
    }
}
