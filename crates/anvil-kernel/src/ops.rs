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
    s.make_consistent();
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
    s.make_consistent();
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

/// Segments used around a full circle for primitives.
pub const CIRCLE_SEGMENTS: usize = 48;

fn circle_points(r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            DVec2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

/// Axis-aligned box with one corner at `corner` and the given extents.
pub fn box_solid(corner: anvil_math::DVec3, size: anvil_math::DVec3) -> KernelResult<Solid> {
    if size.x.abs() < 1e-9 || size.y.abs() < 1e-9 || size.z.abs() < 1e-9 {
        return Err(KernelError::InvalidInput("box has a zero dimension".into()));
    }
    let plane = Plane {
        origin: corner,
        x_axis: anvil_math::DVec3::X * size.x.signum(),
        y_axis: anvil_math::DVec3::Y * size.y.signum(),
    };
    let prof = vec![
        DVec2::ZERO,
        DVec2::new(size.x.abs(), 0.0),
        DVec2::new(size.x.abs(), size.y.abs()),
        DVec2::new(0.0, size.y.abs()),
    ];
    extrude(&plane, &prof, size.z)
}

/// Cylinder with its base circle on `plane` at `center` (plane coords).
pub fn cylinder(plane: &Plane, center: DVec2, radius: f64, height: f64) -> KernelResult<Solid> {
    if radius <= 0.0 {
        return Err(KernelError::InvalidInput("cylinder radius must be positive".into()));
    }
    let prof: Vec<DVec2> = circle_points(radius, CIRCLE_SEGMENTS).into_iter().map(|p| p + center).collect();
    let mut s = extrude(plane, &prof, height)?;
    for f in s.faces.values_mut() {
        if f.outer.len() == 4 {
            f.surface = Surface::Cylindrical { id: 0 };
        }
    }
    Ok(s)
}

/// Sphere by revolving a half-disc profile.
pub fn sphere(center: anvil_math::DVec3, radius: f64) -> KernelResult<Solid> {
    if radius <= 0.0 {
        return Err(KernelError::InvalidInput("sphere radius must be positive".into()));
    }
    let n = CIRCLE_SEGMENTS / 2;
    // Half circle in the XZ plane from the south pole to the north pole, x >= 0.
    let mut prof = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / n as f64;
        prof.push(DVec2::new(radius * t.cos(), radius * t.sin()));
    }
    // Close along the axis (points on the axis are removed by revolve).
    let plane = Plane { origin: center, x_axis: anvil_math::DVec3::X, y_axis: anvil_math::DVec3::Z };
    let axis = Axis::new(center, anvil_math::DVec3::Z);
    revolve(&plane, &prof, &axis, std::f64::consts::TAU)
}

/// Torus about the Z axis through `center`.
pub fn torus(center: anvil_math::DVec3, major: f64, minor: f64) -> KernelResult<Solid> {
    if minor <= 0.0 || major <= minor {
        return Err(KernelError::InvalidInput("torus needs 0 < minor < major".into()));
    }
    let prof: Vec<DVec2> =
        circle_points(minor, CIRCLE_SEGMENTS / 2).into_iter().map(|p| p + DVec2::new(major, 0.0)).collect();
    let plane = Plane { origin: center, x_axis: anvil_math::DVec3::X, y_axis: anvil_math::DVec3::Z };
    let axis = Axis::new(center, anvil_math::DVec3::Z);
    revolve(&plane, &prof, &axis, std::f64::consts::TAU)
}

