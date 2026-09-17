//! Surface nets: turn a field into a closed triangle mesh.
//!
//! The field is sampled at the corners of a grid of cubes of side `step`
//! over a box. Every cube whose corners differ in sign gets one vertex at
//! the mean of its edge crossings, and every grid edge that changes sign
//! joins the four cubes around it with two triangles. The result is closed
//! whenever the field is positive on the whole boundary of the box, so
//! callers pad the box by a few steps.

use crate::Field;
use anvil_math::DVec3;

/// Mesh the zero surface of `field` inside the box `lo..hi` with cubes of
/// side `step`. Returns triangles wound so their normals point outward
/// (toward positive values).
pub fn surface_nets(field: &dyn Field, lo: DVec3, hi: DVec3, step: f64) -> Vec<[DVec3; 3]> {
    let n = [
        ((hi.x - lo.x) / step).ceil().max(1.0) as usize,
        ((hi.y - lo.y) / step).ceil().max(1.0) as usize,
        ((hi.z - lo.z) / step).ceil().max(1.0) as usize,
    ];
    let corners = [n[0] + 1, n[1] + 1, n[2] + 1];
    let cidx = |i: usize, j: usize, k: usize| (i * corners[1] + j) * corners[2] + k;
    let pos = |i: usize, j: usize, k: usize| lo + DVec3::new(i as f64, j as f64, k as f64) * step;
    let mut v = vec![0.0f64; corners[0] * corners[1] * corners[2]];
    for i in 0..corners[0] {
        for j in 0..corners[1] {
            for k in 0..corners[2] {
                let d = field.at(pos(i, j, k));
                // A corner exactly on the surface counts as outside.
                v[cidx(i, j, k)] = if d == 0.0 { f64::MIN_POSITIVE } else { d };
            }
        }
    }
    // One vertex per cube with a sign change.
    let vidx = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
    let mut cell_vertex: Vec<Option<u32>> = vec![None; n[0] * n[1] * n[2]];
    let mut points: Vec<DVec3> = Vec::new();
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                let mut cv = [0.0; 8];
                let mut cp = [DVec3::ZERO; 8];
                for (c, (cv, cp)) in cv.iter_mut().zip(cp.iter_mut()).enumerate() {
                    let (di, dj, dk) = ((c >> 2) & 1, (c >> 1) & 1, c & 1);
                    *cv = v[cidx(i + di, j + dj, k + dk)];
                    *cp = pos(i + di, j + dj, k + dk);
                }
                let mut sum = DVec3::ZERO;
                let mut count = 0.0;
                for (a, b) in EDGES {
                    if (cv[a] < 0.0) != (cv[b] < 0.0) {
                        let t = cv[a] / (cv[a] - cv[b]);
                        sum += cp[a] + (cp[b] - cp[a]) * t;
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    cell_vertex[vidx(i, j, k)] = Some(points.len() as u32);
                    points.push(sum / count);
                }
            }
        }
    }
    // Two triangles per sign changing grid edge.
    let mut tris: Vec<[DVec3; 3]> = Vec::new();
    let mut quad = |cells: [Option<u32>; 4], flip: bool| {
        let (Some(a), Some(b), Some(c), Some(d)) = (cells[0], cells[1], cells[2], cells[3]) else { return };
        let (pa, pb, pc, pd) = (points[a as usize], points[b as usize], points[c as usize], points[d as usize]);
        if flip {
            tris.push([pa, pc, pb]);
            tris.push([pa, pd, pc]);
        } else {
            tris.push([pa, pb, pc]);
            tris.push([pa, pc, pd]);
        }
    };
    let cell = |i: isize, j: isize, k: isize| -> Option<u32> {
        if i < 0 || j < 0 || k < 0 || i as usize >= n[0] || j as usize >= n[1] || k as usize >= n[2] {
            None
        } else {
            cell_vertex[vidx(i as usize, j as usize, k as usize)]
        }
    };
    for i in 0..corners[0] {
        for j in 0..corners[1] {
            for k in 0..corners[2] {
                let (ii, jj, kk) = (i as isize, j as isize, k as isize);
                let here = v[cidx(i, j, k)] < 0.0;
                // Edge along x from (i, j, k) to (i + 1, j, k): the four
                // cubes around it differ in j and k.
                if i + 1 < corners[0] && here != (v[cidx(i + 1, j, k)] < 0.0) {
                    quad(
                        [cell(ii, jj - 1, kk - 1), cell(ii, jj, kk - 1), cell(ii, jj, kk), cell(ii, jj - 1, kk)],
                        !here,
                    );
                }
                if j + 1 < corners[1] && here != (v[cidx(i, j + 1, k)] < 0.0) {
                    quad(
                        [cell(ii - 1, jj, kk - 1), cell(ii - 1, jj, kk), cell(ii, jj, kk), cell(ii, jj, kk - 1)],
                        !here,
                    );
                }
                if k + 1 < corners[2] && here != (v[cidx(i, j, k + 1)] < 0.0) {
                    quad(
                        [cell(ii - 1, jj - 1, kk), cell(ii, jj - 1, kk), cell(ii, jj, kk), cell(ii - 1, jj, kk)],
                        !here,
                    );
                }
            }
        }
    }
    tris
}

/// Signed volume of a triangle soup (positive when the normals point out).
pub fn volume(tris: &[[DVec3; 3]]) -> f64 {
    tris.iter().map(|t| t[0].dot(t[1].cross(t[2])) / 6.0).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sphere;

    #[test]
    fn sphere_mesh_is_closed_and_has_the_right_volume() {
        let s = Sphere { c: DVec3::ZERO, r: 10.0 };
        let tris = surface_nets(&s, DVec3::splat(-12.0), DVec3::splat(12.0), 0.5);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
        let vol = volume(&tris);
        assert!((vol - exact).abs() / exact < 0.02, "{vol} vs {exact}");
        // Every edge is used twice in opposite directions.
        let mut uses: std::collections::HashMap<([i64; 3], [i64; 3]), i32> = std::collections::HashMap::new();
        let key = |p: DVec3| [(p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64, (p.z * 1e6).round() as i64];
        for t in &tris {
            for e in 0..3 {
                let (a, b) = (key(t[e]), key(t[(e + 1) % 3]));
                *uses.entry((a, b)).or_default() += 1;
                *uses.entry((b, a)).or_default() -= 1;
            }
        }
        assert!(uses.values().all(|&u| u == 0), "open or inconsistent edges");
    }
}
