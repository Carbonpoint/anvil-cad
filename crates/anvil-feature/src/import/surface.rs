//! Curved STEP surfaces as maps from a parameter plane (u, v) to space,
//! with the way back, so a face's boundary can be carried into (u, v),
//! triangulated there, and carried out again.

use anvil_math::DVec3;
use std::f64::consts::{FRAC_PI_2, TAU};

/// A local frame: origin, normal (z), and x axis.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub o: DVec3,
    pub z: DVec3,
    pub x: DVec3,
}

impl Frame {
    pub fn y(&self) -> DVec3 {
        self.z.cross(self.x)
    }
    pub fn local(&self, p: DVec3) -> DVec3 {
        let d = p - self.o;
        DVec3::new(d.dot(self.x), d.dot(self.y()), d.dot(self.z))
    }
    fn radial(&self, u: f64) -> DVec3 {
        self.x * u.cos() + self.y() * u.sin()
    }
}

/// A polyline with arc length, for swept surfaces.
#[derive(Clone, Debug)]
pub struct Path {
    pts: Vec<DVec3>,
    /// Arc length at each point.
    s: Vec<f64>,
}

impl Path {
    pub fn new(pts: Vec<DVec3>) -> Option<Path> {
        if pts.len() < 2 {
            return None;
        }
        let mut s = vec![0.0];
        for w in pts.windows(2) {
            s.push(s[s.len() - 1] + (w[1] - w[0]).length());
        }
        (s[s.len() - 1] > 1e-12).then_some(Path { pts, s })
    }
    pub fn points(&self) -> &[DVec3] {
        &self.pts
    }
    fn len(&self) -> f64 {
        self.s[self.s.len() - 1]
    }
    fn at(&self, t: f64) -> DVec3 {
        let t = t.clamp(0.0, self.len());
        let k = self.s.partition_point(|&x| x <= t).clamp(1, self.pts.len() - 1);
        let (s0, s1) = (self.s[k - 1], self.s[k]);
        let f = if s1 > s0 { (t - s0) / (s1 - s0) } else { 0.0 };
        self.pts[k - 1].lerp(self.pts[k], f)
    }
    /// Arc length of the point on the path nearest `p`, found with `map`
    /// applied to every path point first.
    fn nearest(&self, p: DVec3, map: impl Fn(DVec3) -> DVec3) -> f64 {
        let mut best = (f64::MAX, 0.0);
        for k in 1..self.pts.len() {
            let (a, b) = (map(self.pts[k - 1]), map(self.pts[k]));
            let ab = b - a;
            let f =
                if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let d = (a + ab * f - p).length_squared();
            if d < best.0 {
                best = (d, self.s[k - 1] + (self.s[k] - self.s[k - 1]) * f);
            }
        }
        best.1
    }
}

/// A tensor B-spline surface, maybe rational, from STEP data.
#[derive(Clone, Debug)]
pub struct BSplineSurface {
    pub du: usize,
    pub dv: usize,
    /// Control points, row by row: `ctrl[i][j]` with i along u.
    pub ctrl: Vec<Vec<DVec3>>,
    pub weights: Vec<Vec<f64>>,
    pub ku: Vec<f64>,
    pub kv: Vec<f64>,
    /// A coarse grid of (u, v, point) for the first guess of `inverse`.
    seeds: Vec<(f64, f64, DVec3)>,
}

