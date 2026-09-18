//! Slices straight from a field: closed contours of the zero set on a
//! plane z = const by marching squares, with the crossings bisected on
//! the field, chained into loops. A printer wants layers; a field gives
//! them without a mesh in between, at any layer height, exactly.

use crate::Field;
use anvil_math::{DVec2, DVec3};

/// Closed contours of `field = 0` on the plane `z`, inside the box
/// `lo..hi` (x and y), sampled every `step`. Each loop is a list of
/// points in mm; the material is on the left of the walking direction.
pub fn contours(field: &dyn Field, z: f64, lo: DVec2, hi: DVec2, step: f64) -> Vec<Vec<DVec2>> {
    let nx = ((hi.x - lo.x) / step).ceil().max(1.0) as usize;
    let ny = ((hi.y - lo.y) / step).ceil().max(1.0) as usize;
    let cx = nx + 1;
    let cy = ny + 1;
    let pos = |i: usize, j: usize| DVec2::new(lo.x + i as f64 * step, lo.y + j as f64 * step);
    let at = |p: DVec2| field.at(DVec3::new(p.x, p.y, z));
    let mut v = vec![0.0f64; cx * cy];
    for j in 0..cy {
        for i in 0..cx {
            let d = at(pos(i, j));
            v[j * cx + i] = if d == 0.0 { f64::MIN_POSITIVE } else { d };
        }
    }
    // Crossing point on the edge between two corners, bisected.
    let cross = |a: DVec2, va: f64, b: DVec2, vb: f64| -> DVec2 {
        let mut pa = a;
        let mut pb = b;
        let mut fa = va;
        let t = va / (va - vb);
        let lin = a + (b - a) * t;
        if at(lin).abs() < 0.01 * step {
            return lin;
        }
        for _ in 0..8 {
            let m = (pa + pb) * 0.5;
            let fm = at(m);
            if (fm < 0.0) == (fa < 0.0) {
                pa = m;
                fa = fm;
            } else {
                pb = m;
            }
        }
        (pa + pb) * 0.5
    };
    // Edge keys: (i, j, 0) is the edge from corner (i, j) to (i + 1, j),
    // (i, j, 1) from (i, j) to (i, j + 1). Segments join two edge keys and
    // carry their end points.
    type EdgeKey = (usize, usize, u8);
    let mut segments: Vec<(EdgeKey, DVec2, EdgeKey, DVec2)> = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let val = [v[j * cx + i], v[j * cx + i + 1], v[(j + 1) * cx + i + 1], v[(j + 1) * cx + i]];
            let inside = [val[0] < 0.0, val[1] < 0.0, val[2] < 0.0, val[3] < 0.0];
            let code = inside.iter().enumerate().fold(0u8, |acc, (k, &b)| acc | ((b as u8) << k));
            if code == 0 || code == 15 {
                continue;
            }
            // Edges of the cell in order: bottom, right, top, left.
            let edge_key = |e: usize| -> EdgeKey {
                match e {
                    0 => (i, j, 0),
                    1 => (i + 1, j, 1),
                    2 => (i, j + 1, 0),
                    _ => (i, j, 1),
                }
            };
            let edge_point = |e: usize| -> DVec2 {
                let (a, b) = (e, (e + 1) % 4);
                cross(pos(c[a].0, c[a].1), val[a], pos(c[b].0, c[b].1), val[b])
            };
            // Segment pairs by case, oriented with the inside on the left:
            // walking from the first edge to the second keeps material to
            // the left.
            let pairs: Vec<(usize, usize)> = match code {
                1 => vec![(3, 0)],
                2 => vec![(0, 1)],
                3 => vec![(3, 1)],
                4 => vec![(1, 2)],
                5 => {
                    let centre = at((pos(i, j) + pos(i + 1, j + 1)) * 0.5) < 0.0;
                    if centre {
                        vec![(3, 2), (1, 0)]
                    } else {
                        vec![(3, 0), (1, 2)]
                    }
                }
                6 => vec![(0, 2)],
                7 => vec![(3, 2)],
                8 => vec![(2, 3)],
                9 => vec![(2, 0)],
                10 => {
                    let centre = at((pos(i, j) + pos(i + 1, j + 1)) * 0.5) < 0.0;
                    if centre {
                        vec![(0, 3), (2, 1)]
                    } else {
                        vec![(0, 1), (2, 3)]
                    }
                }
                11 => vec![(2, 1)],
                12 => vec![(1, 3)],
                13 => vec![(1, 0)],
                14 => vec![(0, 3)],
                _ => vec![],
            };
            for (a, b) in pairs {
                segments.push((edge_key(a), edge_point(a), edge_key(b), edge_point(b)));
            }
        }
    }
    // Chain segments: each edge key appears at most twice (once as an
    // end, once as a start of the next segment).
    let mut next: std::collections::HashMap<EdgeKey, usize> = std::collections::HashMap::new();
    for (k, s) in segments.iter().enumerate() {
        next.insert(s.0, k);
    }
    let mut used = vec![false; segments.len()];
    let mut loops = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        let mut lp = Vec::new();
        let mut k = start;
        loop {
            used[k] = true;
            lp.push(segments[k].1);
            let Some(&n) = next.get(&segments[k].2) else { break };
            if n == start {
                break;
            }
            if used[n] {
                break;
            }
            k = n;
        }
        if lp.len() >= 3 {
            // The case table walks with material on the right; turn the
            // loops so outer loops run counter clockwise.
            lp.reverse();
            loops.push(lp);
        }
    }
    loops
}