/// Connect a list of 3D rings (same point count) with quads and cap the ends.
fn skin_rings(rings: &[Vec<anvil_math::DVec3>], closed: bool, surface: Surface) -> KernelResult<Solid> {
    if rings.len() < 2 {
        return Err(KernelError::InvalidInput("need at least two sections".into()));
    }
    let n = rings[0].len();
    if rings.iter().any(|r| r.len() != n) || n < 3 {
        return Err(KernelError::InvalidInput("sections must have the same point count".into()));
    }
    let mut s = Solid::new();
    let ids: Vec<Vec<VertexId>> = rings.iter().map(|r| r.iter().map(|&p| s.add_vertex(p)).collect()).collect();
    let count = if closed { rings.len() } else { rings.len() - 1 };
    for r in 0..count {
        let r2 = (r + 1) % rings.len();
        for i in 0..n {
            let j = (i + 1) % n;
            s.add_face(vec![ids[r][i], ids[r2][i], ids[r2][j], ids[r][j]], surface);
        }
    }
    if !closed {
        let mut first = ids[0].clone();
        first.reverse();
        s.add_face(first, Surface::Plane);
        s.add_face(ids[rings.len() - 1].clone(), Surface::Plane);
    }
    s.make_consistent();
    Ok(s)
}

/// Sweep a profile along a polyline path. The first path point is placed at
/// the sketch plane origin; the profile is carried along with a
/// rotation-minimising frame. `closed` joins the last section to the first.
pub fn sweep(plane: &Plane, profile: &[DVec2], path: &[anvil_math::DVec3], closed: bool) -> KernelResult<Solid> {
    use anvil_math::DVec3;
    if profile.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if path.len() < 2 {
        return Err(KernelError::InvalidInput("sweep path needs at least two points".into()));
    }
    let prof = ccw(profile);
    let m = path.len();
    let seg_dir = |i: usize| -> DVec3 {
        let a = path[i];
        let b = path[(i + 1) % m];
        (b - a).normalize_or_zero()
    };
    // Tangent at each path vertex.
    let mut tangents: Vec<DVec3> = Vec::with_capacity(m);
    for i in 0..m {
        let t = if closed {
            (seg_dir((i + m - 1) % m) + seg_dir(i)).normalize_or_zero()
        } else if i == 0 {
            seg_dir(0)
        } else if i == m - 1 {
            seg_dir(m - 2)
        } else {
            (seg_dir(i - 1) + seg_dir(i)).normalize_or_zero()
        };
        tangents.push(if t.length_squared() < 1e-12 { seg_dir(i.min(m - 2)) } else { t });
    }
    // Initial frame: sketch plane axes, re-projected so x is perpendicular to t0.
    let t0 = tangents[0];
    let mut x = plane.x_axis - t0 * plane.x_axis.dot(t0);
    if x.length_squared() < 1e-12 {
        x = plane.y_axis - t0 * plane.y_axis.dot(t0);
    }
    let mut x = x.normalize();
    let mut rings = Vec::with_capacity(m);
    let mut prev_t = t0;
    for (i, &t) in tangents.iter().enumerate() {
        // Rotation-minimising transport of x from prev_t to t.
        let axis = prev_t.cross(t);
        if axis.length_squared() > 1e-14 {
            let ang = prev_t.dot(t).clamp(-1.0, 1.0).acos();
            x = anvil_math::DQuat::from_axis_angle(axis.normalize(), ang) * x;
        }
        x = (x - t * x.dot(t)).normalize();
        let y = t.cross(x);
        rings.push(prof.iter().map(|p| path[i] + x * p.x + y * p.y).collect::<Vec<_>>());
        prev_t = t;
    }
    skin_rings(&rings, closed, Surface::Cylindrical { id: 1 })
}

