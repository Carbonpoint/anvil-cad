//! Triangulation of a region in a surface's parameter plane.
//!
//! The region is an outer loop with holes. The steps:
//! 1. Ear clipping gives a first triangulation of the loops.
//! 2. Extra inner points, when asked for, split the triangle they fall in.
//! 3. Edge flips make the result a constrained Delaunay triangulation:
//!    no loop edge is flipped, and every other edge is flipped until no
//!    point lies inside the circumcircle of a neighbouring triangle.
//!
//! Step 3 matters on a curved surface. Ear clipping likes long thin
//! triangles, and a long triangle on a cylinder cuts through the inside
//! instead of following the wall. Delaunay triangles join near points.

use anvil_math::DVec2;
use std::collections::{HashMap, HashSet};

/// Triangles of the region, as indices into `pts`, counter-clockwise.
/// `pts[..outer_n]` is the outer loop in order, then each hole in `holes`
/// as index lists; points after the loops are extra inner points. Returns
/// `None` when the loops do not make a usable region.
pub fn triangulate(pts: &[DVec2], outer_n: usize, holes: &[Vec<usize>], inner: &[usize]) -> Option<Vec<[usize; 3]>> {
    if outer_n < 3 || pts.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let flat = anvil_kernel::mesh::ear_clip_with_holes(pts, outer_n, holes);
    let mut tris: Vec<[usize; 3]> = flat.as_chunks::<3>().0.to_vec();
    for t in &mut tris {
        if orient(pts[t[0]], pts[t[1]], pts[t[2]]) < 0.0 {
            t.swap(1, 2);
        }
    }
    // The triangles must cover the region: outer area minus holes.
    let loop_area = |idx: &[usize]| -> f64 {
        let n = idx.len();
        (0..n).map(|i| pts[idx[i]].perp_dot(pts[idx[(i + 1) % n]])).sum::<f64>().abs() * 0.5
    };
    let outer: Vec<usize> = (0..outer_n).collect();
    let want = loop_area(&outer) - holes.iter().map(|h| loop_area(h)).sum::<f64>();
    let got: f64 = tris.iter().map(|t| orient(pts[t[0]], pts[t[1]], pts[t[2]]) * 0.5).sum();
    if want <= 0.0 || (got - want).abs() > 1e-3 * want {
        return None;
    }
    let mut fixed: HashSet<(usize, usize)> = HashSet::new();
    for lp in std::iter::once(&outer).chain(holes.iter()) {
        for i in 0..lp.len() {
            fixed.insert(key(lp[i], lp[(i + 1) % lp.len()]));
        }
    }
    for &p in inner {
        insert(pts, &mut tris, &fixed, p);
    }
    delaunay_flips(pts, &mut tris, &fixed);
    Some(tris)
}

/// Put point `p` into the triangulation: split the triangle it is in, or
/// the two triangles on each side of the edge it lies on. A point on a
/// loop edge is left out.
fn insert(pts: &[DVec2], tris: &mut Vec<[usize; 3]>, fixed: &HashSet<(usize, usize)>, p: usize) {
    let q = pts[p];
    for k in 0..tris.len() {
        let t = tris[k];
        let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        let s = orient(a, b, c);
        if s <= 0.0 {
            continue;
        }
        let eps = 1e-9 * s;
        let o = [orient(a, b, q), orient(b, c, q), orient(c, a, q)];
        if o.iter().any(|&x| x < -eps) {
            continue;
        }
        let on: Vec<usize> = (0..3).filter(|&i| o[i].abs() <= eps).collect();
        match on.as_slice() {
            [] => {
                tris[k] = [t[0], t[1], p];
                tris.push([t[1], t[2], p]);
                tris.push([t[2], t[0], p]);
            }
            [i] => {
                let (x, y, z) = (t[*i], t[(i + 1) % 3], t[(i + 2) % 3]);
                if fixed.contains(&key(x, y)) {
                    return;
                }
                // The neighbour across x-y has the edge as y, x.
                let Some(m) =
                    (0..tris.len()).find(|&m| m != k && (0..3).any(|j| tris[m][j] == y && tris[m][(j + 1) % 3] == x))
                else {
                    return;
                };
                let tm = tris[m];
                let j = (0..3).find(|&j| tm[j] == y).unwrap_or(0);
                let w = tm[(j + 2) % 3];
                tris[k] = [x, p, z];
                tris.push([p, y, z]);
                tris[m] = [y, p, w];
                tris.push([p, x, w]);
            }
            // On a corner: the point is already there.
            _ => {}
        }
        return;
    }
}

