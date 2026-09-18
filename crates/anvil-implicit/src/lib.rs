//! Implicit modelling for Anvil: a shape is a function of space, negative
//! inside and positive outside, and the surface is where it is zero. This
//! is the representation behind field driven tools such as nTop. Lattices,
//! shells, smooth blends, and booleans are one line each on a field,
//! and they never fail the way a B-rep boolean can.
//!
//! The crate is independent of the B-rep kernel. `sampled::Sampled` turns
//! a triangle mesh into a field, `mesh::surface_nets` turns any field back
//! into triangles, and `anvil-feature` wraps both in features.
//!
//! Distances are approximate: a lattice function is scaled to be close to
//! a distance near its surface, and a sampled field is exact only at the
//! grid resolution. That is enough for meshing at the same resolution.

use anvil_math::DVec3;

pub mod aero;
pub mod beam;
pub mod expr;
pub mod mesh;
pub mod pointmap;
pub mod sampled;
pub mod slice;
pub mod vtk;
pub mod wing;
pub use beam::{cylindrical, BeamCell, BeamLattice, Graded, Honeycomb, Radial, Ramp, Warp};
// CellRamp and Blend live in this file.
pub use pointmap::{PointMap, Remap};
pub use sampled::Sampled;
pub use vtk::{GridField, Threshold};

/// A scalar field over space. Negative inside, positive outside, zero on
/// the surface. Values near the surface should be close to a distance.
pub trait Field: Sync {
    fn at(&self, p: DVec3) -> f64;

    /// Gradient of the field, by central differences unless a type knows
    /// better. Near the surface of a distance field this is the unit
    /// normal.
    fn grad(&self, p: DVec3) -> DVec3 {
        let h = 1e-4 * (1.0 + p.length());
        DVec3::new(
            self.at(p + DVec3::new(h, 0.0, 0.0)) - self.at(p - DVec3::new(h, 0.0, 0.0)),
            self.at(p + DVec3::new(0.0, h, 0.0)) - self.at(p - DVec3::new(0.0, h, 0.0)),
            self.at(p + DVec3::new(0.0, 0.0, h)) - self.at(p - DVec3::new(0.0, 0.0, h)),
        ) / (2.0 * h)
    }
}

impl<F: Field> Field for &F {
    fn at(&self, p: DVec3) -> f64 {
        (**self).at(p)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        (**self).grad(p)
    }
}

impl<F: Field + ?Sized> Field for Box<F> {
    fn at(&self, p: DVec3) -> f64 {
        (**self).at(p)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        (**self).grad(p)
    }
}

impl<F: Field + Send + ?Sized> Field for std::sync::Arc<F> {
    fn at(&self, p: DVec3) -> f64 {
        (**self).at(p)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        (**self).grad(p)
    }
}

/// A closure as a field.
pub struct Func<F>(pub F);
impl<F: Fn(DVec3) -> f64 + Sync> Field for Func<F> {
    fn at(&self, p: DVec3) -> f64 {
        (self.0)(p)
    }
}

/// Sphere of radius `r` about `c`.
pub struct Sphere {
    pub c: DVec3,
    pub r: f64,
}
impl Field for Sphere {
    fn at(&self, p: DVec3) -> f64 {
        (p - self.c).length() - self.r
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        (p - self.c).normalize_or_zero()
    }
}

/// Axis aligned box from `lo` to `hi`.
pub struct BoxField {
    pub lo: DVec3,
    pub hi: DVec3,
}
impl Field for BoxField {
    fn at(&self, p: DVec3) -> f64 {
        let c = (self.lo + self.hi) * 0.5;
        let h = (self.hi - self.lo) * 0.5;
        let q = (p - c).abs() - h;
        q.max(DVec3::ZERO).length() + q.x.max(q.y).max(q.z).min(0.0)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        let c = (self.lo + self.hi) * 0.5;
        let h = (self.hi - self.lo) * 0.5;
        let d = p - c;
        let q = d.abs() - h;
        let sign = DVec3::new(d.x.signum(), d.y.signum(), d.z.signum());
        let outside = q.max(DVec3::ZERO);
        if outside.length_squared() > 0.0 {
            (outside * sign).normalize()
        } else if q.x >= q.y && q.x >= q.z {
            DVec3::new(sign.x, 0.0, 0.0)
        } else if q.y >= q.z {
            DVec3::new(0.0, sign.y, 0.0)
        } else {
            DVec3::new(0.0, 0.0, sign.z)
        }
    }
}

