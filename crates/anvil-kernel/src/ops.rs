//! Modeling operations.

use crate::topology::{Solid, Surface, SurfaceGeom, VertexId};
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

/// Extrude an outer profile with holes. Holes must lie inside the outer
/// loop and not touch it.
pub fn extrude_with_holes(plane: &Plane, outer: &[DVec2], holes: &[Vec<DVec2>], distance: f64) -> KernelResult<Solid> {
    if holes.is_empty() {
        return extrude(plane, outer, distance);
    }
    if outer.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if distance.abs() <= anvil_math::LINEAR_TOL {
        return Err(KernelError::InvalidInput("extrude distance is zero".into()));
    }
    let prof = ccw(outer);
    let hole_profs: Vec<Vec<DVec2>> = holes
        .iter()
        .filter(|h| h.len() >= 3)
        .map(|h| {
            let mut c = ccw(h);
            c.reverse(); // holes run clockwise
            c
        })
        .collect();
    let offset = plane.normal() * distance;
    let mut s = Solid::new();
    let mk = |s: &mut Solid, pts: &[DVec2], lift: bool| -> Vec<VertexId> {
        pts.iter()
            .map(|&p| s.add_vertex(plane.to_world(p) + if lift { offset } else { anvil_math::DVec3::ZERO }))
            .collect()
    };
    let bottom = mk(&mut s, &prof, false);
    let top = mk(&mut s, &prof, true);
    let hb: Vec<Vec<VertexId>> = hole_profs.iter().map(|h| mk(&mut s, h, false)).collect();
    let ht: Vec<Vec<VertexId>> = hole_profs.iter().map(|h| mk(&mut s, h, true)).collect();
    let mut lo = bottom.clone();
    lo.reverse();
    let lo_holes: Vec<Vec<VertexId>> = hb
        .iter()
        .map(|h| {
            let mut r = h.clone();
            r.reverse();
            r
        })
        .collect();
    s.add_face_with_holes(lo, lo_holes, Surface::Plane);
    s.add_face_with_holes(top.clone(), ht.clone(), Surface::Plane);
    let walls = |s: &mut Solid, b: &[VertexId], t: &[VertexId]| {
        let n = b.len();
        for i in 0..n {
            let j = (i + 1) % n;
            s.add_face(vec![b[i], b[j], t[j], t[i]], Surface::Plane);
        }
    };
    walls(&mut s, &bottom, &top);
    for (b, t) in hb.iter().zip(ht.iter()) {
        walls(&mut s, b, t);
    }
    s.make_consistent();
    Ok(s)
}

/// Revolve a closed profile about `axis` (given in world space, lying in the
/// sketch plane) by `angle` radians. A full 2*pi gives a closed ring.
pub fn revolve(plane: &Plane, profile: &[DVec2], axis: &Axis, angle: f64) -> KernelResult<Solid> {
    revolve_n(plane, profile, axis, angle, REVOLVE_SEGMENTS)
}

/// Turn angle (radians) between profile segments that starts a new
/// surface run. Smooth curves stay one surface; corners split them.
const RUN_CORNER: f64 = 0.35;

