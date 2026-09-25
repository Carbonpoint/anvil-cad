//! Meshing one curved STEP face.
//!
//! The face's boundary loops are carried into the surface's parameter
//! plane (u, v) and triangulated there. Three things make that harder
//! than on a plane:
//!
//! * **Angles wrap.** On a cylinder u runs round and round. A loop's u
//!   values are unwrapped so that it does not jump by a full turn.
//! * **Loops can go all the way round.** A cylinder face with no seam edge
//!   is bounded by two loops that each circle the axis. In (u, v) they are
//!   two open lines; they are joined into one ring along a cut. A sphere
//!   cap has one such loop, closed through the pole.
//! * **Poles.** At the top of a sphere every u meets in one point. A loop
//!   point at a pole is split in two, one for each meridian it touches.
//!
//! Boundary points keep their exact positions in space, so the faces
//! around this one weld to it. Points added inside the face come from the
//! surface itself.

use super::step::Bound;
use super::surface::Surf;
use super::tess;
use anvil_math::{DVec2, DVec3};

/// Chord tolerance for points inside a face, in millimetres.
const TOL: f64 = 0.01;

/// Most inner points one face may get.
const MAX_INNER: f64 = 4000.0;

/// One ring in the parameter plane, with the space point of each vertex.
struct Ring {
    uv: Vec<DVec2>,
    xyz: Vec<DVec3>,
}

