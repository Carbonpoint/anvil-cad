//! A field sampled on a grid, built from a closed triangle mesh.
//!
//! Inside or outside is decided by ray parity along grid rows, then an
//! exact Euclidean distance transform gives each voxel its distance to the
//! nearest boundary voxel. The result is a signed distance accurate to
//! about one voxel, read back with trilinear interpolation.

use crate::Field;
use anvil_math::DVec3;

pub struct Sampled {
    pub lo: DVec3,
    pub step: f64,
    pub n: [usize; 3],
    /// Signed distance at voxel centres, x-major.
    pub values: Vec<f64>,
}

impl Sampled {
    /// Sample the inside of a closed triangle mesh at `step`, padded by
    /// `pad` voxels of outside on every side.
    pub fn from_triangles(tris: &[[DVec3; 3]], step: f64, pad: usize) -> Sampled {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for t in tris {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let lo = lo - DVec3::splat(step * pad as f64);
        let hi = hi + DVec3::splat(step * pad as f64);
        let n = [
            ((hi.x - lo.x) / step).ceil().max(1.0) as usize,
            ((hi.y - lo.y) / step).ceil().max(1.0) as usize,
            ((hi.z - lo.z) / step).ceil().max(1.0) as usize,
        ];
        let idx = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
        // Parity along x rows: for each (j, k) row through voxel centres,
        // collect the x of every triangle crossing, then walk the row.
        let mut crossings: Vec<Vec<f64>> = vec![Vec::new(); n[1] * n[2]];
        for t in tris {
            let (a, b, c) = (t[0], t[1], t[2]);
            let ymin = a.y.min(b.y).min(c.y);
            let ymax = a.y.max(b.y).max(c.y);
            let zmin = a.z.min(b.z).min(c.z);
            let zmax = a.z.max(b.z).max(c.z);
            let j0 = (((ymin - lo.y) / step - 0.5).ceil().max(0.0)) as usize;
            let j1 = (((ymax - lo.y) / step - 0.5).floor().min(n[1] as f64 - 1.0)) as isize;
            let k0 = (((zmin - lo.z) / step - 0.5).ceil().max(0.0)) as usize;
            let k1 = (((zmax - lo.z) / step - 0.5).floor().min(n[2] as f64 - 1.0)) as isize;
            for j in j0..=(j1.max(-1) as usize).min(n[1].saturating_sub(1)) {
                if j1 < 0 {
                    break;
                }
                let y = lo.y + (j as f64 + 0.5) * step;
                for k in k0..=(k1.max(-1) as usize).min(n[2].saturating_sub(1)) {
                    if k1 < 0 {
                        break;
                    }
                    let z = lo.z + (k as f64 + 0.5) * step;
                    if let Some(x) = ray_x(a, b, c, y, z) {
                        crossings[j * n[2] + k].push(x);
                    }
                }
            }
        }
        let mut inside = vec![false; n[0] * n[1] * n[2]];
        for j in 0..n[1] {
            for k in 0..n[2] {
                let xs = &mut crossings[j * n[2] + k];
                if xs.is_empty() {
                    continue;
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                for i in 0..n[0] {
                    let x = lo.x + (i as f64 + 0.5) * step;
                    let before = xs.partition_point(|&c| c < x);
                    inside[idx(i, j, k)] = before % 2 == 1;
                }
            }
        }
        // Distance to the nearest voxel of the other sign, both ways.
        let d_out = distance_transform(&inside, n, true);
        let d_in = distance_transform(&inside, n, false);
        let values = (0..inside.len())
            .map(|q| if inside[q] { -(d_in[q].sqrt() - 0.5) * step } else { (d_out[q].sqrt() - 0.5) * step })
            .collect();
        Sampled { lo, step, n, values }
    }

    fn value(&self, i: usize, j: usize, k: usize) -> f64 {
        let i = i.min(self.n[0] - 1);
        let j = j.min(self.n[1] - 1);
        let k = k.min(self.n[2] - 1);
        self.values[(i * self.n[1] + j) * self.n[2] + k]
    }
}

/// x where the line through (y, z) crosses the triangle, if it does. The
/// triangle is projected onto (y, z) and turned counter clockwise; a hit
/// exactly on an edge counts only for a left or top edge, so an edge two
/// triangles share is counted once, and a silhouette edge zero or two
/// times. That keeps the parity right without any perturbation.
fn ray_x(a: DVec3, b: DVec3, c: DVec3, y: f64, z: f64) -> Option<f64> {
    let mut v = [a, b, c];
    let area = (b.y - a.y) * (c.z - a.z) - (b.z - a.z) * (c.y - a.y);
    if area == 0.0 {
        return None;
    }
    if area < 0.0 {
        v.swap(1, 2);
    }
    let area = area.abs();
    let mut w = [0.0; 3];
    for e in 0..3 {
        let (p, q) = (v[(e + 1) % 3], v[(e + 2) % 3]);
        let (dy, dz) = (q.y - p.y, q.z - p.z);
        let we = dy * (z - p.z) - dz * (y - p.y);
        if we < 0.0 || (we == 0.0 && !(dz < 0.0 || (dz == 0.0 && dy < 0.0))) {
            return None;
        }
        w[e] = we;
    }
    Some((w[0] * v[0].x + w[1] * v[1].x + w[2] * v[2].x) / area)
}

/// Squared Euclidean distance (in voxels) from every voxel to the nearest
/// voxel whose `inside` equals `target`. Felzenszwalb and Huttenlocher,
/// one pass per axis.
fn distance_transform(inside: &[bool], n: [usize; 3], target: bool) -> Vec<f64> {
    const INF: f64 = 1e20;
    let mut f: Vec<f64> = inside.iter().map(|&b| if b == target { 0.0 } else { INF }).collect();
    let idx = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
    let mut line = Vec::new();
    let mut out = Vec::new();
    // Along z.
    for i in 0..n[0] {
        for j in 0..n[1] {
            line.clear();
            line.extend((0..n[2]).map(|k| f[idx(i, j, k)]));
            dt1d(&line, &mut out);
            for k in 0..n[2] {
                f[idx(i, j, k)] = out[k];
            }
        }
    }
    // Along y.
    for i in 0..n[0] {
        for k in 0..n[2] {
            line.clear();
            line.extend((0..n[1]).map(|j| f[idx(i, j, k)]));
            dt1d(&line, &mut out);
            for j in 0..n[1] {
                f[idx(i, j, k)] = out[j];
            }
        }
    }
    // Along x.
    for j in 0..n[1] {
        for k in 0..n[2] {
            line.clear();
            line.extend((0..n[0]).map(|i| f[idx(i, j, k)]));
            dt1d(&line, &mut out);
            for i in 0..n[0] {
                f[idx(i, j, k)] = out[i];
            }
        }
    }
    f
}

/// One dimensional squared distance transform with lower envelope of
/// parabolas.
fn dt1d(f: &[f64], out: &mut Vec<f64>) {
    let n = f.len();
    out.clear();
    out.resize(n, 0.0);
    if n == 0 {
        return;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * q as f64 - 2.0 * p as f64);
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            if s <= z[k] {
                // k == 0: replace the first parabola.
                v[0] = q;
                z[0] = f64::NEG_INFINITY;
                z[1] = f64::INFINITY;
                break;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *o = (q as f64 - p as f64).powi(2) + f[p];
    }
}

impl Field for Sampled {
    fn at(&self, p: DVec3) -> f64 {
        // Voxel centre coordinates.
        let g = (p - self.lo) / self.step - DVec3::splat(0.5);
        let c = |x: f64, n: usize| x.clamp(0.0, (n as f64 - 1.0).max(0.0));
        let (x, y, z) = (c(g.x, self.n[0]), c(g.y, self.n[1]), c(g.z, self.n[2]));
        let (i, j, k) = (x.floor() as usize, y.floor() as usize, z.floor() as usize);
        let (fx, fy, fz) = (x - i as f64, y - j as f64, z - k as f64);
        let mut acc = 0.0;
        for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
            for (dj, wy) in [(0, 1.0 - fy), (1, fy)] {
                for (dk, wz) in [(0, 1.0 - fz), (1, fz)] {
                    let w = wx * wy * wz;
                    if w > 0.0 {
                        acc += w * self.value(i + di, j + dj, k + dk);
                    }
                }
            }
        }
        // Beyond the grid, which is padded with outside voxels, the
        // distance keeps growing with the distance to the grid.
        let outside = (g - DVec3::new(x, y, z)).length() * self.step;
        acc + outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{surface_nets, volume};
    use crate::Sphere;

    #[test]
    fn sampled_sphere_round_trips() {
        let s = Sphere { c: DVec3::ZERO, r: 10.0 };
        let tris = surface_nets(&s, DVec3::splat(-12.0), DVec3::splat(12.0), 0.5);
        let f = Sampled::from_triangles(&tris, 0.5, 3);
        assert!(f.at(DVec3::ZERO) < -8.0, "{}", f.at(DVec3::ZERO));
        assert!((f.at(DVec3::new(10.0, 0.0, 0.0))).abs() < 0.6);
        assert!(f.at(DVec3::new(13.0, 0.0, 0.0)) > 2.0);
        let again =
            surface_nets(&f, f.lo, f.lo + DVec3::new(f.n[0] as f64, f.n[1] as f64, f.n[2] as f64) * f.step, 0.5);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
        let vol = volume(&again);
        assert!((vol - exact).abs() / exact < 0.05, "{vol} vs {exact}");
    }

    #[test]
    fn distance_transform_counts_voxels() {
        let n = [5, 1, 1];
        let inside = [true, false, false, false, true];
        let d = distance_transform(&inside, n, true);
        assert_eq!(d, vec![0.0, 1.0, 4.0, 1.0, 0.0]);
    }
}