/// Union: the nearer surface wins.
pub struct Union<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Union<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).min(self.1.at(p))
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        if self.0.at(p) <= self.1.at(p) {
            self.0.grad(p)
        } else {
            self.1.grad(p)
        }
    }
}

/// Intersection.
pub struct Intersect<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Intersect<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).max(self.1.at(p))
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        if self.0.at(p) >= self.1.at(p) {
            self.0.grad(p)
        } else {
            self.1.grad(p)
        }
    }
}

/// `A` minus `B`.
pub struct Subtract<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Subtract<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).max(-self.1.at(p))
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        if self.0.at(p) >= -self.1.at(p) {
            self.0.grad(p)
        } else {
            -self.1.grad(p)
        }
    }
}

/// Union with a fillet of radius about `k` between the two shapes.
pub struct SmoothUnion<A, B> {
    pub a: A,
    pub b: B,
    pub k: f64,
}
impl<A: Field, B: Field> Field for SmoothUnion<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        let (a, b) = (self.a.at(p), self.b.at(p));
        let h = (0.5 + 0.5 * (b - a) / self.k).clamp(0.0, 1.0);
        b + (a - b) * h - self.k * h * (1.0 - h)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        let (a, b) = (self.a.at(p), self.b.at(p));
        let h = (0.5 + 0.5 * (b - a) / self.k).clamp(0.0, 1.0);
        // The blend weight varies slowly; the mix of the two gradients is
        // close enough for a normal.
        (self.a.grad(p) * h + self.b.grad(p) * (1.0 - h)).normalize_or_zero()
    }
}

/// Grow (positive) or shrink (negative) a shape by `d`.
pub struct Offset<A> {
    pub a: A,
    pub d: f64,
}
impl<A: Field> Field for Offset<A> {
    fn at(&self, p: DVec3) -> f64 {
        self.a.at(p) - self.d
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        self.a.grad(p)
    }
}

/// The skin of a shape: material within `t / 2` of its surface.
pub struct Shell<A> {
    pub a: A,
    pub t: f64,
}
impl<A: Field> Field for Shell<A> {
    fn at(&self, p: DVec3) -> f64 {
        self.a.at(p).abs() - self.t * 0.5
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        self.a.grad(p) * self.a.at(p).signum()
    }
}

/// Material within `t` inside the surface of a shape (an inward shell).
pub struct Skin<A> {
    pub a: A,
    pub t: f64,
}
impl<A: Field> Field for Skin<A> {
    fn at(&self, p: DVec3) -> f64 {
        let d = self.a.at(p);
        d.max(-(d + self.t))
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        let d = self.a.at(p);
        if d >= -(d + self.t) {
            self.a.grad(p)
        } else {
            -self.a.grad(p)
        }
    }
}

/// Triply periodic minimal surface families for sheet lattices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tpms {
    Gyroid,
    SchwarzP,
    Diamond,
}

