//! Aircraft geometry as fields: NACA four digit airfoils, a lofted wing,
//! a fuselage of revolution, and rigid transforms to place tails.
//!
//! The wing is a loft: for a point, find the span station, take the
//! section at that station (chord, twist, sweep, dihedral, airfoil
//! interpolated between root and tip), map the point into the section's
//! chord frame, and evaluate the 2D airfoil distance scaled by the chord.
//! Beyond the tip the distance grows with the overhang, so the tip closes.
//! The result is close to a distance near the surface, which is what the
//! mesher needs.

use crate::Field;
use anvil_math::{DVec2, DVec3};

/// NACA four digit airfoil `MPTT`: camber `m` (percent chord), camber
/// position `p` (tenths of chord), thickness `t` (percent chord).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Naca4 {
    pub m: f64,
    pub p: f64,
    pub t: f64,
}

impl Naca4 {
    /// Parse "2412" into m 0.02, p 0.4, t 0.12.
    pub fn parse(code: &str) -> Option<Naca4> {
        let d: Vec<u32> = code.trim().chars().map(|c| c.to_digit(10)).collect::<Option<Vec<_>>>()?;
        if d.len() != 4 {
            return None;
        }
        Some(Naca4 { m: d[0] as f64 / 100.0, p: d[1] as f64 / 10.0, t: (d[2] * 10 + d[3]) as f64 / 100.0 })
    }

    /// Half thickness at chord fraction `x`, closed trailing edge.
    pub fn thickness(&self, x: f64) -> f64 {
        5.0 * self.t * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x * x * x - 0.1036 * x * x * x * x)
    }

    /// Camber line height and slope at chord fraction `x`.
    pub fn camber(&self, x: f64) -> (f64, f64) {
        let (m, p) = (self.m, self.p);
        if m <= 0.0 || p <= 0.0 {
            return (0.0, 0.0);
        }
        if x < p {
            (m / (p * p) * (2.0 * p * x - x * x), 2.0 * m / (p * p) * (p - x))
        } else {
            let q = 1.0 - p;
            (m / (q * q) * (1.0 - 2.0 * p + 2.0 * p * x - x * x), 2.0 * m / (q * q) * (p - x))
        }
    }

    /// Closed outline in chord units, counter clockwise from the trailing
    /// edge over the upper surface to the leading edge and back along the
    /// lower surface. `n` points per side, clustered at the leading edge.
    pub fn outline(&self, n: usize) -> Vec<DVec2> {
        self.outline_blunt(n, 0.0)
    }

    /// Outline with a blunt trailing edge of thickness `te` (chord
    /// fraction), added as a ramp from zero at the leading edge, so the
    /// trailing edge is a short flat that a mesher and a printer resolve.
    pub fn outline_blunt(&self, n: usize, te: f64) -> Vec<DVec2> {
        let n = n.max(8);
        let mut pts = Vec::with_capacity(2 * n);
        let station = |k: usize| 0.5 * (1.0 - (std::f64::consts::PI * k as f64 / n as f64).cos());
        for k in (0..=n).rev() {
            let x = station(k);
            let yt = self.thickness(x) + 0.5 * te * x;
            let (yc, dy) = self.camber(x);
            let th = dy.atan();
            pts.push(DVec2::new(x - yt * th.sin(), yc + yt * th.cos()));
        }
        for k in 1..=n {
            let x = station(k);
            let yt = self.thickness(x) + 0.5 * te * x;
            let (yc, dy) = self.camber(x);
            let th = dy.atan();
            pts.push(DVec2::new(x + yt * th.sin(), yc - yt * th.cos()));
        }
        if te <= 0.0 {
            // Sharp trailing edge: the last lower point equals the first upper point.
            pts.pop();
        }
        pts
    }

    /// Zero lift angle from thin airfoil theory, in radians (negative for
    /// positive camber).
    pub fn zero_lift_angle(&self) -> f64 {
        // alpha_L0 = -(1/pi) * integral_0^pi dz/dx (cos(theta) - 1) dtheta
        let n = 400;
        let mut sum = 0.0;
        for i in 0..n {
            let th = std::f64::consts::PI * (i as f64 + 0.5) / n as f64;
            let x = 0.5 * (1.0 - th.cos());
            let (_, dz) = self.camber(x);
            sum += dz * (th.cos() - 1.0);
        }
        -sum * (std::f64::consts::PI / n as f64) / std::f64::consts::PI
    }
}