/// Triangles of the face, oriented so their normal follows the face
/// normal: the surface normal when `sense` is true.
pub(crate) fn mesh_face(surf: &Surf, loops: &[Bound], sense: bool) -> Option<Vec<[DVec3; 3]>> {
    let (pu, pv) = surf.periods();
    let mut closed: Vec<(Ring, bool)> = Vec::new();
    let mut wrapping: Vec<(Ring, usize, i64)> = Vec::new();
    for b in loops {
        let ring = to_param(surf, &b.pts)?;
        let (wu, wv) = (winding(&ring, 0, pu), winding(&ring, 1, pv));
        match (wu, wv) {
            (0, 0) => closed.push((ring, b.outer)),
            (w, 0) if w.abs() == 1 => wrapping.push((ring, 0, w)),
            (0, w) if w.abs() == 1 => wrapping.push((ring, 1, w)),
            _ => return None,
        }
    }
    // Join loops that go all the way round.
    match wrapping.len() {
        0 => {}
        1 => {
            let (ring, axis, w) = wrapping.pop()?;
            closed.push((cap(surf, ring, axis, w, sense)?, true));
        }
        2 => {
            let (b, axis_b, wb) = wrapping.pop()?;
            let (a, axis_a, wa) = wrapping.pop()?;
            if axis_a != axis_b || wa != -wb {
                return None;
            }
            let period = if axis_a == 0 { pu? } else { pv? };
            let (a, b) = if wa > 0 { (a, b) } else { (b, a) };
            closed.push((band(a, b, axis_a, period), true));
        }
        _ => return None,
    }
    // The outer ring: the one the file marks, else the largest.
    let marked: Vec<usize> = (0..closed.len()).filter(|&i| closed[i].1).collect();
    let k = if marked.len() == 1 {
        marked[0]
    } else {
        (0..closed.len()).max_by(|&a, &b| area(&closed[a].0.uv).abs().total_cmp(&area(&closed[b].0.uv).abs()))?
    };
    let outer = closed.remove(k).0;
    let mut holes: Vec<Ring> = closed.into_iter().map(|(r, _)| r).collect();
    // Move each hole by whole turns to sit over the outer ring.
    let oc = centroid(&outer.uv);
    for h in &mut holes {
        let hc = centroid(&h.uv);
        let shift = DVec2::new(
            pu.map_or(0.0, |p| ((oc.x - hc.x) / p).round() * p),
            pv.map_or(0.0, |p| ((oc.y - hc.y) / p).round() * p),
        );
        for q in &mut h.uv {
            *q += shift;
        }
    }
    // A plane where distances are fair, for the Delaunay test.
    let (su, sv) = surf.metric((oc.x, oc.y));
    if !(su.is_finite() && sv.is_finite() && su > 0.0 && sv > 0.0) {
        return None;
    }
    let scale = |q: DVec2| DVec2::new(q.x * su, q.y * sv);
    let mut pts: Vec<DVec2> = Vec::new();
    let mut xyz: Vec<DVec3> = Vec::new();
    let mut uv: Vec<DVec2> = Vec::new();
    let mut add_ring = |r: &Ring, pts: &mut Vec<DVec2>| -> Vec<usize> {
        let mut idx = Vec::new();
        for (q, p) in r.uv.iter().zip(&r.xyz) {
            let s = scale(*q);
            if idx.last().is_some_and(|&l: &usize| (pts[l] - s).length() < 1e-9) {
                continue;
            }
            idx.push(pts.len());
            pts.push(s);
            xyz.push(*p);
            uv.push(*q);
        }
        while idx.len() > 1 && (pts[idx[0]] - pts[idx[idx.len() - 1]]).length() < 1e-9 {
            idx.pop();
            pts.pop();
            xyz.pop();
            uv.pop();
        }
        idx
    };
    let outer_idx = add_ring(&outer, &mut pts);
    let outer_n = outer_idx.len();
    let hole_idx: Vec<Vec<usize>> = holes.iter().map(|h| add_ring(h, &mut pts)).filter(|h| h.len() >= 3).collect();
    if outer_n < 3 {
        return None;
    }
    // Points inside, on surfaces that bend both ways.
    let mut inner: Vec<usize> = Vec::new();
    if surf.doubly_curved() {
        let rings: Vec<Vec<DVec2>> =
            std::iter::once(&outer_idx).chain(hole_idx.iter()).map(|r| r.iter().map(|&i| pts[i]).collect()).collect();
        let region = (area(&rings[0]).abs() - rings[1..].iter().map(|r| area(r).abs()).sum::<f64>()).max(0.0);
        let mut h = spacing(surf);
        if region / (h * h) > MAX_INNER {
            h = (region / MAX_INNER).sqrt();
        }
        let (lo, hi) =
            rings[0].iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
        let mut y = lo.y + h * 0.5;
        let mut row = 0;
        while y < hi.y {
            let mut x = lo.x + h * if row % 2 == 0 { 0.5 } else { 1.0 };
            while x < hi.x {
                let q = DVec2::new(x, y);
                let inside = rings.iter().filter(|r| contains(r, q)).count() % 2 == 1;
                if inside && rings.iter().all(|r| clear_of(r, q, 0.4 * h)) {
                    let (u, v) = (x / su, y / sv);
                    inner.push(pts.len());
                    pts.push(q);
                    xyz.push(surf.eval(u, v));
                    uv.push(DVec2::new(u, v));
                }
                x += h;
            }
            y += h * 0.866;
            row += 1;
        }
    }
    let tris = tess::triangulate(&pts, outer_n, &hole_idx, &inner)?;
    // Counter-clockwise in (u, v) gives the surface normal in space.
    let out: Vec<[DVec3; 3]> = tris
        .iter()
        .map(|t| if sense { [xyz[t[0]], xyz[t[1]], xyz[t[2]]] } else { [xyz[t[0]], xyz[t[2]], xyz[t[1]]] })
        .filter(|t| t.iter().all(|p| p.is_finite()))
        .collect();
    (!out.is_empty()).then_some(out)
}

/// Carry a loop into (u, v), unwrapping angles and splitting pole points.
fn to_param(surf: &Surf, pts: &[DVec3]) -> Option<Ring> {
    let (pu, pv) = surf.periods();
    let poles: Vec<(f64, DVec3)> = surf.poles().into_iter().map(|v| (v, surf.eval(0.0, v))).collect();
    let pole_of = |p: DVec3| poles.iter().find(|(_, q)| (p - *q).length() < 1e-6).map(|(v, _)| *v);
    let raw: Vec<(DVec2, Option<f64>)> = pts
        .iter()
        .map(|&p| {
            let (u, v) = surf.inverse(p);
            (DVec2::new(u, v), pole_of(p))
        })
        .collect();
    let n = raw.len();
    if raw.iter().all(|(_, pole)| pole.is_some()) || n < 2 {
        return None;
    }
    let mut ring = Ring { uv: Vec::new(), xyz: Vec::new() };
    for i in 0..n {
        match raw[i].1 {
            None => {
                ring.uv.push(raw[i].0);
                ring.xyz.push(pts[i]);
            }
            Some(vp) => {
                // The meridians in and out of the pole.
                let prev = (1..n).map(|k| &raw[(i + n - k) % n]).find(|r| r.1.is_none())?.0.x;
                let next = (1..n).map(|k| &raw[(i + k) % n]).find(|r| r.1.is_none())?.0.x;
                ring.uv.push(DVec2::new(prev, vp));
                ring.xyz.push(pts[i]);
                ring.uv.push(DVec2::new(next, vp));
                ring.xyz.push(pts[i]);
            }
        }
    }
    for i in 1..ring.uv.len() {
        let prev = ring.uv[i - 1];
        let q = &mut ring.uv[i];
        if let Some(p) = pu {
            q.x = prev.x + wrap(q.x - prev.x, p);
        }
        if let Some(p) = pv {
            q.y = prev.y + wrap(q.y - prev.y, p);
        }
    }
    Some(ring)
}

