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
        self.nearest(p).0
    }

    /// Distance to the nearest centreline and the unit direction from
    /// that centreline point to `p`.
    pub fn nearest(&self, p: DVec3) -> (f64, DVec3) {
        let q = p / self.size;
        let cell = q.floor();
        let local = q - cell;
        let mut best = f64::INFINITY;
        let mut dir = DVec3::ZERO;
        for (a, b) in &self.segments {
            let ab = *b - *a;
            let t = ((local - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let off = local - (*a + ab * t);
            let d = off.length_squared();
            if d < best {
                best = d;
                dir = off;
            }
        }
        (best.sqrt() * self.size, dir.normalize_or_zero())
    }
}

impl BeamLattice {
    /// The beams of this lattice inside a body, as world space segments:
    /// every cell edge of every cell that meets the box `lo..hi`, kept
    /// when both ends are inside (`inside` negative) and clipped to the
    /// surface when one end is. For the 3MF beam lattice extension.
    pub fn beams_in(&self, inside: &dyn Field, lo: DVec3, hi: DVec3) -> Vec<(DVec3, DVec3)> {
        let base = self.cell.segments();
        let key = |p: DVec3| ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64);
        type Key = (i64, i64, i64);
        let mut seen: std::collections::HashSet<(Key, Key)> = std::collections::HashSet::new();
        let mut out = Vec::new();
        let clip = |a: DVec3, b: DVec3| -> DVec3 {
            // a inside, b outside: bisect to the surface.
            let (mut pa, mut pb) = (a, b);
            for _ in 0..12 {
                let m = (pa + pb) * 0.5;
                if inside.at(m) < 0.0 {
                    pa = m;
                } else {
                    pb = m;
                }
            }
            (pa + pb) * 0.5
        };
        let c0 = (lo / self.size).floor();
        let c1 = (hi / self.size).ceil();
        let mut cz = c0.z;
        while cz < c1.z {
            let mut cy = c0.y;
            while cy < c1.y {
                let mut cx = c0.x;
                while cx < c1.x {
                    let off = DVec3::new(cx, cy, cz) * self.size;
                    for (a, b) in &base {
                        let (pa, pb) = (off + *a * self.size, off + *b * self.size);
                        let (ka, kb) = (key(pa), key(pb));
                        let k = if ka < kb { (ka, kb) } else { (kb, ka) };
                        if !seen.insert(k) {
                            continue;
                        }
                        let (ia, ib) = (inside.at(pa) < 0.0, inside.at(pb) < 0.0);
                        match (ia, ib) {
                            (true, true) => out.push((pa, pb)),
                            (true, false) => out.push((pa, clip(pa, pb))),
                            (false, true) => out.push((clip(pb, pa), pb)),
                            _ => {}
                        }
                    }
                    cx += 1.0;
                }
                cy += 1.0;
            }
            cz += 1.0;
        }
        out
    }
}

impl Field for BeamLattice {
    fn at(&self, p: DVec3) -> f64 {
        self.centreline_distance(p) - self.radius
    }
    fn grad(&self, p: DVec3) -> DVec3 {
        self.nearest(p).1
    }
}

/// A honeycomb of ribs: the walls of a regular hexagonal tiling in the
/// XY plane, `cell` across between opposite walls, extruded along Z.
/// The distance is to the nearest wall centreline, so `Graded` and the
/// conformal warp work on it like on any lattice.
pub struct Honeycomb {
    pub cell: f64,
    pub wall: f64,
}

impl Honeycomb {
    /// Distance in the plane from `p` to the nearest cell wall centreline.
    pub fn wall_distance(&self, p: DVec3) -> f64 {
        // Cell centres on a triangular lattice with spacing `cell`.
        let s = self.cell.max(1e-9);
        let e1 = anvil_math::DVec2::new(s, 0.0);
        let e2 = anvil_math::DVec2::new(0.5 * s, s * 3f64.sqrt() * 0.5);
        let q = anvil_math::DVec2::new(p.x, p.y);
        // Lattice coordinates of q, then the nearest centre among the
        // neighbours of the rounded coordinates.
        let det = e1.x * e2.y - e1.y * e2.x;
        let a = (q.x * e2.y - q.y * e2.x) / det;
        let b = (e1.x * q.y - e1.y * q.x) / det;
        let (ia, ib) = (a.round() as i64, b.round() as i64);
        let mut centres: Vec<anvil_math::DVec2> = Vec::with_capacity(9);
        for da in -1..=1 {
            for db in -1..=1 {
                centres.push(e1 * (ia + da) as f64 + e2 * (ib + db) as f64);
            }
        }
        let (mut best_i, mut best_d) = (0usize, f64::INFINITY);
        for (i, c) in centres.iter().enumerate() {
            let d = (q - *c).length_squared();
            if d < best_d {
                best_d = d;
                best_i = i;
            }
        }
        let c1 = centres[best_i];
        // Distance to the Voronoi boundary: the nearest bisector plane
        // between the home centre and any other centre.
        let mut wall = f64::INFINITY;
        for (i, c) in centres.iter().enumerate() {
            if i == best_i {
                continue;
            }
            let sep = (*c - c1).length();
            let d = ((q - *c).length_squared() - (q - c1).length_squared()) / (2.0 * sep);
            wall = wall.min(d);
        }
        wall
    }
}

impl Field for Honeycomb {
    fn at(&self, p: DVec3) -> f64 {
        self.wall_distance(p) - 0.5 * self.wall
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
    fn grad(&self, p: DVec3) -> DVec3 {
        // The thickness varies slowly next to the core distance.
        self.core.grad(p)
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
    fn honeycomb_walls_are_where_they_should_be() {
        let h = Honeycomb { cell: 10.0, wall: 1.0 };
        // Midway between two neighbouring centres is on a wall.
        assert!((h.at(DVec3::new(5.0, 0.0, 3.0)) + 0.5).abs() < 1e-9);
        // A cell centre is half a cell from the walls: cell across between
        // walls is 10, so the inscribed radius is 5.
        assert!((h.at(DVec3::ZERO) - 4.5).abs() < 1e-9, "{}", h.at(DVec3::ZERO));
        // Same at a far cell and independent of z.
        assert!((h.at(DVec3::new(35.0, 8.66, -20.0)) - 4.5).abs() < 0.05);
    }

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
    fn beams_inside_a_sphere_are_clipped_to_it() {
        let l = BeamLattice::new(BeamCell::Cubic, 5.0, 0.4);
        let s = crate::Sphere { c: DVec3::ZERO, r: 12.0 };
        let beams = l.beams_in(&s, DVec3::splat(-13.0), DVec3::splat(13.0));
        assert!(beams.len() > 100, "{}", beams.len());
        for (a, b) in &beams {
            assert!(a.length() < 12.01 && b.length() < 12.01, "beam end outside the sphere");
        }
        // A beam through the centre exists along each axis.
        assert!(beams.iter().any(|(a, b)| a.y == 0.0 && a.z == 0.0 && b.y == 0.0 && b.z == 0.0));
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
