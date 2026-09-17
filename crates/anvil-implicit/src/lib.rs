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

pub mod mesh;
pub mod sampled;
pub use sampled::Sampled;

/// A scalar field over space. Negative inside, positive outside, zero on
/// the surface. Values near the surface should be close to a distance.
pub trait Field: Sync {
    fn at(&self, p: DVec3) -> f64;
}

impl<F: Field> Field for &F {
    fn at(&self, p: DVec3) -> f64 {
        (**self).at(p)
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
}

/// Union: the nearer surface wins.
pub struct Union<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Union<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).min(self.1.at(p))
    }
}

/// Intersection.
pub struct Intersect<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Intersect<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).max(self.1.at(p))
    }
}

/// `A` minus `B`.
pub struct Subtract<A, B>(pub A, pub B);
impl<A: Field, B: Field> Field for Subtract<A, B> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).max(-self.1.at(p))
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
impl Field for Lattice {
    fn at(&self, p: DVec3) -> f64 {
        let w = std::f64::consts::TAU / self.cell;
        let (x, y, z) = (p.x * w, p.y * w, p.z * w);
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        // (level set value, typical |gradient| in the scaled coordinates)
        let (g, grad) = match self.kind {
            Tpms::Gyroid => (sx * cy + sy * cz + sz * cx, 1.2),
            Tpms::SchwarzP => (cx + cy + cz, 1.2),
            Tpms::Diamond => (sx * sy * sz + sx * cy * cz + cx * sy * cz + cx * cy * sz, 0.9),
        };
        g.abs() / (grad * w) - self.wall * 0.5
    }
}

/// A field behind a trait object, so features can build one at run time.
pub struct Dyn(pub Box<dyn Field>);
impl Field for Dyn {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p)
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
        // Walking away from the sheet, the value grows about linearly.
        let d = l.at(DVec3::new(0.0, 0.0, 0.3)) - l.at(DVec3::ZERO);
        assert!(d > 0.15 && d < 0.45, "{d}");
    }
}
