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
pub mod mesh;
pub mod pointmap;
pub mod sampled;
pub mod vtk;
pub mod wing;
pub use beam::{cylindrical, BeamCell, BeamLattice, Graded, Radial, Ramp, Warp};
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
        let (g, gr, w) = self.level(p);
        let mag = (gr.length_squared() + 0.05 * 0.05).sqrt();
        g.abs() / (mag * w) - self.wall * 0.5
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
