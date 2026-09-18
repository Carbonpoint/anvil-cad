//! Fields from data: a scalar sampled at scattered points (a simulation
//! result, a measurement) read from CSV, looked up by inverse distance
//! weighting over a bucket grid. This is the hinge of field driven
//! design: a wall thickness or a beam radius that follows a stress map.

use crate::Field;
use anvil_math::DVec3;

/// A scalar known at scattered points.
pub struct PointMap {
    pts: Vec<(DVec3, f64)>,
    lo: DVec3,
    hi: DVec3,
    cell: f64,
    n: [usize; 3],
    buckets: Vec<Vec<u32>>,
    /// Value used where no sample is within `reach`.
    pub fallback: f64,
    /// Search radius; samples farther than this do not count.
    pub reach: f64,
    /// Inverse distance power (2 is the usual choice).
    pub power: f64,
}

impl PointMap {
    /// Build from points and values. `reach` is the search radius; it
    /// defaults to three times the mean sample spacing when zero.
    pub fn new(pts: Vec<(DVec3, f64)>, reach: f64, fallback: f64) -> PointMap {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for (p, _) in &pts {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        if pts.is_empty() {
            lo = DVec3::ZERO;
            hi = DVec3::ONE;
        }
        let ext = (hi - lo).max(DVec3::splat(1e-9));
        let spacing = (ext.x * ext.y * ext.z / pts.len().max(1) as f64).cbrt().max(1e-9);
        let reach = if reach > 0.0 { reach } else { 3.0 * spacing };
        let cell = reach;
        let n = [
            (ext.x / cell).ceil() as usize + 1,
            (ext.y / cell).ceil() as usize + 1,
            (ext.z / cell).ceil() as usize + 1,
        ];
        let mut buckets = vec![Vec::new(); n[0] * n[1] * n[2]];
        for (k, (p, _)) in pts.iter().enumerate() {
            let (i, j, l) = Self::cell_of(lo, cell, n, *p);
            buckets[(i * n[1] + j) * n[2] + l].push(k as u32);
        }
        PointMap { pts, lo, hi, cell, n, buckets, fallback, reach, power: 2.0 }
    }

    fn cell_of(lo: DVec3, cell: f64, n: [usize; 3], p: DVec3) -> (usize, usize, usize) {
        let g = (p - lo) / cell;
        (
            (g.x.floor().max(0.0) as usize).min(n[0] - 1),
            (g.y.floor().max(0.0) as usize).min(n[1] - 1),
            (g.z.floor().max(0.0) as usize).min(n[2] - 1),
        )
    }

    /// Parse CSV lines of `x,y,z,value` (a header line and blank lines
    /// are skipped; separators may be commas, semicolons, or whitespace).
    pub fn parse_csv(text: &str) -> Result<Vec<(DVec3, f64)>, String> {
        let mut out = Vec::new();
        for (ln, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> =
                line.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|f| !f.is_empty()).collect();
            if fields.len() < 4 {
                if ln == 0 {
                    continue;
                }
                return Err(format!("line {}: need x, y, z, value", ln + 1));
            }
            let nums: Result<Vec<f64>, _> = fields[..4].iter().map(|f| f.parse::<f64>()).collect();
            match nums {
                Ok(v) => out.push((DVec3::new(v[0], v[1], v[2]), v[3])),
                Err(_) if ln == 0 => continue,
                Err(e) => return Err(format!("line {}: {e}", ln + 1)),
            }
        }
        if out.is_empty() {
            return Err("no samples".into());
        }
        Ok(out)
    }

    pub fn from_csv(text: &str, reach: f64, fallback: f64) -> Result<PointMap, String> {
        Ok(PointMap::new(Self::parse_csv(text)?, reach, fallback))
    }

    pub fn len(&self) -> usize {
        self.pts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pts.is_empty()
    }

    /// Corners of the box the samples span.
    pub fn bounds(&self) -> (DVec3, DVec3) {
        (self.lo, self.hi)
    }

    /// Smallest and largest sample value.
    pub fn range(&self) -> (f64, f64) {
        self.pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)))
    }
}

impl Field for PointMap {
    fn at(&self, p: DVec3) -> f64 {
        if self.pts.is_empty() {
            return self.fallback;
        }
        let (ci, cj, ck) = Self::cell_of(self.lo, self.cell, self.n, p);
        let r2 = self.reach * self.reach;
        let mut num = 0.0;
        let mut den = 0.0;
        for di in -1i64..=1 {
            for dj in -1i64..=1 {
                for dk in -1i64..=1 {
                    let (i, j, k) = (ci as i64 + di, cj as i64 + dj, ck as i64 + dk);
                    if i < 0
                        || j < 0
                        || k < 0
                        || i as usize >= self.n[0]
                        || j as usize >= self.n[1]
                        || k as usize >= self.n[2]
                    {
                        continue;
                    }
                    for &idx in &self.buckets[(i as usize * self.n[1] + j as usize) * self.n[2] + k as usize] {
                        let (q, v) = self.pts[idx as usize];
                        let d2 = (q - p).length_squared();
                        if d2 > r2 {
                            continue;
                        }
                        if d2 < 1e-18 {
                            return v;
                        }
                        let w = 1.0 / d2.powf(self.power * 0.5);
                        num += w * v;
                        den += w;
                    }
                }
            }
        }
        if den > 0.0 {
            num / den
        } else {
            self.fallback
        }
    }
}

/// Map a scalar field linearly: `a` where the input is `in_lo`, `b`
/// where it is `in_hi`, clamped. Turns a stress map into a thickness.
pub struct Remap<F> {
    pub field: F,
    pub in_lo: f64,
    pub in_hi: f64,
    pub a: f64,
    pub b: f64,
}

impl<F: Field> Field for Remap<F> {
    fn at(&self, p: DVec3) -> f64 {
        let span = (self.in_hi - self.in_lo).abs().max(1e-18);
        let t = ((self.field.at(p) - self.in_lo) / span).clamp(0.0, 1.0);
        self.a + (self.b - self.a) * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_points_interpolate_and_fall_back() {
        let csv = "x,y,z,stress\n0,0,0,10\n10,0,0,20\n0,10,0,30\n0,0,10,40\n";
        let m = PointMap::from_csv(csv, 20.0, -1.0).unwrap();
        assert_eq!(m.len(), 4);
        assert!((m.at(DVec3::new(10.0, 0.0, 0.0)) - 20.0).abs() < 1e-9, "exact at a sample");
        let mid = m.at(DVec3::new(5.0, 0.0, 0.0));
        assert!(mid > 10.0 && mid < 20.0, "{mid}");
        assert_eq!(m.at(DVec3::new(100.0, 100.0, 100.0)), -1.0, "fallback out of reach");
        let r = Remap { field: &m, in_lo: 10.0, in_hi: 40.0, a: 1.0, b: 3.0 };
        assert!((r.at(DVec3::new(0.0, 0.0, 10.0)) - 3.0).abs() < 1e-9);
        assert!((r.at(DVec3::new(0.0, 0.0, 0.0)) - 1.0).abs() < 1e-9);
    }
}