/// Like `revolve`, with `segments` facets per full turn. Each smooth run of
/// the profile becomes its own `Surface::Revolved` id, and the solid
/// records the run geometry in `surfaces`, so a face can be selected and
/// patterned as one continuous surface.
pub fn revolve_n(plane: &Plane, profile: &[DVec2], axis: &Axis, angle: f64, segments: usize) -> KernelResult<Solid> {
    if profile.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if angle.abs() <= anvil_math::ANGULAR_TOL {
        return Err(KernelError::InvalidInput("revolve angle is zero".into()));
    }
    let segments = segments.clamp(6, 4096);
    let angle = angle.clamp(-std::f64::consts::TAU, std::f64::consts::TAU);
    let full = (angle.abs() - std::f64::consts::TAU).abs() < 1e-9;
    let prof = ccw(profile);
    let n = prof.len();
    let steps = ((angle.abs() / std::f64::consts::TAU) * segments as f64).ceil().max(1.0) as usize;
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

    // Axis frame: r along `x_axis` (from the axis toward the profile), z along the axis.
    let radial = |p: anvil_math::DVec3| {
        let d = p - axis.origin;
        d - axis.dir * d.dot(axis.dir)
    };
    let x_axis = world
        .iter()
        .map(|&p| radial(p))
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .map(|v| v.normalize_or_zero())
        .unwrap_or(anvil_math::DVec3::X);
    let rz = |p: anvil_math::DVec3| DVec2::new(radial(p).dot(x_axis), (p - axis.origin).dot(axis.dir));
    let on_axis: Vec<bool> = world.iter().map(|&p| radial(p).length() < 1e-9).collect();

    // Split the closed profile into smooth runs. A run breaks at a corner
    // or at a segment lying on the axis (which produces no faces).
    let seg_dir = |i: usize| (rz(world[(i + 1) % n]) - rz(world[i])).normalize_or_zero();
    let seg_on_axis = |i: usize| on_axis[i] && on_axis[(i + 1) % n];
    let corner_at = |i: usize| {
        // Corner between segment i-1 and segment i.
        let a = seg_dir((i + n - 1) % n);
        let b = seg_dir(i);
        seg_on_axis((i + n - 1) % n) || seg_on_axis(i) || a.dot(b) < RUN_CORNER.cos()
    };
    let mut run_of_seg = vec![0u32; n];
    let mut runs: Vec<Vec<usize>> = Vec::new();
    // Start at a corner if there is one, so a smooth loop is one run.
    let start = (0..n).find(|&i| corner_at(i)).unwrap_or(0);
    let mut current: Vec<usize> = Vec::new();
    for k in 0..n {
        let i = (start + k) % n;
        if k > 0 && corner_at(i) && !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
        if !seg_on_axis(i) {
            current.push(i);
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    for (id, segs) in runs.iter().enumerate() {
        for &i in segs {
            run_of_seg[i] = id as u32;
        }
    }

    let mut s = Solid::new();
    let mut ring_ids: Vec<Vec<VertexId>> = Vec::with_capacity(rings);
    // Profile points on the axis become one pole vertex shared by all rings.
    let mut poles: Vec<Option<VertexId>> = vec![None; world.len()];
    for r in 0..rings {
        let t = angle * r as f64 / steps as f64;
        let mut ids = Vec::with_capacity(world.len());
        for (k, &p) in world.iter().enumerate() {
            if on_axis[k] {
                let id = *poles[k].get_or_insert_with(|| s.add_vertex(p));
                ids.push(id);
            } else {
                ids.push(s.add_vertex(axis.rotate(p, t)));
            }
        }
        ring_ids.push(ids);
    }
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
            s.push_face(quad, Surface::Revolved { id: run_of_seg[i] });
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
        s.push_face(start, Surface::Plane);
        s.push_face(end, Surface::Plane);
    }
    for (id, segs) in runs.iter().enumerate() {
        let mut run: Vec<DVec2> = segs.iter().map(|&i| rz(world[i])).collect();
        if let Some(&last) = segs.last() {
            run.push(rz(world[(last + 1) % n]));
        }
        s.surfaces.insert(id as u32, SurfaceGeom::Revolved { axis: *axis, x_axis, run, segments });
    }
    s.rebuild_edges();
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
    sweep_scaled(plane, profile, path, closed, &[])
}

/// Sweep with the section scaled at each path vertex by `scales`. An
/// empty slice means no scaling; otherwise it must have one value per
/// path vertex. A tapered spout is a pipe with scales from 1 down to the
/// tip ratio.
pub fn sweep_scaled(
    plane: &Plane,
    profile: &[DVec2],
    path: &[anvil_math::DVec3],
    closed: bool,
    scales: &[f64],
) -> KernelResult<Solid> {
    use anvil_math::DVec3;
    if profile.len() < 3 {
        return Err(KernelError::DegenerateProfile);
    }
    if path.len() < 2 {
        return Err(KernelError::InvalidInput("sweep path needs at least two points".into()));
    }
    if !scales.is_empty() && scales.len() != path.len() {
        return Err(KernelError::InvalidInput("sweep needs one scale per path vertex".into()));
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
        // Miter at bends: stretch the section along the bend direction by
        // 1/cos(half angle) so the pipe keeps its size through the corner.
        let interior = closed || (i > 0 && i + 1 < m);
        let (bend, stretch) = if interior {
            let din = seg_dir((i + m - 1) % m);
            let dout = seg_dir(i % m);
            let cos_half = din.dot(t).clamp(0.2, 1.0);
            let b = dout - din;
            let b = (b - t * b.dot(t)).normalize_or_zero();
            (b, 1.0 / cos_half - 1.0)
        } else {
            (DVec3::ZERO, 0.0)
        };
        let k = scales.get(i).copied().unwrap_or(1.0).max(1e-6);
        rings.push(
            prof.iter()
                .map(|p| {
                    let v = (x * p.x + y * p.y) * k;
                    path[i] + v + bend * (v.dot(bend) * stretch)
                })
                .collect::<Vec<_>>(),
        );
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

/// Helix path about the Z axis through `center`, for coils and springs.
pub fn helix_path(
    center: anvil_math::DVec3,
    radius: f64,
    pitch: f64,
    turns: f64,
    segments_per_turn: usize,
) -> Vec<anvil_math::DVec3> {
    let n = ((turns * segments_per_turn as f64).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let t = turns * i as f64 / n as f64;
            let a = t * std::f64::consts::TAU;
            center + anvil_math::DVec3::new(radius * a.cos(), radius * a.sin(), pitch * t)
        })
        .collect()
}

/// Build a solid from a closed triangle mesh (for STL import). Every
/// triangle becomes a face. Vertices closer than `weld` are merged.
pub fn from_triangles(tris: &[[anvil_math::DVec3; 3]], weld: f64) -> Solid {
    from_tagged_triangles(tris, None, weld)
}

/// As `from_triangles`, with a surface tag per triangle (faces get
/// `Surface::Revolved { id: tag }`), so exporters can group faces by
/// the part of a field they came from. Without tags every face gets 9.
pub fn from_tagged_triangles(tris: &[[anvil_math::DVec3; 3]], tags: Option<&[u32]>, weld: f64) -> Solid {
    use std::collections::HashMap;
    let mut s = Solid::new();
    let key = |p: anvil_math::DVec3| -> (i64, i64, i64) {
        ((p.x / weld).round() as i64, (p.y / weld).round() as i64, (p.z / weld).round() as i64)
    };
    let mut map: HashMap<(i64, i64, i64), VertexId> = HashMap::new();
    for (k, t) in tris.iter().enumerate() {
        let ids: Vec<VertexId> = t.iter().map(|&p| *map.entry(key(p)).or_insert_with(|| s.add_vertex(p))).collect();
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
            continue;
        }
        let id = tags.and_then(|t| t.get(k).copied()).unwrap_or(9);
        s.faces.insert(crate::topology::Face { outer: ids, inner: Vec::new(), surface: Surface::Revolved { id } });
    }
    // Edges for display: rebuild from faces.
    let faces: Vec<Vec<VertexId>> = s.faces.values().map(|f| f.outer.clone()).collect();
    let mut seen = std::collections::HashSet::new();
    for f in faces {
        for i in 0..f.len() {
            let a = f[i];
            let b = f[(i + 1) % f.len()];
            let k = if a < b { (a, b) } else { (b, a) };
            if seen.insert(k) {
                s.edges.insert(crate::topology::Edge { a, b });
            }
        }
    }
    s.make_consistent();
    s
}

/// Split a solid by a plane. Returns (below, above) along the plane normal.
/// Works on planar-facet solids: faces are clipped and each half is capped
/// with the intersection polygon(s).
pub fn split_by_plane(solid: &Solid, plane: &Plane) -> KernelResult<(Solid, Solid)> {
    use anvil_math::DVec3;
    let n = plane.normal();
    let dist = |p: DVec3| (p - plane.origin).dot(n);
    let mut halves = [Solid::new(), Solid::new()];
    let mut welds: [std::collections::HashMap<(i64, i64, i64), VertexId>; 2] = Default::default();
    let weld_key = |p: DVec3| ((p.x * 1e7).round() as i64, (p.y * 1e7).round() as i64, (p.z * 1e7).round() as i64);
    let mut any_cut = false;
    let eps = 1e-9 * (1.0 + solid.bounds().diagonal());
    // Faces with holes are split as their triangles; a hole loop cut by
    // the plane would otherwise need a merge with the outer loop.
    let tri =
        if solid.faces.values().any(|f| !f.inner.is_empty()) { Some(crate::mesh::tessellate(solid)) } else { None };
    let mut by_face: std::collections::HashMap<crate::FaceId, Vec<[DVec3; 3]>> = std::collections::HashMap::new();
    if let Some(m) = &tri {
        for (k, t) in m.indices.as_chunks::<3>().0.iter().enumerate() {
            if let Some(&fid) = m.face_of_tri.get(k) {
                if !solid.faces[fid].inner.is_empty() {
                    by_face.entry(fid).or_default().push([
                        m.positions[t[0] as usize],
                        m.positions[t[1] as usize],
                        m.positions[t[2] as usize],
                    ]);
                }
            }
        }
    }
    let mut split_loop = |pts: &[DVec3], surface: Surface| {
        let ds: Vec<f64> = pts.iter().map(|&p| dist(p)).collect();
        // A face lying in the plane goes to the half it closes.
        if ds.iter().all(|d| d.abs() <= eps) {
            let mut nv = DVec3::ZERO;
            for i in 0..pts.len() {
                let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
                nv += p.cross(q);
            }
            let side = if nv.dot(n) > 0.0 { 0 } else { 1 };
            let h = &mut halves[side];
            let w = &mut welds[side];
            let ids: Vec<VertexId> =
                pts.iter().map(|&p| *w.entry(weld_key(p)).or_insert_with(|| h.add_vertex(p))).collect();
            h.push_face(ids, surface);
            return;
        }
        if ds.iter().any(|d| *d > eps) && ds.iter().any(|d| *d < -eps) {
            any_cut = true;
        }
        for (side, keep_positive) in [(0usize, false), (1usize, true)] {
            let inside = |d: f64| if keep_positive { d >= -eps } else { d <= eps };
            let mut poly: Vec<DVec3> = Vec::new();
            for i in 0..pts.len() {
                let j = (i + 1) % pts.len();
                let (p, q) = (pts[i], pts[j]);
                let (dp, dq) = (ds[i], ds[j]);
                if inside(dp) {
                    poly.push(p);
                }
                if (dp > eps && dq < -eps) || (dp < -eps && dq > eps) {
                    // Same crossing point from both sides of the edge.
                    let (a, b, da, db) = if p.x < q.x || (p.x == q.x && (p.y < q.y || (p.y == q.y && p.z <= q.z))) {
                        (p, q, dp, dq)
                    } else {
                        (q, p, dq, dp)
                    };
                    let t = da / (da - db);
                    poly.push(a + (b - a) * t);
                }
            }
            // Drop repeated points (a vertex on the plane next to its crossing).
            poly.dedup_by(|a, b| weld_key(*a) == weld_key(*b));
            if poly.len() >= 3 && weld_key(poly[0]) == weld_key(*poly.last().unwrap()) {
                poly.pop();
            }
            if poly.len() >= 3 {
                let h = &mut halves[side];
                let w = &mut welds[side];
                let ids: Vec<VertexId> =
                    poly.iter().map(|&p| *w.entry(weld_key(p)).or_insert_with(|| h.add_vertex(p))).collect();
                h.push_face(ids, surface);
            }
        }
    };
    for (fid, f) in &solid.faces {
        if let Some(tris) = by_face.get(&fid) {
            for t in tris {
                split_loop(t, f.surface);
            }
        } else {
            let pts: Vec<DVec3> = f.outer.iter().map(|&v| solid.pos(v)).collect();
            split_loop(&pts, f.surface);
        }
    }
    if !any_cut {
        return Err(KernelError::InvalidInput("plane does not cut the body".into()));
    }
    // The cap of each half is exactly its open boundary: chain the edges
    // used once into loops, then nest holes inside the outer loops.
    let [mut below, mut above] = halves;
    for (h, outward) in [(&mut below, n), (&mut above, -n)] {
        cap_open_boundary(h, outward);
        h.surfaces = solid.surfaces.clone();
        h.rebuild_edges();
        h.make_consistent();
    }
    Ok((below, above))
}

/// Close every open boundary loop of `s` with a planar face. Loops are
/// chained by vertex id, so the result is exact. Loops that lie inside
/// another loop become holes of that face. `n` is the outward normal of
/// the cap.
fn cap_open_boundary(s: &mut Solid, n: anvil_math::DVec3) {
    use anvil_math::DVec3;
    use std::collections::HashMap;
    // Directed use counts per undirected edge: `fwd` counts a -> b for
    // the sorted pair (a, b). An edge is open when the total is odd; the
    // extra pair from a sliver at a boolean seam cancels out, and the
    // direction that wins is the one the side faces really use.
    let mut uses: HashMap<(VertexId, VertexId), (usize, usize)> = HashMap::new();
    for f in s.faces.values() {
        for lp in std::iter::once(&f.outer).chain(f.inner.iter()) {
            for i in 0..lp.len() {
                let (a, b) = (lp[i], lp[(i + 1) % lp.len()]);
                if a == b {
                    continue;
                }
                let e = uses.entry(if a < b { (a, b) } else { (b, a) }).or_insert((0, 0));
                if a < b {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
        }
    }
    // The cap runs opposite to the side faces along every open edge.
    let mut open: Vec<(VertexId, VertexId)> = uses
        .into_iter()
        .filter(|(_, (f, b))| (f + b) % 2 == 1)
        .map(|((a, b), (f, bw))| if f > bw { (b, a) } else { (a, b) })
        .collect();
    open.sort();
    // Connectivity is undirected, so a chain never dangles at a vertex
    // whose recorded direction is off. The direction only decides which
    // way to go where more than one edge is on offer.
    let dir_ok: std::collections::BTreeSet<(VertexId, VertexId)> = open.iter().copied().collect();
    let key = |a: VertexId, b: VertexId| if a < b { (a, b) } else { (b, a) };
    let mut adj: std::collections::BTreeMap<VertexId, Vec<VertexId>> = std::collections::BTreeMap::new();
    let mut unused: std::collections::BTreeSet<(VertexId, VertexId)> = std::collections::BTreeSet::new();
    for (a, b) in &open {
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
        unused.insert(key(*a, *b));
    }
    let mut left = unused.len();
    let mut loops: Vec<Vec<VertexId>> = Vec::new();
    for &(start, first) in &open {
        if !unused.remove(&key(start, first)) {
            continue;
        }
        left -= 1;
        let mut lp = vec![start, first];
        let mut prev = start;
        let mut cur = first;
        loop {
            let all: Vec<VertexId> = adj
                .get(&cur)
                .map(|v| v.iter().copied().filter(|&c| unused.contains(&key(cur, c))).collect())
                .unwrap_or_default();
            if all.is_empty() {
                break;
            }
            let forward: Vec<VertexId> = all.iter().copied().filter(|&c| dir_ok.contains(&(cur, c))).collect();
            let cands = if forward.is_empty() { all } else { forward };
            // At a vertex where two loops touch, take the sharpest left
            // turn seen from outside: the cap runs counter-clockwise there,
            // so the region stays on the left and the loops never cross.
            let nx = if cands.len() == 1 {
                cands[0]
            } else {
                let d_in = (s.pos(cur) - s.pos(prev)).normalize_or_zero();
                let mut best = (cands[0], f64::NEG_INFINITY);
                for &c in &cands {
                    let d_out = (s.pos(c) - s.pos(cur)).normalize_or_zero();
                    let turn = d_in.cross(d_out).dot(n).atan2(d_in.dot(d_out));
                    if turn > best.1 {
                        best = (c, turn);
                    }
                }
                best.0
            };
            unused.remove(&key(cur, nx));
            left -= 1;
            if nx == start {
                break;
            }
            lp.push(nx);
            prev = cur;
            cur = nx;
            if lp.len() > 1_000_000 {
                break;
            }
        }
        if lp.len() >= 3 {
            loops.push(lp);
        }
    }
    if std::env::var("ANVIL_SPLIT_DEBUG").is_ok() {
        let sizes: Vec<usize> = loops.iter().map(|l| l.len()).collect();
        eprintln!("cap: {} loops {:?}, {} edges left over", loops.len(), sizes, left);
        for lp in &loops {
            let near = |cx: f64, cz: f64| {
                lp.iter()
                    .filter(|&&v| {
                        let p = s.pos(v);
                        ((p.x - cx).powi(2) + (p.z - cz).powi(2)).sqrt() < 2.4
                    })
                    .count()
            };
            let degree: Vec<usize> = lp.iter().map(|v| adj.get(v).map(|l| l.len()).unwrap_or(0)).collect();
            let branches = degree.iter().filter(|&&d| d > 2).count();
            // Compare the loop's own area with its triangulated area.
            let helper = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
            let u = n.cross(helper).normalize();
            let v = n.cross(u);
            let k = lp.len();
            let shoelace: f64 = (0..k)
                .map(|i| {
                    let p = s.pos(lp[i]);
                    let q = s.pos(lp[(i + 1) % k]);
                    p.dot(u) * q.dot(v) - q.dot(u) * p.dot(v)
                })
                .sum::<f64>()
                * 0.5;
            let mut probe = Solid::new();
            let ids: Vec<VertexId> = lp.iter().map(|&x| probe.add_vertex(s.pos(x))).collect();
            probe.push_face(ids, Surface::Plane);
            let m = crate::mesh::tessellate(&probe);
            let tri_area: f64 = m
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| {
                    let (a, b, c) =
                        (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
                    (b - a).cross(c - a).length() * 0.5
                })
                .sum();
            eprintln!("  loop area {shoelace:.1} vs triangulated {tri_area:.1}, {} triangles", m.indices.len() / 3);
            eprintln!(
                "  loop of {}: {} vertices near lug hole +x, {} near lug hole -x, {} branch vertices",
                lp.len(),
                near(60.0, 80.0),
                near(-60.0, 80.0),
                branches
            );
        }
    }
    if loops.is_empty() {
        return;
    }
    // Signed area in the cap plane: positive loops are outers.
    let helper = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = n.cross(helper).normalize();
    let v = n.cross(u);
    let to2 = |p: DVec3| (p.dot(u), p.dot(v));
    let area = |lp: &Vec<VertexId>| -> f64 {
        let k = lp.len();
        (0..k)
            .map(|i| {
                let (x0, y0) = to2(s.pos(lp[i]));
                let (x1, y1) = to2(s.pos(lp[(i + 1) % k]));
                x0 * y1 - x1 * y0
            })
            .sum::<f64>()
            * 0.5
    };
    let inside = |pt: (f64, f64), lp: &Vec<VertexId>| -> bool {
        let k = lp.len();
        let mut c = false;
        for i in 0..k {
            let (x0, y0) = to2(s.pos(lp[i]));
            let (x1, y1) = to2(s.pos(lp[(i + 1) % k]));
            if (y0 > pt.1) != (y1 > pt.1) && pt.0 < (x1 - x0) * (pt.1 - y0) / (y1 - y0) + x0 {
                c = !c;
            }
        }
        c
    };
    let areas: Vec<f64> = loops.iter().map(area).collect();
    // The sign convention depends on which side the cap faces; take the
    // larger total as the outer sign.
    let pos: f64 = areas.iter().filter(|a| **a > 0.0).sum();
    let neg: f64 = areas.iter().filter(|a| **a < 0.0).map(|a| -a).sum();
    let outer_sign = if pos >= neg { 1.0 } else { -1.0 };
    let mut outers: Vec<(usize, f64)> =
        areas.iter().enumerate().filter(|(_, a)| **a * outer_sign > 0.0).map(|(i, a)| (i, a.abs())).collect();
    outers.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); loops.len()];
    for (i, a) in areas.iter().enumerate() {
        if *a * outer_sign >= 0.0 {
            continue;
        }
        let pt = to2(s.pos(loops[i][0]));
        // Smallest outer that contains the hole.
        if let Some((o, _)) = outers.iter().find(|(o, _)| inside(pt, &loops[*o])) {
            holes[*o].push(i);
        }
    }
    let mut faces: Vec<(Vec<VertexId>, Vec<Vec<VertexId>>)> = Vec::new();
    for (o, _) in &outers {
        let inner: Vec<Vec<VertexId>> = holes[*o].iter().map(|&h| loops[h].clone()).collect();
        faces.push((loops[*o].clone(), inner));
    }
    for (outer, inner) in faces {
        s.faces.insert(crate::topology::Face { outer, inner, surface: Surface::Plane });
    }
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

    #[test]
    fn split_cube_gives_two_halves() {
        let b = box_solid(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)).unwrap();
        let plane = Plane { origin: DVec3::new(0.0, 0.0, 4.0), ..Plane::XY };
        let (lo, hi) = split_by_plane(&b, &plane).unwrap();
        assert!((lo.volume() - 400.0).abs() < 1e-6, "{}", lo.volume());
        assert!((hi.volume() - 600.0).abs() < 1e-6, "{}", hi.volume());
        assert_eq!(lo.euler_characteristic(), 2);
    }

    #[test]
    fn triangles_round_trip_to_solid() {
        let b = box_solid(DVec3::ZERO, DVec3::new(2.0, 2.0, 2.0)).unwrap();
        let m = crate::mesh::tessellate(&b);
        let tris: Vec<[DVec3; 3]> = m
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]])
            .collect();
        let s = from_triangles(&tris, 1e-6);
        assert!((s.volume() - 8.0).abs() < 1e-9, "{}", s.volume());
    }

    #[test]
    fn helix_sweep_makes_a_coil() {
        let path = helix_path(DVec3::ZERO, 10.0, 4.0, 3.0, 24);
        let prof: Vec<DVec2> = (0..32)
            .map(|i| {
                let t = i as f64 / 32.0 * std::f64::consts::TAU;
                DVec2::new(t.cos(), t.sin())
            })
            .collect();
        let plane = Plane { origin: path[0], x_axis: DVec3::X, y_axis: DVec3::Z };
        let s = sweep(&plane, &prof, &path, false).unwrap();
        let length = 3.0 * (std::f64::consts::TAU * 10.0f64).hypot(4.0);
        let exact = length * std::f64::consts::PI;
        assert!((s.volume() - exact).abs() / exact < 0.05, "{} vs {}", s.volume(), exact);
    }
    #[test]
    fn extrude_with_hole_volume() {
        let outer = square(10.0);
        let hole = vec![DVec2::new(3.0, 3.0), DVec2::new(7.0, 3.0), DVec2::new(7.0, 7.0), DVec2::new(3.0, 7.0)];
        let s = extrude_with_holes(&Plane::XY, &outer, &[hole], 2.0).unwrap();
        assert!((s.volume() - 168.0).abs() < 1e-6, "{}", s.volume());
        assert_eq!(s.faces.len(), 10);
    }
}
