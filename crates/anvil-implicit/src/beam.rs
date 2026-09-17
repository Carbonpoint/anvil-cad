//! Beam lattices on a unit cell, graded thickness, and warps that make a
//! lattice follow a cylinder. Milestone F2 of the field driven plan.
//!
//! A beam lattice is the distance to the nearest beam centreline minus
//! the beam radius. The beams of one cell are listed once in unit cell
//! coordinates, together with every beam of the 26 neighbouring cells
//! that comes within reach of the cell, so the distance at any point is
//! a minimum over a short fixed list.

use crate::Field;
use anvil_math::DVec3;

/// Unit cells for beam lattices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamCell {
    /// Beams along the cube edges.
    Cubic,
    /// Cube edges plus the four body diagonals (body centred cubic).
    Bcc,
    /// Face centred cubic: face centre to the corners of its face and to
    /// the neighbouring face centres, the octet truss.
    Octet,
    /// Kelvin cell approximation: the octet without the outer cube edges,
    /// a lighter open cell.
    Kelvin,
}

impl BeamCell {
    pub fn parse(s: &str) -> Option<BeamCell> {
        match s.to_ascii_lowercase().as_str() {
            "cubic" | "cube" | "grid" => Some(BeamCell::Cubic),
            "bcc" => Some(BeamCell::Bcc),
            "octet" | "fcc" => Some(BeamCell::Octet),
            "kelvin" => Some(BeamCell::Kelvin),
            _ => None,
        }
    }
    pub const NAMES: [&'static str; 4] = ["cubic", "bcc", "octet", "kelvin"];

    /// Beams of one unit cell as segment end points in [0, 1]^3.
    fn segments(self) -> Vec<(DVec3, DVec3)> {
        let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
        let mut out = Vec::new();
        let edges = |out: &mut Vec<(DVec3, DVec3)>| {
            for a in [0.0, 1.0] {
                for b in [0.0, 1.0] {
                    out.push((v(0.0, a, b), v(1.0, a, b)));
                    out.push((v(a, 0.0, b), v(a, 1.0, b)));
                    out.push((v(a, b, 0.0), v(a, b, 1.0)));
                }
            }
        };
        match self {
            BeamCell::Cubic => edges(&mut out),
            BeamCell::Bcc => {
                edges(&mut out);
                let c = v(0.5, 0.5, 0.5);
                for x in [0.0, 1.0] {
                    for y in [0.0, 1.0] {
                        for z in [0.0, 1.0] {
                            out.push((c, v(x, y, z)));
                        }
                    }
                }
            }
            BeamCell::Octet | BeamCell::Kelvin => {
                if self == BeamCell::Octet {
                    edges(&mut out);
                }
                // Face centres.
                let faces = [
                    v(0.5, 0.5, 0.0),
                    v(0.5, 0.5, 1.0),
                    v(0.5, 0.0, 0.5),
                    v(0.5, 1.0, 0.5),
                    v(0.0, 0.5, 0.5),
                    v(1.0, 0.5, 0.5),
                ];
                // Face centre to its four corners.
                for f in faces {
                    for x in [0.0, 1.0] {
                        for y in [0.0, 1.0] {
                            for z in [0.0, 1.0] {
                                let corner = v(x, y, z);
                                let d = corner - f;
                                // A corner of this face has one coordinate equal
                                // to the face's fixed coordinate.
                                if d.abs().max_element() <= 0.5 + 1e-9 && (d.x == 0.0 || d.y == 0.0 || d.z == 0.0) {
                                    out.push((f, corner));
                                }
                            }
                        }
                    }
                }
                // Inner octahedron: adjacent face centres.
                for (i, a) in faces.iter().enumerate() {
                    for b in faces.iter().skip(i + 1) {
                        if (*a - *b).length() < 0.8 {
                            out.push((*a, *b));
                        }
                    }
                }
            }
        }
        out
    }
}

/// A beam lattice: `size` is the cell edge, `radius` the beam radius.
pub struct BeamLattice {
    pub cell: BeamCell,
    pub size: f64,
    pub radius: f64,
    /// Segments of the cell and of neighbours within reach, in cell units.
    segments: Vec<(DVec3, DVec3)>,
}

impl BeamLattice {
    pub fn new(cell: BeamCell, size: f64, radius: f64) -> BeamLattice {
        let base = cell.segments();
        let reach = (radius / size.max(1e-9)).min(0.5) + 1e-6;
        let mut segments = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let off = DVec3::new(dx as f64, dy as f64, dz as f64);
                    for (a, b) in &base {
                        let (a, b) = (*a + off, *b + off);
                        let lo = a.min(b);
                        let hi = a.max(b);
                        if hi.x >= -reach
                            && hi.y >= -reach
                            && hi.z >= -reach
                            && lo.x <= 1.0 + reach
                            && lo.y <= 1.0 + reach
                            && lo.z <= 1.0 + reach
                        {
                            segments.push((a, b));
                        }
                    }
                }
            }
        }
        // Drop duplicates (a cube edge is listed by up to four cells).
        segments.sort_by(|p, q| {
            let kp = [p.0.x, p.0.y, p.0.z, p.1.x, p.1.y, p.1.z];
            let kq = [q.0.x, q.0.y, q.0.z, q.1.x, q.1.y, q.1.z];
            kp.partial_cmp(&kq).unwrap_or(std::cmp::Ordering::Equal)
        });
        segments.dedup_by(|p, q| (p.0 - q.0).length() < 1e-9 && (p.1 - q.1).length() < 1e-9);
        BeamLattice { cell, size, radius, segments }
    }

    /// Distance from `p` to the nearest beam centreline, in world units.
    pub fn centreline_distance(&self, p: DVec3) -> f64 {
        let q = p / self.size;
        let cell = q.floor();
        let local = q - cell;
        let mut best = f64::INFINITY;
        for (a, b) in &self.segments {
            let ab = *b - *a;
            let t = ((local - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let d = (local - (*a + ab * t)).length_squared();
            if d < best {
                best = d;
            }
        }
        best.sqrt() * self.size
    }
}

impl Field for BeamLattice {
    fn at(&self, p: DVec3) -> f64 {
        self.centreline_distance(p) - self.radius
    }
}

/// A lattice whose thickness follows a scalar field: beams or sheets get
/// thicker where `thickness` is larger. `core` must return the distance
/// to the lattice centreline or mid sheet (a `BeamLattice` with radius 0
/// or a `Lattice` with wall 0).
pub struct Graded<C, T> {
    pub core: C,
    pub thickness: T,
}

impl<C: Field, T: Field> Field for Graded<C, T> {
    fn at(&self, p: DVec3) -> f64 {
        self.core.at(p) - 0.5 * self.thickness.at(p).max(0.0)
    }
}

/// A scalar that ramps linearly along a direction: `a` at `from`, `b` at
/// `to`, clamped beyond.
pub struct Ramp {
    pub from: DVec3,
    pub to: DVec3,
    pub a: f64,
    pub b: f64,
}

impl Field for Ramp {
    fn at(&self, p: DVec3) -> f64 {
        let d = self.to - self.from;
        let t = ((p - self.from).dot(d) / d.length_squared().max(1e-18)).clamp(0.0, 1.0);
        self.a + (self.b - self.a) * t
    }
}

/// A scalar that ramps with distance from a point: `a` at the centre, `b`
/// at `radius` and beyond.
pub struct Radial {
    pub centre: DVec3,
    pub radius: f64,
    pub a: f64,
    pub b: f64,
}

impl Field for Radial {
    fn at(&self, p: DVec3) -> f64 {
        let t = ((p - self.centre).length() / self.radius.max(1e-9)).clamp(0.0, 1.0);
        self.a + (self.b - self.a) * t
    }
}

/// Evaluate a field in warped coordinates, so a lattice follows a shape.
pub struct Warp<F, M> {
    pub field: F,
    pub map: M,
}

impl<F: Field, M: Fn(DVec3) -> DVec3 + Sync> Field for Warp<F, M> {
    fn at(&self, p: DVec3) -> f64 {
        self.field.at((self.map)(p))
    }
}

/// Cylindrical warp about the Z axis: `x` becomes the arc length around
/// the axis at `radius`, `y` the distance from the axis. A lattice in
/// these coordinates wraps around a cylinder with its cells aligned to
/// the wall, one ring of cells per `size` of arc at `radius`.
pub fn cylindrical(radius: f64) -> impl Fn(DVec3) -> DVec3 + Sync {
    move |p: DVec3| {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        let theta = p.y.atan2(p.x);
        DVec3::new(theta * radius, r, p.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_beams_sit_on_the_cell_edges() {
        let l = BeamLattice::new(BeamCell::Cubic, 10.0, 1.0);
        // On an edge the distance is zero, so the value is minus the radius.
        assert!((l.at(DVec3::new(3.0, 0.0, 0.0)) + 1.0).abs() < 1e-9);
        assert!((l.at(DVec3::new(13.0, 10.0, 20.0)) + 1.0).abs() < 1e-9);
        // The cell centre is furthest from every edge: 5 * sqrt 2 away.
        let c = l.at(DVec3::new(5.0, 5.0, 5.0));
        assert!((c - (50.0f64.sqrt() - 1.0)).abs() < 1e-9, "{c}");
    }

    #[test]
    fn octet_has_more_beams_than_cubic() {
        let a = BeamLattice::new(BeamCell::Cubic, 1.0, 0.05);
        let b = BeamLattice::new(BeamCell::Octet, 1.0, 0.05);
        assert!(b.segments.len() > a.segments.len());
        // The octet passes through the face centres.
        assert!(b.at(DVec3::new(0.5, 0.5, 0.0)) < 0.0);
        assert!(b.at(DVec3::new(0.25, 0.25, 0.0)) < 0.0, "on a face diagonal beam");
    }

    #[test]
    fn graded_thickness_follows_the_ramp() {
        let core = BeamLattice::new(BeamCell::Cubic, 10.0, 0.0);
        let g = Graded { core, thickness: Ramp { from: DVec3::ZERO, to: DVec3::new(0.0, 0.0, 100.0), a: 1.0, b: 3.0 } };
        // On an edge, the value is minus half the local thickness.
        assert!((g.at(DVec3::new(3.0, 0.0, 0.0)) + 0.5).abs() < 1e-9);
        assert!((g.at(DVec3::new(3.0, 0.0, 100.0)) + 1.5).abs() < 1e-9);
    }

    #[test]
    fn cylindrical_warp_wraps_the_lattice() {
        let w = Warp { field: BeamLattice::new(BeamCell::Cubic, 5.0, 0.5), map: cylindrical(20.0) };
        // Points on the axis-parallel beam at r = 20, theta = 0: x = 20.
        assert!(w.at(DVec3::new(20.0, 0.0, 7.0)) < 0.0);
        // A quarter turn later, at 20 * pi / 2 = 31.4 of arc, is not a beam
        // for cell size 5 (31.4 / 5 is not whole), but the ring at z = 5 is.
        assert!(w.at(DVec3::new(0.0, 20.0, 5.0)) < 0.0);
    }
}