fn key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Twice the signed area of a, b, c: positive when counter-clockwise.
fn orient(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b - a).perp_dot(c - a)
}

/// True when `d` lies inside the circumcircle of the counter-clockwise
/// triangle a, b, c.
fn in_circle(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let (ax, ay) = (a.x - d.x, a.y - d.y);
    let (bx, by) = (b.x - d.x, b.y - d.y);
    let (cx, cy) = (c.x - d.x, c.y - d.y);
    let det = (ax * ax + ay * ay) * (bx * cy - cx * by) - (bx * bx + by * by) * (ax * cy - cx * ay)
        + (cx * cx + cy * cy) * (ax * by - bx * ay);
    let scale = (ax * ax + ay * ay + bx * bx + by * by + cx * cx + cy * cy).powi(2).max(1e-300);
    det > 1e-12 * scale
}

/// Flip edges until the triangulation is Delaunay, leaving `fixed` edges.
fn delaunay_flips(pts: &[DVec2], tris: &mut [[usize; 3]], fixed: &HashSet<(usize, usize)>) {
    // Each pass flips every edge that fails, skipping triangles already
    // changed in that pass. The number of flips is finite, so this ends;
    // the pass limit only guards against rounding loops.
    for _ in 0..200 {
        let mut edges: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
        for (ti, t) in tris.iter().enumerate() {
            for k in 0..3 {
                edges.entry(key(t[k], t[(k + 1) % 3])).or_default().push((ti, t[(k + 2) % 3]));
            }
        }
        let mut touched = vec![false; tris.len()];
        let mut flipped = 0;
        let mut list: Vec<_> = edges.into_iter().filter(|(e, v)| v.len() == 2 && !fixed.contains(e)).collect();
        list.sort_by_key(|(e, _)| *e);
        for ((a, b), v) in list {
            let ((t1, c), (t2, d)) = (v[0], v[1]);
            if touched[t1] || touched[t2] {
                continue;
            }
            // Orient so that t1 = (a, b, c) is counter-clockwise.
            let (a, b) = if orient(pts[a], pts[b], pts[c]) > 0.0 { (a, b) } else { (b, a) };
            if !in_circle(pts[a], pts[b], pts[c], pts[d]) {
                continue;
            }
            // Flip only a convex quad: the new diagonal c-d must cross a-b.
            if orient(pts[c], pts[d], pts[a]) >= 0.0 || orient(pts[c], pts[d], pts[b]) <= 0.0 {
                continue;
            }
            tris[t1] = [a, d, c];
            tris[t2] = [d, b, c];
            touched[t1] = true;
            touched[t2] = true;
            flipped += 1;
        }
        if flipped == 0 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_strip_joins_near_points() {
        // Two dense rows, like the two circles of a cylinder unrolled.
        // Every triangle must span one step along the strip, not many.
        let n = 20;
        let mut pts: Vec<DVec2> = (0..=n).map(|i| DVec2::new(i as f64, 0.0)).collect();
        pts.extend((0..=n).rev().map(|i| DVec2::new(i as f64, 3.0)));
        let tris = triangulate(&pts, pts.len(), &[], &[]).unwrap();
        assert_eq!(tris.len(), 2 * n);
        for t in &tris {
            let xs: Vec<f64> = t.iter().map(|&i| pts[i].x).collect();
            let span = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
            assert!(span <= 1.0 + 1e-9, "a triangle spans {span}: {t:?}");
        }
    }

    #[test]
    fn inner_points_are_used() {
        let pts = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 4.0),
            DVec2::new(0.0, 4.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(3.0, 1.0),
            DVec2::new(2.0, 3.0),
        ];
        let tris = triangulate(&pts, 4, &[], &[4, 5, 6]).unwrap();
        for p in 4..7 {
            assert!(tris.iter().any(|t| t.contains(&p)), "point {p} unused");
        }
        let area: f64 = tris.iter().map(|t| orient(pts[t[0]], pts[t[1]], pts[t[2]]) * 0.5).sum();
        assert!((area - 16.0).abs() < 1e-9);
    }
}
