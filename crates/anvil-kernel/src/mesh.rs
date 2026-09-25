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

    let mut pts3: Vec<DVec3> = f.outer.iter().map(|&vid| solid.pos(vid)).collect();
    let mut pts2: Vec<DVec2> = pts3.iter().map(|&p| to2d(p)).collect();
    let tris = if f.inner.is_empty() {
        ear_clip(&pts2)
    } else {
        // Holes: append their points after the outer loop and bridge them.
        let outer_n = pts2.len();
        let mut holes: Vec<Vec<usize>> = Vec::new();
        for h in &f.inner {
            let start = pts2.len();
            for &vid in h {
                pts3.push(solid.pos(vid));
                pts2.push(to2d(solid.pos(vid)));
            }
            holes.push((start..pts2.len()).collect());
        }
        ear_clip_with_holes(&pts2, outer_n, &holes)
    };

    let face_of_tri = vec![id; tris.len() / 3];
    TriMesh {
        normals: vec![n; pts3.len()],
        positions: pts3,
        indices: tris.iter().map(|&i| i as u32).collect(),
        face_of_tri,
    }
}

/// Ear clipping for a simple polygon. Input is any winding; output triangles
/// are counter-clockwise in the given 2D frame. O(n^2), fine for CAD facets.
pub fn ear_clip(poly: &[DVec2]) -> Vec<usize> {
    // Vertices on a straight run (collinear with their neighbours) stall
    // ear clipping. Set them aside, clip the rest, then split the triangle
    // edges they lie on, so neighbouring faces still share every vertex.
    let n = poly.len();
    if n < 4 {
        return ear_clip_core(poly, &(0..n).collect::<Vec<_>>());
    }
    let scale = poly.iter().fold(0.0f64, |m, p| m.max(p.x.abs()).max(p.y.abs())).max(1.0);
    // Cross product tolerance (area-like) and squared distance tolerance.
    let tol = 1e-12 * scale * scale;
    let dist2 = (1e-9 * scale).powi(2);
    let mut keep: Vec<usize> = (0..n).collect();
    let mut removed: Vec<usize> = Vec::new();
    loop {
        let m = keep.len();
        if m <= 3 {
            break;
        }
        let mut hit = None;
        for k in 0..m {
            let (a, b, c) = (poly[keep[(k + m - 1) % m]], poly[keep[k]], poly[keep[(k + 1) % m]]);
            if (b - a).perp_dot(c - b).abs() <= tol && (b - a).dot(c - b) > 0.0 {
                hit = Some(k);
                break;
            }
        }
        match hit {
            Some(k) => removed.push(keep.remove(k)),
            None => break,
        }
    }
    let mut tris = ear_clip_core(poly, &keep);
    for r in removed {
        let p = poly[r];
        let mut split = None;
        'find: for (t, tri) in tris.chunks(3).enumerate() {
            for e in 0..3 {
                let (i, j) = (tri[e], tri[(e + 1) % 3]);
                let (a, b) = (poly[i], poly[j]);
                let ab = b - a;
                let len2 = ab.length_squared();
                if len2 < 1e-24 {
                    continue;
                }
                let u = (p - a).dot(ab) / len2;
                if u > 1e-12 && u < 1.0 - 1e-12 && (p - (a + ab * u)).length_squared() <= dist2 {
                    split = Some((t, e));
                    break 'find;
                }
            }
        }
        if let Some((t, e)) = split {
            let tri = [tris[t * 3], tris[t * 3 + 1], tris[t * 3 + 2]];
            let (i, j, k) = (tri[e], tri[(e + 1) % 3], tri[(e + 2) % 3]);
            tris[t * 3] = i;
            tris[t * 3 + 1] = r;
            tris[t * 3 + 2] = k;
            tris.extend_from_slice(&[r, j, k]);
        }
    }
    tris
}

/// Ear clipping over the subset `sub` of `poly` (indices in order).
fn ear_clip_core(poly: &[DVec2], sub: &[usize]) -> Vec<usize> {
    let pts: Vec<DVec2> = sub.iter().map(|&i| poly[i]).collect();
    ear_clip_plain(&pts).into_iter().map(|k| sub[k]).collect()
}