/// `d` moved by whole periods into (-p/2, p/2].
fn wrap(d: f64, p: f64) -> f64 {
    d - p * (d / p).round()
}

/// Whole turns a closed ring makes along `axis` (0 is u, 1 is v).
fn winding(r: &Ring, axis: usize, period: Option<f64>) -> i64 {
    let Some(p) = period else { return 0 };
    let c = |q: DVec2| if axis == 0 { q.x } else { q.y };
    let (first, last) = (c(r.uv[0]), c(r.uv[r.uv.len() - 1]));
    let total = last - first + wrap(first - last, p);
    (total / p).round() as i64
}

/// Two loops that go round in opposite directions, joined into one ring
/// along a cut. `a` goes forward along `axis`, `b` backward.
fn band(a: Ring, b: Ring, axis: usize, period: f64) -> Ring {
    let c = |q: DVec2| if axis == 0 { q.x } else { q.y };
    let step = |q: DVec2, k: f64| if axis == 0 { q + DVec2::new(k, 0.0) } else { q + DVec2::new(0.0, k) };
    let a0 = a.uv[0];
    // Start b at its point nearest the cut, one turn ahead of a's start.
    let cut = c(a0) + period;
    let bi = (0..b.uv.len())
        .min_by(|&i, &j| wrap(c(b.uv[i]) - cut, period).abs().total_cmp(&wrap(c(b.uv[j]) - cut, period).abs()))
        .unwrap_or(0);
    let n = b.uv.len();
    let mut ring = Ring { uv: a.uv.clone(), xyz: a.xyz.clone() };
    ring.uv.push(step(a0, period));
    ring.xyz.push(a.xyz[0]);
    // Unwrap b from its new start, moved so that start sits at the cut.
    let b0 = b.uv[bi];
    let lift = cut + wrap(c(b0) - cut, period) - c(b0);
    let mut prev = step(b0, lift);
    for k in 0..n {
        let i = (bi + k) % n;
        let mut q = step(b.uv[i], lift);
        if k > 0 {
            let d = wrap(c(q) - c(prev), period);
            q = step(q, c(prev) + d - c(q));
        }
        ring.uv.push(q);
        ring.xyz.push(b.xyz[i]);
        prev = q;
    }
    ring.uv.push(step(step(b0, lift), -period));
    ring.xyz.push(b.xyz[bi]);
    ring
}

/// One loop that goes round, closed through the pole on the face's side.
fn cap(surf: &Surf, r: Ring, axis: usize, w: i64, sense: bool) -> Option<Ring> {
    if axis != 0 {
        return None;
    }
    let mean_v = r.uv.iter().map(|q| q.y).sum::<f64>() / r.uv.len() as f64;
    // The face lies left of its loop when looked at from its normal, and
    // counter-clockwise in (u, v) looks from the surface normal.
    let above = (w > 0) == sense;
    let vp = surf.poles().into_iter().find(|&v| (v > mean_v) == above)?;
    let a0 = r.uv[0];
    let end = a0.x + w as f64 * std::f64::consts::TAU;
    let mut ring = r;
    ring.uv.push(DVec2::new(end, a0.y));
    ring.xyz.push(ring.xyz[0]);
    let pole = surf.eval(0.0, vp);
    ring.uv.push(DVec2::new(end, vp));
    ring.xyz.push(pole);
    ring.uv.push(DVec2::new(a0.x, vp));
    ring.xyz.push(pole);
    Some(ring)
}