/// Signed distance to a closed 2D polygon, negative inside.
pub fn polygon_distance(pts: &[DVec2], p: DVec2) -> f64 {
    let n = pts.len();
    let mut d2 = f64::INFINITY;
    let mut inside = false;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let e = b - a;
        let t = ((p - a).dot(e) / e.length_squared().max(1e-18)).clamp(0.0, 1.0);
        d2 = d2.min((p - (a + e * t)).length_squared());
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    let d = d2.sqrt();
    if inside {
        -d
    } else {
        d
    }
}

/// A lofted wing. Root at y = 0, tips at y = plus and minus half the
/// span, chord along +x from the leading edge, lift along +z. Angles in
/// degrees. The quarter chord line carries sweep and dihedral; twist is
/// about the quarter chord, positive nose up.
#[derive(Clone, Debug)]
pub struct Wing {
    pub span: f64,
    pub root_chord: f64,
    pub tip_chord: f64,
    /// Leading edge sweep, degrees.
    pub sweep: f64,
    pub dihedral: f64,
    pub twist_root: f64,
    pub twist_tip: f64,
    pub root: Naca4,
    pub tip: Naca4,
    /// Trailing edge thickness in world units (0 for a sharp edge).
    pub te_thickness: f64,
    /// Precomputed section outlines from root to tip.
    sections: Vec<Vec<DVec2>>,
}

impl Wing {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        span: f64,
        root_chord: f64,
        tip_chord: f64,
        sweep: f64,
        dihedral: f64,
        twist_root: f64,
        twist_tip: f64,
        root: Naca4,
        tip: Naca4,
    ) -> Wing {
        Wing::with_trailing_edge(span, root_chord, tip_chord, sweep, dihedral, twist_root, twist_tip, root, tip, 0.0)
    }

    /// As `new`, with a blunt trailing edge of `te_thickness` (world
    /// units) at every station.
    #[allow(clippy::too_many_arguments)]
    pub fn with_trailing_edge(
        span: f64,
        root_chord: f64,
        tip_chord: f64,
        sweep: f64,
        dihedral: f64,
        twist_root: f64,
        twist_tip: f64,
        root: Naca4,
        tip: Naca4,
        te_thickness: f64,
    ) -> Wing {
        let stations = 9;
        let sections = (0..stations)
            .map(|k| {
                let eta = k as f64 / (stations - 1) as f64;
                let a = Naca4 {
                    m: root.m + (tip.m - root.m) * eta,
                    p: root.p + (tip.p - root.p) * eta,
                    t: root.t + (tip.t - root.t) * eta,
                };
                let c = root_chord + (tip_chord - root_chord) * eta;
                a.outline_blunt(48, (te_thickness / c.max(1e-9)).max(0.0))
            })
            .collect();
        Wing { span, root_chord, tip_chord, sweep, dihedral, twist_root, twist_tip, root, tip, te_thickness, sections }
    }

    pub fn chord(&self, eta: f64) -> f64 {
        self.root_chord + (self.tip_chord - self.root_chord) * eta
    }

    pub fn area(&self) -> f64 {
        0.5 * (self.root_chord + self.tip_chord) * self.span
    }

    pub fn aspect_ratio(&self) -> f64 {
        self.span * self.span / self.area().max(1e-12)
    }

    pub fn taper(&self) -> f64 {
        self.tip_chord / self.root_chord.max(1e-12)
    }

    /// Twist at span fraction `eta`, degrees.
    pub fn twist(&self, eta: f64) -> f64 {
        self.twist_root + (self.twist_tip - self.twist_root) * eta
    }

    /// Distance in the section plane at span fraction `eta` (0 to 1), for
    /// a point given by its x and z, in world units.
    fn section_distance(&self, eta: f64, x: f64, z: f64) -> f64 {
        let half = 0.5 * self.span;
        let c = self.chord(eta);
        // Quarter chord point of this station.
        let le_x = eta * half * self.sweep.to_radians().tan();
        let le_z = eta * half * self.dihedral.to_radians().tan();
        let qx = le_x + 0.25 * c;
        // Into the twisted chord frame: nose up twist rotates the section
        // leading edge upward, so untwist the point the other way.
        let th = -self.twist(eta).to_radians();
        let (sn, cs) = th.sin_cos();
        let dx = x - qx;
        let dz = z - le_z;
        let u = (dx * cs - dz * sn) / c + 0.25;
        let w = (dx * sn + dz * cs) / c;
        // Blend the two nearest precomputed sections.
        let f = eta * (self.sections.len() - 1) as f64;
        let k = (f.floor() as usize).min(self.sections.len() - 2);
        let t = f - k as f64;
        let p = DVec2::new(u, w);
        let d0 = polygon_distance(&self.sections[k], p);
        let d1 = polygon_distance(&self.sections[k + 1], p);
        (d0 + (d1 - d0) * t) * c
    }
}