impl Tpms {
    pub fn parse(s: &str) -> Option<Tpms> {
        match s.to_ascii_lowercase().as_str() {
            "gyroid" => Some(Tpms::Gyroid),
            "schwarz" | "schwarz p" | "schwarzp" | "primitive" => Some(Tpms::SchwarzP),
            "diamond" => Some(Tpms::Diamond),
            _ => None,
        }
    }
    pub const NAMES: [&'static str; 3] = ["gyroid", "schwarz", "diamond"];
}

/// A sheet lattice: the TPMS surface thickened to `wall`, repeating every
/// `cell` in each direction. The level set function is divided by its
/// typical gradient so the value is close to a distance near the sheet.
pub struct Lattice {
    pub kind: Tpms,
    pub cell: f64,
    pub wall: f64,
}
impl Tpms {
    /// Level set value and its gradient at scaled coordinates (one cell
    /// is a full turn of each coordinate).
    pub fn level_at(self, x: f64, y: f64, z: f64) -> (f64, DVec3) {
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        let (g, gx, gy, gz) = match self {
            Tpms::Gyroid => (sx * cy + sy * cz + sz * cx, cx * cy - sz * sx, cy * cz - sx * sy, cz * cx - sy * sz),
            Tpms::SchwarzP => (cx + cy + cz, -sx, -sy, -sz),
            Tpms::Diamond => (
                sx * sy * sz + sx * cy * cz + cx * sy * cz + cx * cy * sz,
                cx * sy * sz + cx * cy * cz - sx * sy * cz - sx * cy * sz,
                sx * cy * sz - sx * sy * cz + cx * cy * cz - cx * sy * sz,
                sx * sy * cz - sx * cy * sz - cx * sy * sz + cx * cy * cz,
            ),
        };
        (g, DVec3::new(gx, gy, gz))
    }
}

/// A sheet lattice whose cell size ramps linearly along one axis, from
/// `cell_a` at `from` to `cell_b` at `to` (clamped beyond). The phase
/// along that axis is the integral of `2 pi / cell`, so the local period
/// is the local cell size everywhere and the cells never tear; across
/// the other two axes the local cell size scales the coordinate.
pub struct CellRamp {
    pub kind: Tpms,
    pub axis: usize,
    pub from: f64,
    pub to: f64,
    pub cell_a: f64,
    pub cell_b: f64,
    pub wall: f64,
}

impl CellRamp {
    /// Local cell size, its slope, and the phase along the ramp axis at
    /// coordinate `t`.
    fn cell_and_phase(&self, t: f64) -> (f64, f64, f64) {
        use std::f64::consts::TAU;
        let (a, b) = (self.from, self.to);
        let span = (b - a).abs().max(1e-12);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let (ca, cb) = if a <= b { (self.cell_a, self.cell_b) } else { (self.cell_b, self.cell_a) };
        let k = (cb - ca) / span;
        let phase_in = |u: f64| -> f64 {
            if k.abs() < 1e-12 {
                TAU * (u - lo) / ca
            } else {
                TAU / k * ((ca + k * (u - lo)) / ca).ln()
            }
        };
        if t < lo {
            (ca, 0.0, TAU * (t - lo) / ca)
        } else if t > hi {
            (cb, 0.0, phase_in(hi) + TAU * (t - hi) / cb)
        } else {
            (ca + k * (t - lo), k, phase_in(t))
        }
    }