impl BSplineSurface {
    /// Build from STEP form: knots listed once with multiplicities.
    pub fn new(
        du: usize,
        dv: usize,
        ctrl: Vec<Vec<DVec3>>,
        weights: Option<Vec<Vec<f64>>>,
        (umult, uknot): (&[f64], &[f64]),
        (vmult, vknot): (&[f64], &[f64]),
    ) -> Option<Self> {
        let nu = ctrl.len();
        let nv = ctrl.first()?.len();
        if du == 0 || dv == 0 || du > 16 || dv > 16 || nu <= du || nv <= dv || ctrl.iter().any(|r| r.len() != nv) {
            return None;
        }
        let expand = |m: &[f64], k: &[f64], deg: usize, n: usize| -> Option<Vec<f64>> {
            if m.len() != k.len() {
                return None;
            }
            let mut out = Vec::new();
            for (m, k) in m.iter().zip(k) {
                let m = *m as usize;
                if m == 0 || m > deg + 1 || out.len() + m > n + deg + 1 {
                    return None;
                }
                out.extend(std::iter::repeat_n(*k, m));
            }
            (out.len() == n + deg + 1).then_some(out)
        };
        let ku = expand(umult, uknot, du, nu)?;
        let kv = expand(vmult, vknot, dv, nv)?;
        let weights = match weights {
            Some(w) if w.len() == nu && w.iter().all(|r| r.len() == nv) => w,
            _ => vec![vec![1.0; nv]; nu],
        };
        let mut s = BSplineSurface { du, dv, ctrl, weights, ku, kv, seeds: Vec::new() };
        let ((u0, u1), (v0, v1)) = (s.u_range(), s.v_range());
        if u1 <= u0 || v1 <= v0 {
            return None;
        }
        let (gu, gv) = ((nu - du) * 6 + 6, (nv - dv) * 6 + 6);
        for i in 0..=gu {
            for j in 0..=gv {
                let (u, v) = (u0 + (u1 - u0) * i as f64 / gu as f64, v0 + (v1 - v0) * j as f64 / gv as f64);
                let p = s.eval(u, v);
                if p.is_finite() {
                    s.seeds.push((u, v, p));
                }
            }
        }
        (!s.seeds.is_empty()).then_some(s)
    }

    pub fn u_range(&self) -> (f64, f64) {
        (self.ku[self.du], self.ku[self.ctrl.len()])
    }
    pub fn v_range(&self) -> (f64, f64) {
        (self.kv[self.dv], self.kv[self.ctrl[0].len()])
    }

    pub fn eval(&self, u: f64, v: f64) -> DVec3 {
        // Along v in every row that the u span needs, then along u.
        let (nu, nv) = (self.ctrl.len(), self.ctrl[0].len());
        let span = |k: &[f64], deg: usize, n: usize, t: f64| {
            let mut s = deg;
            while s + 1 < n && k[s + 1] <= t {
                s += 1;
            }
            s
        };
        let su = span(&self.ku, self.du, nu, u);
        let sv = span(&self.kv, self.dv, nv, v);
        let mut col: Vec<(DVec3, f64)> = Vec::with_capacity(self.du + 1);
        for i in su - self.du..=su {
            let row: Vec<(DVec3, f64)> =
                (sv - self.dv..=sv).map(|j| (self.ctrl[i][j] * self.weights[i][j], self.weights[i][j])).collect();
            col.push(de_boor(&self.kv, self.dv, sv, row, v));
        }
        let (p, w) = de_boor(&self.ku, self.du, su, col, u);
        if w.abs() > 1e-300 {
            p / w
        } else {
            DVec3::NAN
        }
    }

    /// The (u, v) of the surface point nearest `p`.
    fn inverse(&self, p: DVec3) -> (f64, f64) {
        let (mut u, mut v, _) = self
            .seeds
            .iter()
            .min_by(|a, b| (a.2 - p).length_squared().total_cmp(&(b.2 - p).length_squared()))
            .copied()
            .unwrap_or((0.0, 0.0, DVec3::ZERO));
        let ((u0, u1), (v0, v1)) = (self.u_range(), self.v_range());
        let (hu, hv) = ((u1 - u0) * 1e-6, (v1 - v0) * 1e-6);
        for _ in 0..20 {
            let s = self.eval(u, v);
            let (ua, ub) = ((u - hu).max(u0), (u + hu).min(u1));
            let (va, vb) = ((v - hv).max(v0), (v + hv).min(v1));
            let su = (self.eval(ub, v) - self.eval(ua, v)) / (ub - ua);
            let sv = (self.eval(u, vb) - self.eval(u, va)) / (vb - va);
            let r = p - s;
            let (a, b, c) = (su.dot(su), su.dot(sv), sv.dot(sv));
            let det = a * c - b * b;
            if det.abs() < 1e-300 {
                break;
            }
            let (ru, rv) = (su.dot(r), sv.dot(r));
            let (du, dv) = ((c * ru - b * rv) / det, (a * rv - b * ru) / det);
            u = (u + du).clamp(u0, u1);
            v = (v + dv).clamp(v0, v1);
            if du.abs() < hu && dv.abs() < hv {
                break;
            }
        }
        (u, v)
    }
}