impl Field for Wing {
    fn at(&self, p: DVec3) -> f64 {
        let half = 0.5 * self.span;
        let y = p.y.abs();
        let eta = (y / half).min(1.0);
        let d = self.section_distance(eta, p.x, p.z);
        if y > half {
            let over = y - half;
            if d > 0.0 {
                (d * d + over * over).sqrt()
            } else {
                over
            }
        } else {
            d
        }
    }
}

/// A body of revolution about the X axis from a radius profile
/// `(x, r)`, closed at both ends.
#[derive(Clone, Debug)]
pub struct Fuselage {
    outline: Vec<DVec2>,
}

impl Fuselage {
    /// From a radius profile in order of increasing x.
    pub fn from_profile(profile: &[(f64, f64)]) -> Fuselage {
        let mut outline: Vec<DVec2> = profile.iter().map(|&(x, r)| DVec2::new(x, r)).collect();
        // Close along the axis, far below r = 0 so the axis is inside.
        let (x0, x1) = (profile.first().map(|p| p.0).unwrap_or(0.0), profile.last().map(|p| p.0).unwrap_or(1.0));
        let depth = profile.iter().map(|p| p.1).fold(0.0, f64::max) * 2.0 + 1.0;
        outline.push(DVec2::new(x1, -depth));
        outline.push(DVec2::new(x0, -depth));
        Fuselage { outline }
    }

    /// A smooth pointed body: length `len`, maximum radius `r`, nose at
    /// x = 0. Sears-Haack like, `r * (4 x (1 - x))^0.75`.
    pub fn sears_haack(len: f64, r: f64) -> Fuselage {
        let n = 40;
        let profile: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let x = i as f64 / n as f64;
                (x * len, r * (4.0 * x * (1.0 - x)).max(0.0).powf(0.75))
            })
            .collect();
        Fuselage::from_profile(&profile)
    }
}

impl Field for Fuselage {
    fn at(&self, p: DVec3) -> f64 {
        let rho = (p.y * p.y + p.z * p.z).sqrt();
        polygon_distance(&self.outline, DVec2::new(p.x, rho))
    }
}

/// A field moved by a rigid transform: rotation about X by `roll`, then
/// translation. Distances are preserved.
pub struct Placed<F> {
    pub field: F,
    pub roll_deg: f64,
    pub at: DVec3,
}

impl<F: Field> Field for Placed<F> {
    fn at(&self, p: DVec3) -> f64 {
        let q = p - self.at;
        let (s, c) = (-self.roll_deg.to_radians()).sin_cos();
        let local = DVec3::new(q.x, q.y * c - q.z * s, q.y * s + q.z * c);
        self.field.at(local)
    }
}