fn ear_clip_plain(poly: &[DVec2]) -> Vec<usize> {
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
            // A reflex vertex inside the ear, or on its boundary, spoils
            // it. The boundary counts because the bridge to a hole puts
            // two copies of a point in the loop; a copy sitting on the
            // ear's corner can still turn back into the ear.
            let blocked = (0..m).any(|k| {
                let j = idx[k];
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let (p, q, r) = (poly[idx[(k + m - 1) % m]], poly[j], poly[idx[(k + 1) % m]]);
                let reflex = (q - p).perp_dot(r - q) <= 1e-14;
                reflex && point_in_tri_closed(q, a, b, c)
            });
            if blocked {
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

/// Ear clipping for a polygon with holes. `pts[..outer_n]` is the outer
/// loop; `holes` lists index ranges of each hole. Each hole is bridged to
/// the nearest outer vertex, which is adequate for text glyphs and simple
/// pockets. Output triangles index into `pts`.
pub fn ear_clip_with_holes(pts: &[DVec2], outer_n: usize, holes: &[Vec<usize>]) -> Vec<usize> {
    let area = |idx: &[usize]| -> f64 {
        let n = idx.len();
        (0..n).map(|i| pts[idx[i]].perp_dot(pts[idx[(i + 1) % n]])).sum()
    };
    let mut merged: Vec<usize> = (0..outer_n).collect();
    if area(&merged) < 0.0 {
        merged.reverse();
    }
    // Process holes from rightmost to leftmost.
    let mut hs: Vec<Vec<usize>> = holes.to_vec();
    for h in &mut hs {
        if area(h) > 0.0 {
            h.reverse(); // holes go clockwise
        }
    }
    hs.sort_by(|a, b| {
        let ma = a.iter().map(|&i| pts[i].x).fold(f64::NEG_INFINITY, f64::max);
        let mb = b.iter().map(|&i| pts[i].x).fold(f64::NEG_INFINITY, f64::max);
        mb.partial_cmp(&ma).unwrap()
    });
    for h in hs {
        // Hole vertex with max x, then the closest merged vertex to it.
        let (hk, _) =
            h.iter()
                .enumerate()
                .fold((0, f64::NEG_INFINITY), |acc, (k, &i)| if pts[i].x > acc.1 { (k, pts[i].x) } else { acc });
        let hp = pts[h[hk]];
        // Nearest merged vertex whose bridge does not cross any edge.
        let crosses = |a: DVec2, b: DVec2, loop_idx: &[usize]| -> bool {
            let n = loop_idx.len();
            (0..n).any(|e| {
                let (c, d) = (pts[loop_idx[e]], pts[loop_idx[(e + 1) % n]]);
                if (c - a).length() < 1e-12
                    || (d - a).length() < 1e-12
                    || (c - b).length() < 1e-12
                    || (d - b).length() < 1e-12
                {
                    return false;
                }
                let r = b - a;
                let q = d - c;
                let den = r.perp_dot(q);
                if den.abs() < 1e-18 {
                    return false;
                }
                let t = (c - a).perp_dot(q) / den;
                let u = (c - a).perp_dot(r) / den;
                t > 1e-9 && t < 1.0 - 1e-9 && u > 1e-9 && u < 1.0 - 1e-9
            })
        };
        let mut order: Vec<usize> = (0..merged.len()).collect();
        order.sort_by(|&a, &b| {
            (pts[merged[a]] - hp).length_squared().partial_cmp(&(pts[merged[b]] - hp).length_squared()).unwrap()
        });
        let mi = order
            .iter()
            .copied()
            .find(|&k| !crosses(hp, pts[merged[k]], &merged) && !crosses(hp, pts[merged[k]], &h))
            .unwrap_or(order[0]);
        let mut new_loop = Vec::with_capacity(merged.len() + h.len() + 2);
        new_loop.extend_from_slice(&merged[..=mi]);
        for k in 0..h.len() {
            new_loop.push(h[(hk + k) % h.len()]);
        }
        new_loop.push(h[hk]);
        new_loop.push(merged[mi]);
        new_loop.extend_from_slice(&merged[mi + 1..]);
        merged = new_loop;
    }
    let local: Vec<DVec2> = merged.iter().map(|&i| pts[i]).collect();
    ear_clip(&local).into_iter().map(|i| merged[i]).collect()
}

/// Inside or on the boundary of the counter-clockwise triangle a, b, c.
fn point_in_tri_closed(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let s1 = (b - a).perp_dot(p - a);
    let s2 = (c - b).perp_dot(p - b);
    let s3 = (a - c).perp_dot(p - c);
    s1 >= -1e-14 && s2 >= -1e-14 && s3 >= -1e-14
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
    #[test]
    fn ear_clip_square_with_square_hole() {
        let mut pts = vec![DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)];
        pts.extend([DVec2::new(3.0, 3.0), DVec2::new(7.0, 3.0), DVec2::new(7.0, 7.0), DVec2::new(3.0, 7.0)]);
        let tris = ear_clip_with_holes(&pts, 4, &[vec![4, 5, 6, 7]]);
        let area: f64 = tris.chunks(3).map(|t| (pts[t[1]] - pts[t[0]]).perp_dot(pts[t[2]] - pts[t[0]]) * 0.5).sum();
        assert!((area - 84.0).abs() < 1e-9, "{area}");
    }

    #[test]
    fn ear_clip_rectangle_with_extra_edge_vertices() {
        // A rectangle whose edges carry extra vertices, as left by booleans.
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(5.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 4.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(3.0, 10.0),
            DVec2::new(0.0, 10.0),
            DVec2::new(0.0, 6.0),
        ];
        let tris = ear_clip(&pts);
        let mut area = 0.0;
        for t in tris.chunks(3) {
            let a = (pts[t[1]] - pts[t[0]]).perp_dot(pts[t[2]] - pts[t[0]]) * 0.5;
            assert!(a > -1e-12, "no inverted triangles");
            area += a;
        }
        assert!((area - 100.0).abs() < 1e-9, "{area}");
        for v in 0..pts.len() {
            assert!(tris.contains(&v), "vertex {v} is used");
        }
    }
}