/// One step of de Boor on homogeneous points `d` for span `k`.
fn de_boor(kv: &[f64], p: usize, k: usize, mut d: Vec<(DVec3, f64)>, t: f64) -> (DVec3, f64) {
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let den = kv[i + p + 1 - r] - kv[i];
            let a = if den.abs() < 1e-300 { 0.0 } else { (t - kv[i]) / den };
            d[j] = (d[j - 1].0 * (1.0 - a) + d[j].0 * a, d[j - 1].1 * (1.0 - a) + d[j].1 * a);
        }
    }
    d[p]
}

/// A surface a face can lie on.
#[derive(Clone, Debug)]
pub enum Surf {
    Cylinder {
        f: Frame,
        r: f64,
    },
    /// Radius `r` at v = 0, growing by `tan` per unit of v.
    Cone {
        f: Frame,
        r: f64,
        tan: f64,
    },
    Sphere {
        f: Frame,
        r: f64,
    },
    Torus {
        f: Frame,
        big: f64,
        r: f64,
    },
    BSpline(Box<BSplineSurface>),
    /// A curve moved along a direction: S(s, w) = curve(s) + dir * w.
    Extrusion {
        path: Path,
        dir: DVec3,
    },
    /// A curve turned about an axis: u is the angle, v the arc length.
    Revolution {
        path: Path,
        f: Frame,
    },
}

impl Surf {
    /// A surface of revolution of the curve `pts` about the frame's z axis.
    pub fn revolution(pts: &[DVec3], f: Frame) -> Option<Surf> {
        let flat = pts
            .iter()
            .map(|&p| {
                let l = f.local(p);
                DVec3::new((l.x * l.x + l.y * l.y).sqrt(), 0.0, l.z)
            })
            .collect();
        Some(Surf::Revolution { path: Path::new(flat)?, f })
    }

    pub fn eval(&self, u: f64, v: f64) -> DVec3 {
        match self {
            Surf::Cylinder { f, r } => f.o + f.z * v + f.radial(u) * *r,
            Surf::Cone { f, r, tan } => f.o + f.z * v + f.radial(u) * (r + v * tan),
            Surf::Sphere { f, r } => f.o + (f.radial(u) * v.cos() + f.z * v.sin()) * *r,
            Surf::Torus { f, big, r } => f.o + f.radial(u) * (big + r * v.cos()) + f.z * (r * v.sin()),
            Surf::BSpline(b) => b.eval(u, v),
            Surf::Extrusion { path, dir } => path.at(u) + *dir * v,
            Surf::Revolution { path, f } => {
                let q = path.at(v);
                f.o + f.radial(u) * q.x + f.z * q.z
            }
        }
    }

    pub fn inverse(&self, p: DVec3) -> (f64, f64) {
        match self {
            Surf::Cylinder { f, .. } | Surf::Cone { f, .. } => {
                let l = f.local(p);
                (l.y.atan2(l.x), l.z)
            }
            Surf::Sphere { f, r } => {
                let l = f.local(p);
                (l.y.atan2(l.x), (l.z / r).clamp(-1.0, 1.0).asin())
            }
            Surf::Torus { f, big, .. } => {
                let l = f.local(p);
                let rho = (l.x * l.x + l.y * l.y).sqrt() - big;
                (l.y.atan2(l.x), l.z.atan2(rho))
            }
            Surf::BSpline(b) => b.inverse(p),
            Surf::Extrusion { path, dir } => {
                // Along the path in the plane across `dir`, then along `dir`.
                let d = dir.normalize_or_zero();
                let across = |q: DVec3| q - d * q.dot(d);
                let s = path.nearest(across(p), across);
                (s, (p - path.at(s)).dot(*dir) / dir.length_squared())
            }
            Surf::Revolution { path, f } => {
                // v: the profile point at the same radius and height.
                let l = f.local(p);
                let v = path.nearest(DVec3::new((l.x * l.x + l.y * l.y).sqrt(), 0.0, l.z), |q| q);
                (l.y.atan2(l.x), v)
            }
        }
    }