    /// Level set value and its world gradient.
    fn level(&self, p: DVec3) -> (f64, DVec3) {
        use std::f64::consts::TAU;
        let c = [p.x, p.y, p.z];
        let (cell, slope, phase) = self.cell_and_phase(c[self.axis]);
        let w = TAU / cell;
        // d(w)/d(axis coordinate): the other coordinates are scaled by w.
        let dw = -TAU * slope / (cell * cell);
        let mut u = [c[0] * w, c[1] * w, c[2] * w];
        u[self.axis] = phase;
        let (g, gu) = self.kind.level_at(u[0], u[1], u[2]);
        let gu = [gu.x, gu.y, gu.z];
        // Chain rule: along the ramp axis the phase moves at w and the
        // other scaled coordinates drift at c_i * dw; across it, plain w.
        let mut grad = [0.0; 3];
        for i in 0..3 {
            if i == self.axis {
                let mut d = gu[i] * w;
                for j in 0..3 {
                    if j != self.axis {
                        d += gu[j] * c[j] * dw;
                    }
                }
                grad[i] = d;
            } else {
                grad[i] = gu[i] * w;
            }
        }
        (g, DVec3::new(grad[0], grad[1], grad[2]))
    }
}

impl Field for CellRamp {
    fn at(&self, p: DVec3) -> f64 {
        let (g, gr) = self.level(p);
        let mag = (gr.length_squared() + 0.001).sqrt();
        g.abs() / mag - self.wall * 0.5
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        let (g, gr) = self.level(p);
        (gr * g.signum()).normalize_or_zero()
    }
}

/// A mix of two fields by a weight field (0 gives `a`, 1 gives `b`),
/// for a transition between two lattices or two cell sizes. Both sides
/// stay exact; pick commensurate cells (one twice the other) so the
/// transition reads as a subdivision.
pub struct Blend<A, B, W> {
    pub a: A,
    pub b: B,
    pub weight: W,
}

impl<A: Field, B: Field, W: Field> Field for Blend<A, B, W> {
    fn at(&self, p: DVec3) -> f64 {
        let w = self.weight.at(p).clamp(0.0, 1.0);
        if w <= 0.0 {
            self.a.at(p)
        } else if w >= 1.0 {
            self.b.at(p)
        } else {
            self.a.at(p) * (1.0 - w) + self.b.at(p) * w
        }
    }
}

impl Lattice {
    /// Level set value and its gradient in the scaled coordinates.
    fn level(&self, p: DVec3) -> (f64, DVec3, f64) {
        let w = std::f64::consts::TAU / self.cell;
        let (x, y, z) = (p.x * w, p.y * w, p.z * w);
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        let (g, gx, gy, gz) = match self.kind {
            Tpms::Gyroid => (sx * cy + sy * cz + sz * cx, cx * cy - sz * sx, cy * cz - sx * sy, cz * cx - sy * sz),
            Tpms::SchwarzP => (cx + cy + cz, -sx, -sy, -sz),
            Tpms::Diamond => (
                sx * sy * sz + sx * cy * cz + cx * sy * cz + cx * cy * sz,
                cx * sy * sz + cx * cy * cz - sx * sy * cz - sx * cy * sz,
                sx * cy * sz - sx * sy * cz + cx * cy * cz - cx * sy * sz,
                sx * sy * cz - sx * cy * sz - cx * sy * sz + cx * cy * cz,
            ),
        };
        (g, DVec3::new(gx, gy, gz), w)
    }
}

impl Field for Lattice {
    fn at(&self, p: DVec3) -> f64 {
        // Dividing by the gradient magnitude (Taubin's first order
        // distance) keeps the wall thickness even across the cell.
        // A Newton step from the foot of this estimate would sharpen it
        // but jumps between sheets where the wall is thick against the
        // cell, so it is not used: keep the wall under a fifth of the cell.
        let (g, gr, w) = self.level(p);
        let mag = (gr.length_squared() + 0.05 * 0.05).sqrt() * w;
        g.abs() / mag - self.wall * 0.5
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        let (g, gr, _) = self.level(p);
        (gr * g.signum()).normalize_or_zero()
    }
}

/// A field behind a trait object, so features can build one at run time.
pub struct Dyn(pub Box<dyn Field>);
impl Field for Dyn {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p)
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        self.0.grad(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_ramp_keeps_the_local_period() {
        // Cell 4 at x = 0 growing to 12 at x = 60, sheet along x at y = z = 0.
        let r = CellRamp { kind: Tpms::SchwarzP, axis: 0, from: 0.0, to: 60.0, cell_a: 4.0, cell_b: 12.0, wall: 0.0 };
        // Zero crossings of the Schwarz P level along x at y = z = 0 (cos x + 2 = 0
        // has none), so probe the gyroid instead: at y = z = 0 the gyroid is sin x.
        let g = CellRamp { kind: Tpms::Gyroid, axis: 0, from: 0.0, to: 60.0, cell_a: 4.0, cell_b: 12.0, wall: 0.0 };
        let mut crossings = Vec::new();
        let mut prev = g.at(DVec3::new(-0.05, 0.0, 0.0)) - g.wall;
        let mut x = 0.0;
        while x < 70.0 {
            let v = g.kind.level_at(g.cell_and_phase(x).2, 0.0, 0.0).0;
            if prev.signum() != v.signum() {
                crossings.push(x);
            }
            prev = v;
            x += 0.01;
        }
        // Half period near the start is about 2, near the end about 6.
        let first = crossings[1] - crossings[0];
        let last = crossings[crossings.len() - 1] - crossings[crossings.len() - 2];
        assert!((first - 2.0).abs() < 0.3, "first half period {first}");
        assert!((last - 6.0).abs() < 0.4, "last half period {last}");
        assert!(r.at(DVec3::ZERO).is_finite());
    }

    #[test]
    fn cell_ramp_wall_thickness_is_even() {
        let r = CellRamp { kind: Tpms::Gyroid, axis: 0, from: 0.0, to: 60.0, cell_a: 5.0, cell_b: 15.0, wall: 1.0 };
        // Near the sheet the field must change at the rate of a distance:
        // gradient magnitude near one. Sample a lattice of points.
        for x in [2.0, 10.0, 30.0, 50.0, 58.0] {
            let mut mags = Vec::new();
            let mut y = -10.0;
            while y < 10.0 {
                let mut z = -10.0;
                while z < 10.0 {
                    let p = DVec3::new(x, y, z);
                    let v = r.at(p);
                    if v.abs() < 0.8 {
                        // Finite differences of the value, not the analytic
                        // unit normal, so the distance property is tested.
                        let h = 1e-4;
                        let fd = DVec3::new(
                            r.at(p + DVec3::new(h, 0.0, 0.0)) - r.at(p - DVec3::new(h, 0.0, 0.0)),
                            r.at(p + DVec3::new(0.0, h, 0.0)) - r.at(p - DVec3::new(0.0, h, 0.0)),
                            r.at(p + DVec3::new(0.0, 0.0, h)) - r.at(p - DVec3::new(0.0, 0.0, h)),
                        ) / (2.0 * h);
                        mags.push(fd.length());
                    }
                    z += 0.37;
                }
                y += 0.31;
            }
            let mean = mags.iter().sum::<f64>() / mags.len().max(1) as f64;
            let lo = mags.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = mags.iter().cloned().fold(0.0, f64::max);
            eprintln!(
                "x {x}: {} samples near the sheet, gradient magnitude mean {mean:.3}, min {lo:.3}, max {hi:.3}",
                mags.len()
            );
            // First order distance: within about half at a wall of a fifth
            // of the cell, better as the cells grow.
            assert!(mean > 0.8 && mean < 1.6, "gradient magnitude {mean} at x {x}");
        }
        // Volume in the two halves of a 60 x 20 x 20 box at two resolutions.
        use crate::mesh::{surface_nets, volume};
        for step in [0.5, 0.25] {
            let boxed = Intersect(&r, BoxField { lo: DVec3::ZERO, hi: DVec3::new(60.0, 20.0, 20.0) });
            let tris = surface_nets(&boxed, DVec3::splat(-1.0), DVec3::new(61.0, 21.0, 21.0), step);
            let (mut lo, mut hi) = (0.0, 0.0);
            for t in &tris {
                let v6 = t[0].dot(t[1].cross(t[2])) / 6.0;
                if (t[0].x + t[1].x + t[2].x) / 3.0 < 30.0 {
                    lo += v6;
                } else {
                    hi += v6;
                }
            }
            eprintln!("step {step}: small cell half {lo:.0}, large cell half {hi:.0}, total {:.0}", volume(&tris));
        }
    }

    #[test]
    fn sphere_and_box_are_signed() {
        let s = Sphere { c: DVec3::ZERO, r: 2.0 };
        assert!(s.at(DVec3::ZERO) < 0.0);
        assert!((s.at(DVec3::new(3.0, 0.0, 0.0)) - 1.0).abs() < 1e-12);
        let b = BoxField { lo: DVec3::splat(-1.0), hi: DVec3::splat(1.0) };
        assert!((b.at(DVec3::new(2.0, 0.0, 0.0)) - 1.0).abs() < 1e-12);
        assert!((b.at(DVec3::ZERO) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn lattice_wall_is_close_to_a_distance() {
        let l = Lattice { kind: Tpms::Gyroid, cell: 10.0, wall: 1.0 };
        // On the gyroid surface the value is minus half the wall.
        assert!((l.at(DVec3::ZERO) + 0.5).abs() < 1e-12);
        // Walking away from the sheet along its normal at the origin
        // (gradient there is (1, 1, 1)), the value grows like a distance.
        let n = DVec3::splat(1.0 / 3f64.sqrt());
        let d = l.at(n * 0.3) - l.at(DVec3::ZERO);
        assert!((d - 0.3).abs() < 0.03, "{d}");
        // Same check at another point of the sheet for each family.
        for kind in [Tpms::Gyroid, Tpms::SchwarzP, Tpms::Diamond] {
            let l = Lattice { kind, cell: 10.0, wall: 0.0 };
            let p = DVec3::new(2.5, 0.0, 0.0);
            // Move to the sheet along x by bisection.
            let (mut a, mut b) = (p, DVec3::new(7.5, 0.0, 0.0));
            let sheet_val = |q: DVec3| {
                let w = std::f64::consts::TAU / 10.0;
                let (x, y, z) = (q.x * w, q.y * w, q.z * w);
                match kind {
                    Tpms::Gyroid => x.sin() * y.cos() + y.sin() * z.cos() + z.sin() * x.cos(),
                    Tpms::SchwarzP => x.cos() + y.cos() + z.cos(),
                    Tpms::Diamond => {
                        x.sin() * y.sin() * z.sin()
                            + x.sin() * y.cos() * z.cos()
                            + x.cos() * y.sin() * z.cos()
                            + x.cos() * y.cos() * z.sin()
                    }
                }
            };
            if sheet_val(a).signum() == sheet_val(b).signum() {
                continue;
            }
            for _ in 0..60 {
                let m = (a + b) * 0.5;
                if sheet_val(m).signum() == sheet_val(a).signum() {
                    a = m;
                } else {
                    b = m;
                }
            }
            let on = (a + b) * 0.5;
            assert!(l.at(on).abs() < 1e-6, "{kind:?} on the sheet: {}", l.at(on));
            // A small step off the sheet reads as about that step.
            let h = 0.05;
            let off = on + DVec3::new(h, 0.0, 0.0);
            let ratio = l.at(off) / h;
            assert!(ratio > 0.3 && ratio <= 1.05, "{kind:?} ratio {ratio}");
        }
    }
}