/// Spacing of inner points in millimetres: the chord of an arc of the
/// surface's tighter radius stays within `TOL`.
fn spacing(surf: &Surf) -> f64 {
    let chord = |r: f64| {
        let r = r.abs().max(TOL * 2.0);
        r * 2.0 * (1.0 - TOL / r).clamp(-1.0, 1.0).acos()
    };
    match surf {
        Surf::Sphere { r, .. } => chord(*r),
        Surf::Torus { r, big, .. } => chord(r.min(*big)),
        _ => {
            // No simple radius: sample the surface's size instead.
            let (a, b) = (surf.eval(0.0, 0.0), surf.eval(1.0, 1.0));
            ((a - b).length() / 24.0).clamp(0.05, 50.0)
        }
    }
}

fn area(r: &[DVec2]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i].perp_dot(r[(i + 1) % n])).sum::<f64>() * 0.5
}

fn centroid(r: &[DVec2]) -> DVec2 {
    r.iter().copied().sum::<DVec2>() / r.len().max(1) as f64
}

/// Even-odd point in polygon.
fn contains(r: &[DVec2], q: DVec2) -> bool {
    let n = r.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// True when `q` is farther than `d` from every edge of the ring.
fn clear_of(r: &[DVec2], q: DVec2, d: f64) -> bool {
    let n = r.len();
    (0..n).all(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (a + ab * t - q).length() > d
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::surface::Frame;
    use std::f64::consts::TAU;

    fn circle(z: f64, r: f64, n: usize, forward: bool) -> Vec<DVec3> {
        let mut v: Vec<DVec3> = (0..n)
            .map(|k| {
                let a = TAU * k as f64 / n as f64;
                DVec3::new(r * a.cos(), r * a.sin(), z)
            })
            .collect();
        if !forward {
            v.reverse();
        }
        v
    }

    fn signed_volume(tris: &[[DVec3; 3]]) -> f64 {
        tris.iter().map(|t| t[0].dot(t[1].cross(t[2])) / 6.0).sum()
    }

    #[test]
    fn a_cylinder_band_without_a_seam_meshes_round() {
        let f = Frame { o: DVec3::ZERO, z: DVec3::Z, x: DVec3::X };
        let surf = Surf::Cylinder { f, r: 5.0 };
        // Face to the left seen from outside: bottom forward, top back.
        let loops = [
            Bound { pts: circle(0.0, 5.0, 48, true), outer: false },
            Bound { pts: circle(10.0, 5.0, 48, false), outer: false },
        ];
        let tris = mesh_face(&surf, &loops, true).expect("the band meshes");
        assert_eq!(tris.len(), 96, "one strip of two triangles per step");
        for t in &tris {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let c = (t[0] + t[1] + t[2]) / 3.0;
            assert!(n.dot(DVec3::new(c.x, c.y, 0.0)) > 0.0, "a triangle faces inward");
            for p in t {
                assert!(((p.x * p.x + p.y * p.y).sqrt() - 5.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn a_hemisphere_closes_through_its_pole() {
        let f = Frame { o: DVec3::ZERO, z: DVec3::Z, x: DVec3::X };
        let surf = Surf::Sphere { f, r: 10.0 };
        // The equator, going round +u, face on its left (above) from outside.
        let loops = [Bound { pts: circle(0.0, 10.0, 64, true), outer: true }];
        let tris = mesh_face(&surf, &loops, true).expect("the cap meshes");
        // With the flat disc under it, the volume is half a sphere.
        let mut all = tris.clone();
        let disc: Vec<DVec3> = circle(0.0, 10.0, 64, false);
        for i in 1..disc.len() - 1 {
            all.push([disc[0], disc[i], disc[i + 1]]);
        }
        let v = signed_volume(&all);
        let want = 2.0 / 3.0 * std::f64::consts::PI * 1000.0;
        assert!((v - want).abs() < 0.02 * want, "volume {v}, want {want}");
        assert!(tris.iter().flatten().all(|p| (p.length() - 10.0).abs() < 1e-6));
    }
}