    /// Period of u and of v, for angles.
    pub fn periods(&self) -> (Option<f64>, Option<f64>) {
        match self {
            Surf::Cylinder { .. } | Surf::Cone { .. } | Surf::Sphere { .. } | Surf::Revolution { .. } => {
                (Some(TAU), None)
            }
            Surf::Torus { .. } => (Some(TAU), Some(TAU)),
            Surf::BSpline(_) | Surf::Extrusion { .. } => (None, None),
        }
    }

    /// Millimetres per unit of u and of v, near the face, so that a
    /// triangulation in the scaled plane has fair shapes in space.
    pub fn metric(&self, mid: (f64, f64)) -> (f64, f64) {
        match self {
            Surf::Cylinder { r, .. } => (*r, 1.0),
            Surf::Cone { r, tan, .. } => ((r + mid.1 * tan).abs().max(1e-6), (1.0 + tan * tan).sqrt()),
            Surf::Sphere { r, .. } => (r * mid.1.cos().abs().max(0.05), *r),
            Surf::Torus { big, r, .. } => ((big + r * mid.1.cos()).abs().max(1e-6), *r),
            Surf::Extrusion { dir, .. } => (1.0, dir.length()),
            Surf::Revolution { path, .. } => (path.at(mid.1).x.abs().max(1e-6), 1.0),
            Surf::BSpline(_) => {
                let (u, v) = mid;
                let h = 1e-4;
                let su = (self.eval(u + h, v) - self.eval(u - h, v)).length() / (2.0 * h);
                let sv = (self.eval(u, v + h) - self.eval(u, v - h)).length() / (2.0 * h);
                (su.max(1e-9), sv.max(1e-9))
            }
        }
    }

    /// True when the surface bends both ways, so its faces need points
    /// inside, not just on the boundary. A cylinder, a cone and an
    /// extrusion are straight along v.
    pub fn doubly_curved(&self) -> bool {
        !matches!(self, Surf::Cylinder { .. } | Surf::Cone { .. } | Surf::Extrusion { .. })
    }

    /// The v of the poles, where a whole row of u meets in one point: the
    /// two ends of a sphere, the tip of a cone.
    pub fn poles(&self) -> Vec<f64> {
        match self {
            Surf::Sphere { .. } => vec![-FRAC_PI_2, FRAC_PI_2],
            Surf::Cone { r, tan, .. } if tan.abs() > 1e-12 => vec![-r / tan],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame { o: DVec3::new(1.0, 2.0, 3.0), z: DVec3::Z, x: DVec3::X }
    }

    #[test]
    fn inverse_undoes_eval() {
        let path =
            Path::new(vec![DVec3::new(5.0, 0.0, 0.0), DVec3::new(7.0, 0.0, 4.0), DVec3::new(6.0, 0.0, 9.0)]).unwrap();
        let surfs = [
            Surf::Cylinder { f: frame(), r: 4.0 },
            Surf::Cone { f: frame(), r: 4.0, tan: 0.3 },
            Surf::Sphere { f: frame(), r: 4.0 },
            Surf::Torus { f: frame(), big: 10.0, r: 3.0 },
            Surf::revolution(path.points(), frame()).unwrap(),
        ];
        for s in &surfs {
            for (u, v) in [(0.3, 0.2), (-2.0, 1.1), (2.5, 0.7)] {
                let p = s.eval(u, v);
                let (a, b) = s.inverse(p);
                let q = s.eval(a, b);
                assert!((p - q).length() < 1e-6, "{s:?} at ({u}, {v}): {p} came back as {q}");
            }
        }
    }

    #[test]
    fn a_bilinear_bspline_is_a_flat_patch() {
        let ctrl = vec![
            vec![DVec3::ZERO, DVec3::new(0.0, 10.0, 0.0)],
            vec![DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 0.0)],
        ];
        let b = BSplineSurface::new(1, 1, ctrl, None, (&[2.0, 2.0], &[0.0, 1.0]), (&[2.0, 2.0], &[0.0, 1.0])).unwrap();
        let p = b.eval(0.25, 0.5);
        assert!((p - DVec3::new(2.5, 5.0, 0.0)).length() < 1e-12);
        let (u, v) = b.inverse(DVec3::new(7.5, 2.0, 0.0));
        assert!((u - 0.75).abs() < 1e-6 && (v - 0.2).abs() < 1e-6, "{u} {v}");
    }
}