/// Mirror a field across the XZ plane and keep both halves (for a single
/// vertical fin, use the field alone).
pub struct MirrorY<F>(pub F);
impl<F: Field> Field for MirrorY<F> {
    fn at(&self, p: DVec3) -> f64 {
        self.0.at(p).min(self.0.at(DVec3::new(p.x, -p.y, p.z)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naca_2412_has_the_textbook_numbers() {
        let a = Naca4::parse("2412").unwrap();
        assert!((a.t - 0.12).abs() < 1e-12 && (a.m - 0.02).abs() < 1e-12 && (a.p - 0.4).abs() < 1e-12);
        // Maximum half thickness near 30 percent chord, about 6 percent.
        let yt = a.thickness(0.3);
        assert!((yt - 0.06).abs() < 0.002, "{yt}");
        // Closed trailing edge.
        assert!(a.thickness(1.0).abs() < 1e-3);
        // Zero lift angle of the 2412 is about minus 2.1 degrees.
        let a0 = a.zero_lift_angle().to_degrees();
        assert!((a0 + 2.1).abs() < 0.3, "{a0}");
        let outline = a.outline(40);
        assert!(polygon_distance(&outline, DVec2::new(0.3, 0.0)) < 0.0, "chord line is inside");
        assert!(polygon_distance(&outline, DVec2::new(0.3, 0.2)) > 0.0);
    }

    #[test]
    fn wing_distance_is_negative_inside_and_closes_at_the_tip() {
        let w = Wing::new(
            10.0,
            2.0,
            1.0,
            10.0,
            3.0,
            2.0,
            -1.0,
            Naca4::parse("2412").unwrap(),
            Naca4::parse("0012").unwrap(),
        );
        assert!(w.at(DVec3::new(0.6, 0.0, 0.0)) < 0.0, "inside the root section");
        assert!(w.at(DVec3::new(0.6, 0.0, 1.0)) > 0.0, "above the wing");
        // Past the tip the field is positive and grows with the overhang.
        let a = w.at(DVec3::new(1.2, 5.5, 0.15));
        let b = w.at(DVec3::new(1.2, 6.5, 0.15));
        assert!(a > 0.0 && b > a);
        assert!((w.area() - 15.0).abs() < 1e-9 && (w.aspect_ratio() - 100.0 / 15.0).abs() < 1e-9);
    }

    #[test]
    fn trailing_edge_is_straight_between_stations() {
        // Rectangular, untwisted, unswept wing whose airfoil changes from
        // root to tip: the trailing edge must stay at x = chord all along.
        let w =
            Wing::new(10.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, Naca4::parse("2412").unwrap(), Naca4::parse("0012").unwrap());
        let mut worst = 0.0f64;
        for k in 0..=100 {
            let y = 5.0 * k as f64 / 100.0;
            // Find the zero of the field along x at the camber line height
            // of the trailing edge, z = 0, by bisection between 0.9 and 1.3.
            let (mut a, mut b) = (0.9, 1.3);
            for _ in 0..50 {
                let m = 0.5 * (a + b);
                if w.at(DVec3::new(m, y, 0.0)) < 0.0 {
                    a = m;
                } else {
                    b = m;
                }
            }
            let x_te = 0.5 * (a + b);
            worst = worst.max((x_te - 1.0).abs());
            if k % 10 == 0 {
                eprintln!(
                    "eta {:.2}: x_te {:.4}, at 0.98: {:.4}, at 1.02: {:.4}",
                    y / 5.0,
                    x_te,
                    w.at(DVec3::new(0.98, y, 0.0)),
                    w.at(DVec3::new(1.02, y, 0.0))
                );
            }
        }
        assert!(worst < 0.01, "trailing edge wanders by {worst}");
    }

    #[test]
    fn fuselage_is_inside_on_the_axis() {
        let f = Fuselage::sears_haack(20.0, 1.5);
        assert!(f.at(DVec3::new(10.0, 0.0, 0.0)) < -1.4);
        assert!(f.at(DVec3::new(10.0, 0.0, 2.0)) > 0.4);
        assert!(f.at(DVec3::new(-1.0, 0.0, 0.0)) > 0.9, "in front of the nose");
    }
}