/// Signed area of a loop (positive when counter clockwise).
pub fn area(lp: &[DVec2]) -> f64 {
    let n = lp.len();
    (0..n).map(|i| lp[i].x * lp[(i + 1) % n].y - lp[(i + 1) % n].x * lp[i].y).sum::<f64>() * 0.5
}

/// Perimeter of a loop.
pub fn length(lp: &[DVec2]) -> f64 {
    let n = lp.len();
    (0..n).map(|i| (lp[(i + 1) % n] - lp[i]).length()).sum()
}

/// An SVG of the loops in millimetre units, filled by the even odd rule
/// so holes show as holes.
pub fn svg(loops: &[Vec<DVec2>], lo: DVec2, hi: DVec2) -> String {
    let w = hi.x - lo.x;
    let h = hi.y - lo.y;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"{} {} {w} {h}\">\n<g transform=\"translate(0 {}) scale(1 -1)\">\n<path fill=\"#333\" fill-rule=\"evenodd\" stroke=\"none\" d=\"",
        lo.x,
        lo.y,
        hi.y + lo.y
    );
    for lp in loops {
        for (i, p) in lp.iter().enumerate() {
            s.push_str(&format!("{}{:.4} {:.4} ", if i == 0 { "M" } else { "L" }, p.x, p.y));
        }
        s.push_str("Z ");
    }
    s.push_str("\"/>\n</g>\n</svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sphere, Subtract};

    #[test]
    fn sphere_slices_are_circles_with_the_right_area() {
        let s = Sphere { c: DVec3::ZERO, r: 10.0 };
        for z in [0.0, 5.0, 8.0] {
            let loops = contours(&s, z, DVec2::splat(-12.0), DVec2::splat(12.0), 0.5);
            assert_eq!(loops.len(), 1, "one loop at z {z}");
            let r2 = 100.0 - z * z;
            let a = area(&loops[0]);
            assert!((a - std::f64::consts::PI * r2).abs() / (std::f64::consts::PI * r2) < 0.01, "area {a} at z {z}");
            assert!(a > 0.0, "material on the left means counter clockwise outer loops");
        }
    }

    #[test]
    fn a_hole_gives_a_second_loop_of_opposite_turn() {
        let ring = Subtract(Sphere { c: DVec3::ZERO, r: 10.0 }, Sphere { c: DVec3::ZERO, r: 4.0 });
        let loops = contours(&ring, 0.0, DVec2::splat(-12.0), DVec2::splat(12.0), 0.25);
        assert_eq!(loops.len(), 2);
        let areas: Vec<f64> = loops.iter().map(|l| area(l)).collect();
        assert!(areas.iter().any(|a| *a > 0.0) && areas.iter().any(|a| *a < 0.0), "{areas:?}");
        let text = svg(&loops, DVec2::splat(-12.0), DVec2::splat(12.0));
        assert!(text.contains("evenodd") && text.matches('Z').count() == 2);
    }
}