/// Resample a closed polyline to `n` points by arc length, starting at the
/// point nearest to `start_hint`.
fn resample_closed(poly: &[DVec2], n: usize, start_hint: DVec2) -> Vec<DVec2> {
    let k = poly.len();
    let start = (0..k)
        .min_by(|&a, &b| (poly[a] - start_hint).length().partial_cmp(&(poly[b] - start_hint).length()).unwrap())
        .unwrap_or(0);
    let pts: Vec<DVec2> = (0..k).map(|i| poly[(start + i) % k]).collect();
    let mut cum = vec![0.0];
    for i in 0..k {
        cum.push(cum[i] + (pts[(i + 1) % k] - pts[i]).length());
    }
    let total = cum[k];
    (0..n)
        .map(|j| {
            let d = total * j as f64 / n as f64;
            let seg = cum.iter().position(|&c| c > d).unwrap_or(k) - 1;
            let seg = seg.min(k - 1);
            let f = if cum[seg + 1] > cum[seg] { (d - cum[seg]) / (cum[seg + 1] - cum[seg]) } else { 0.0 };
            pts[seg].lerp(pts[(seg + 1) % k], f)
        })
        .collect()
}

/// Loft between closed profiles on their own planes, in order.
pub fn loft(sections: &[(Plane, Vec<DVec2>)]) -> KernelResult<Solid> {
    if sections.len() < 2 {
        return Err(KernelError::InvalidInput("loft needs at least two sections".into()));
    }
    let n = sections.iter().map(|(_, p)| p.len()).max().unwrap().max(CIRCLE_SEGMENTS);
    let mut rings = Vec::new();
    let mut hint = DVec2::new(1e9, 0.0);
    for (plane, prof) in sections {
        if prof.len() < 3 {
            return Err(KernelError::DegenerateProfile);
        }
        let p = ccw(prof);
        // Start every ring at the point with the largest x so twist stays small.
        let rs = resample_closed(&p, n, hint);
        hint = rs[0];
        rings.push(rs.iter().map(|&q| plane.to_world(q)).collect::<Vec<_>>());
    }
    skin_rings(&rings, false, Surface::Revolved { id: 2 })
}

/// Mirror a solid across a plane.
pub fn mirror(solid: &Solid, plane: &Plane) -> Solid {
    let n = plane.normal();
    let o = plane.origin;
    solid.transformed(|p| p - n * (2.0 * (p - o).dot(n)), true)
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

    #[test]
    fn primitives_have_expected_volumes() {
        let b = box_solid(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0)).unwrap();
        assert!((b.volume() - 24.0).abs() < 1e-9);
        let c = cylinder(&Plane::XY, DVec2::ZERO, 5.0, 2.0).unwrap();
        let exact = std::f64::consts::PI * 25.0 * 2.0;
        assert!((c.volume() - exact).abs() / exact < 0.01, "{}", c.volume());
        let s = sphere(DVec3::ZERO, 3.0).unwrap();
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 27.0;
        assert!((s.volume() - exact).abs() / exact < 0.02, "{} vs {}", s.volume(), exact);
        let t = torus(DVec3::ZERO, 10.0, 2.0).unwrap();
        let exact = 2.0 * std::f64::consts::PI * std::f64::consts::PI * 10.0 * 4.0;
        assert!((t.volume() - exact).abs() / exact < 0.02, "{} vs {}", t.volume(), exact);
    }

    #[test]
    fn sweep_straight_path_equals_extrude() {
        let path = vec![DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0), DVec3::new(0.0, 0.0, 10.0)];
        let s = sweep(&Plane::XY, &square(2.0), &path, false).unwrap();
        assert!((s.volume() - 40.0).abs() < 1e-9, "{}", s.volume());
    }

    #[test]
    fn loft_square_to_square_is_prism() {
        let top = Plane { origin: DVec3::new(0.0, 0.0, 3.0), ..Plane::XY };
        let s = loft(&[(Plane::XY, square(2.0)), (top, square(2.0))]).unwrap();
        assert!((s.volume() - 12.0).abs() < 1e-6, "{}", s.volume());
    }

    #[test]
    fn mirror_keeps_positive_volume() {
        let b = box_solid(DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 2.0, 2.0)).unwrap();
        let m = mirror(&b, &Plane::YZ);
        assert!((m.volume() - 8.0).abs() < 1e-9);
        assert!(m.bounds().max.x < 0.0);
    }
}
