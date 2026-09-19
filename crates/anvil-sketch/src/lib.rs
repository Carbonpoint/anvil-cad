//! 2D sketching.
//!
//! Vocabulary used across Anvil:
//! * **Sketch**: a set of 2D entities on a plane plus constraints.
//! * **Entity**: a point, line, circle, or arc. Each owns some scalar
//!   *variables* (for example a point owns x and y).
//! * **Constraint**: an equation between variables (coincident, horizontal,
//!   distance, ...). The solver drives every equation residual to zero.
//! * **Profile**: closed loops extracted from a solved sketch. Features such
//!   as Extrude consume profiles.

pub mod constraint;
pub mod edit;
pub mod profile;
pub mod solver;

use anvil_math::{DVec2, Plane};
use slotmap::{new_key_type, SlotMap};

pub use constraint::Constraint;
pub use profile::Profile;
pub use solver::{SolveReport, SolveStatus};

new_key_type! {
    pub struct EntityId;
    pub struct ConstraintId;
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Entity {
    Point {
        pos: DVec2,
        fixed: bool,
    },
    Line {
        a: EntityId,
        b: EntityId,
        construction: bool,
    },
    Circle {
        center: EntityId,
        radius: f64,
    },
    /// Counter-clockwise arc from `start` to `end` about `center`.
    Arc {
        center: EntityId,
        start: EntityId,
        end: EntityId,
    },
    /// Ellipse with semi-axes `rx`, `ry` rotated by `rotation` radians.
    Ellipse {
        center: EntityId,
        rx: f64,
        ry: f64,
        rotation: f64,
    },
    /// Catmull-Rom spline through control points.
    Spline {
        points: Vec<EntityId>,
        closed: bool,
    },
}

/// Segments used to discretise a full ellipse or one spline span.
pub const CURVE_SEGMENTS: usize = 32;

impl Entity {
    /// Point ids this entity depends on.
    pub fn point_refs(&self) -> Vec<EntityId> {
        match self {
            Entity::Point { .. } => vec![],
            Entity::Line { a, b, .. } => vec![*a, *b],
            Entity::Circle { center, .. } | Entity::Ellipse { center, .. } => vec![*center],
            Entity::Arc { center, start, end } => vec![*center, *start, *end],
            Entity::Spline { points, .. } => points.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Sketch {
    pub plane: Plane,
    pub entities: SlotMap<EntityId, Entity>,
    pub constraints: SlotMap<ConstraintId, Constraint>,
}

impl Sketch {
    pub fn new(plane: Plane) -> Self {
        Sketch { plane, ..Default::default() }
    }

    pub fn add_point(&mut self, x: f64, y: f64) -> EntityId {
        self.entities.insert(Entity::Point { pos: DVec2::new(x, y), fixed: false })
    }

    pub fn add_line(&mut self, a: EntityId, b: EntityId) -> EntityId {
        self.entities.insert(Entity::Line { a, b, construction: false })
    }

    pub fn add_circle(&mut self, center: EntityId, radius: f64) -> EntityId {
        self.entities.insert(Entity::Circle { center, radius })
    }

    /// Centre and radius of a circle or arc.
    pub fn circle_geometry(&self, id: EntityId) -> Option<(DVec2, f64)> {
        match self.entities.get(id)? {
            Entity::Circle { center, radius } => Some((self.point(*center), *radius)),
            Entity::Arc { center, start, .. } => {
                let c = self.point(*center);
                Some((c, (self.point(*start) - c).length()))
            }
            _ => None,
        }
    }

    /// True when two circles are nearer to touching from inside than from
    /// outside, so a tangent constraint between them should keep one inside
    /// the other.
    pub fn circles_nested(&self, a: EntityId, b: EntityId) -> bool {
        let (Some((p, r1)), Some((q, r2))) = (self.circle_geometry(a), self.circle_geometry(b)) else {
            return false;
        };
        let d = (p - q).length();
        (d - (r1 - r2).abs()).abs() < (d - (r1 + r2)).abs()
    }

    pub fn add_arc(&mut self, center: EntityId, start: EntityId, end: EntityId) -> EntityId {
        self.entities.insert(Entity::Arc { center, start, end })
    }

    /// Convenience: a rectangle from corner `(x0,y0)` to `(x1,y1)` with
    /// horizontal and vertical constraints so it stays a rectangle.
    pub fn add_rectangle(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> [EntityId; 4] {
        let p0 = self.add_point(x0, y0);
        let p1 = self.add_point(x1, y0);
        let p2 = self.add_point(x1, y1);
        let p3 = self.add_point(x0, y1);
        let l0 = self.add_line(p0, p1);
        let l1 = self.add_line(p1, p2);
        let l2 = self.add_line(p2, p3);
        let l3 = self.add_line(p3, p0);
        self.constrain(Constraint::Horizontal(l0));
        self.constrain(Constraint::Vertical(l1));
        self.constrain(Constraint::Horizontal(l2));
        self.constrain(Constraint::Vertical(l3));
        [l0, l1, l2, l3]
    }

    pub fn constrain(&mut self, c: Constraint) -> ConstraintId {
        self.constraints.insert(c)
    }

    /// Rectangle by centre and size.
    pub fn add_rectangle_center(&mut self, cx: f64, cy: f64, w: f64, h: f64) -> [EntityId; 4] {
        self.add_rectangle(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
    }

    /// Circle through three points. Returns None if they are collinear.
    pub fn add_circle_3pt(&mut self, a: DVec2, b: DVec2, c: DVec2) -> Option<EntityId> {
        let (center, r) = circumcircle(a, b, c)?;
        let cp = self.add_point(center.x, center.y);
        Some(self.add_circle(cp, r))
    }

    /// Circle with the given diameter end points.
    pub fn add_circle_2pt(&mut self, a: DVec2, b: DVec2) -> EntityId {
        let c = (a + b) * 0.5;
        let cp = self.add_point(c.x, c.y);
        self.add_circle(cp, (b - a).length() / 2.0)
    }

    /// Arc by centre, start point, and end point (counter-clockwise).
    pub fn add_arc_center(&mut self, center: DVec2, start: DVec2, end: DVec2) -> EntityId {
        let r = (start - center).length();
        let ang = (end - center).y.atan2((end - center).x);
        let end = center + DVec2::new(ang.cos(), ang.sin()) * r;
        let c = self.add_point(center.x, center.y);
        let s = self.add_point(start.x, start.y);
        let e = self.add_point(end.x, end.y);
        self.add_arc(c, s, e)
    }

    /// Arc through three points: start, a point on the arc, end.
    pub fn add_arc_3pt(&mut self, start: DVec2, mid: DVec2, end: DVec2) -> Option<EntityId> {
        let (center, _) = circumcircle(start, mid, end)?;
        // Choose direction so the arc passes through `mid`: our arcs are CCW
        // from start to end, so swap ends if `mid` is on the other side.
        let a0 = (start - center).y.atan2((start - center).x);
        let am = (mid - center).y.atan2((mid - center).x);
        let a1 = (end - center).y.atan2((end - center).x);
        let wrap = |x: f64| x.rem_euclid(std::f64::consts::TAU);
        let ccw_contains = wrap(am - a0) < wrap(a1 - a0);
        let c = self.add_point(center.x, center.y);
        let (s, e) = if ccw_contains { (start, end) } else { (end, start) };
        let s = self.add_point(s.x, s.y);
        let e = self.add_point(e.x, e.y);
        Some(self.add_arc(c, s, e))
    }

    /// Regular polygon by centre, circumscribed radius, side count, and
    /// rotation of the first vertex in radians.
    pub fn add_polygon(&mut self, center: DVec2, radius: f64, sides: usize, rotation: f64) -> Vec<EntityId> {
        let sides = sides.max(3);
        let pts: Vec<EntityId> = (0..sides)
            .map(|i| {
                let t = rotation + std::f64::consts::TAU * i as f64 / sides as f64;
                self.add_point(center.x + radius * t.cos(), center.y + radius * t.sin())
            })
            .collect();
        let mut lines = Vec::with_capacity(sides);
        for i in 0..sides {
            lines.push(self.add_line(pts[i], pts[(i + 1) % sides]));
        }
        for i in 1..sides {
            self.constrain(Constraint::EqualLength(lines[0], lines[i]));
        }
        lines
    }

    /// Slot: two semicircle arcs joined by two lines, from centre `a` to
    /// centre `b` with the given width.
    pub fn add_slot(&mut self, a: DVec2, b: DVec2, width: f64) -> Vec<EntityId> {
        let d = (b - a).normalize_or_zero();
        let n = DVec2::new(-d.y, d.x) * (width / 2.0);
        let ca = self.add_point(a.x, a.y);
        let cb = self.add_point(b.x, b.y);
        let a1 = self.add_point(a.x + n.x, a.y + n.y);
        let a2 = self.add_point(a.x - n.x, a.y - n.y);
        let b1 = self.add_point(b.x + n.x, b.y + n.y);
        let b2 = self.add_point(b.x - n.x, b.y - n.y);
        let l1 = self.add_line(a1, b1);
        let l2 = self.add_line(b2, a2);
        // CCW arcs: around b from b1 to b2 going through the far side, around a from a2 to a1.
        let arc_b = self.add_arc(cb, b2, b1);
        let arc_a = self.add_arc(ca, a1, a2);
        self.constrain(Constraint::Parallel(l1, l2));
        self.constrain(Constraint::EqualRadius(arc_a, arc_b));
        vec![l1, arc_b, l2, arc_a]
    }

    /// Line from `start` with the given length and angle in degrees.
    pub fn add_line_polar(&mut self, start: DVec2, length: f64, angle_deg: f64) -> EntityId {
        let t = angle_deg.to_radians();
        let a = self.add_point(start.x, start.y);
        let b = self.add_point(start.x + length * t.cos(), start.y + length * t.sin());
        let l = self.add_line(a, b);
        self.constrain(Constraint::Length(l, length));
        l
    }

    pub fn add_ellipse(&mut self, center: DVec2, rx: f64, ry: f64, rotation: f64) -> EntityId {
        let c = self.add_point(center.x, center.y);
        self.entities.insert(Entity::Ellipse { center: c, rx: rx.max(1e-6), ry: ry.max(1e-6), rotation })
    }

    pub fn add_spline(&mut self, pts: &[DVec2], closed: bool) -> EntityId {
        let ids: Vec<EntityId> = pts.iter().map(|p| self.add_point(p.x, p.y)).collect();
        self.entities.insert(Entity::Spline { points: ids, closed })
    }

    /// Line given by its midpoint and one end.
    pub fn add_line_midpoint(&mut self, mid: DVec2, end: DVec2) -> EntityId {
        let start = mid * 2.0 - end;
        let a = self.add_point(start.x, start.y);
        let b = self.add_point(end.x, end.y);
        let m = self.add_point(mid.x, mid.y);
        let l = self.add_line(a, b);
        self.constrain(Constraint::Midpoint(m, l));
        l
    }

    /// Rectangle by three points: two corners of one edge, then a point on
    /// the opposite edge. Edges stay parallel and perpendicular.
    pub fn add_rectangle_3pt(&mut self, a: DVec2, b: DVec2, c: DVec2) -> [EntityId; 4] {
        let d = (b - a).normalize_or_zero();
        let n = DVec2::new(-d.y, d.x);
        let h = (c - a).dot(n);
        let p0 = self.add_point(a.x, a.y);
        let p1 = self.add_point(b.x, b.y);
        let p2 = self.add_point(b.x + n.x * h, b.y + n.y * h);
        let p3 = self.add_point(a.x + n.x * h, a.y + n.y * h);
        let l0 = self.add_line(p0, p1);
        let l1 = self.add_line(p1, p2);
        let l2 = self.add_line(p2, p3);
        let l3 = self.add_line(p3, p0);
        self.constrain(Constraint::Perpendicular(l0, l1));
        self.constrain(Constraint::Parallel(l0, l2));
        self.constrain(Constraint::Parallel(l1, l3));
        [l0, l1, l2, l3]
    }

    /// Rectangle with a fillet radius per corner, in order bottom-left,
    /// bottom-right, top-right, top-left. A zero radius keeps a sharp corner.
    pub fn add_rounded_rectangle(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, radii: [f64; 4]) -> Vec<EntityId> {
        let corners = [DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
        // Direction from each corner to the next and the previous corner.
        let n = 4;
        let mut pts_in: Vec<DVec2> = Vec::new();
        let mut pts_out: Vec<DVec2> = Vec::new();
        for i in 0..n {
            let c = corners[i];
            let prev = corners[(i + n - 1) % n];
            let next = corners[(i + 1) % n];
            let r = radii[i].max(0.0);
            pts_in.push(c + (prev - c).normalize_or_zero() * r);
            pts_out.push(c + (next - c).normalize_or_zero() * r);
        }
        let mut ids = Vec::new();
        let mut point_ids: Vec<(EntityId, EntityId)> = Vec::new();
        for i in 0..n {
            let a = self.add_point(pts_in[i].x, pts_in[i].y);
            let b = self.add_point(pts_out[i].x, pts_out[i].y);
            point_ids.push((a, b));
        }
        for i in 0..n {
            // Corner arc at corner i from pts_in[i] to pts_out[i] (CCW rectangle).
            let r = radii[i].max(0.0);
            if r > 0.0 {
                let c = corners[i];
                let prev = corners[(i + n - 1) % n];
                let next = corners[(i + 1) % n];
                let center = c + (prev - c).normalize_or_zero() * r + (next - c).normalize_or_zero() * r;
                let cp = self.add_point(center.x, center.y);
                ids.push(self.add_arc(cp, point_ids[i].0, point_ids[i].1));
            } else {
                // Collapse the two coincident points into one.
                self.constrain(Constraint::Coincident(point_ids[i].0, point_ids[i].1));
            }
            // Edge from pts_out[i] to pts_in[i+1].
            let l = self.add_line(point_ids[i].1, point_ids[(i + 1) % n].0);
            if i % 2 == 0 {
                self.constrain(Constraint::Horizontal(l));
            } else {
                self.constrain(Constraint::Vertical(l));
            }
            ids.push(l);
        }
        ids
    }

    /// Regular polygon by centre and inscribed (apothem) radius.
    pub fn add_polygon_inscribed(&mut self, center: DVec2, apothem: f64, sides: usize, rotation: f64) -> Vec<EntityId> {
        let n = sides.max(3) as f64;
        let r = apothem / (std::f64::consts::PI / n).cos();
        self.add_polygon(center, r, sides, rotation + std::f64::consts::PI / n)
    }

    /// Regular polygon from one edge (a to b), built counter-clockwise.
    pub fn add_polygon_edge(&mut self, a: DVec2, b: DVec2, sides: usize) -> Vec<EntityId> {
        let n = sides.max(3) as f64;
        let mid = (a + b) * 0.5;
        let d = b - a;
        let apothem = d.length() / (2.0 * (std::f64::consts::PI / n).tan());
        let nrm = DVec2::new(-d.y, d.x).normalize_or_zero();
        let center = mid + nrm * apothem;
        let r = (a - center).length();
        self.add_polygon(center, r, sides, (a - center).y.atan2((a - center).x))
    }

    /// Slot by centre point, one arc centre, and width (overall length = 2 x half).
    pub fn add_slot_center_point(&mut self, center: DVec2, end_center: DVec2, width: f64) -> Vec<EntityId> {
        let d = end_center - center;
        self.add_slot(center - d, center + d, width)
    }

    /// Polyline sampling of any entity, for display, picking, and profiles.
    pub fn sample(&self, id: EntityId) -> Vec<DVec2> {
        match &self.entities[id] {
            Entity::Point { pos, .. } => vec![*pos],
            Entity::Line { a, b, .. } => vec![self.point(*a), self.point(*b)],
            Entity::Circle { center, radius } => {
                let c = self.point(*center);
                (0..=profile::CIRCLE_SEGMENTS)
                    .map(|i| {
                        let t = i as f64 / profile::CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                        c + DVec2::new(t.cos(), t.sin()) * *radius
                    })
                    .collect()
            }
            Entity::Arc { center, start, end } => {
                let c = self.point(*center);
                let s = self.point(*start);
                let e = self.point(*end);
                let r = (s - c).length();
                let a0 = (s - c).y.atan2((s - c).x);
                let mut a1 = (e - c).y.atan2((e - c).x);
                if a1 <= a0 {
                    a1 += std::f64::consts::TAU;
                }
                let n =
                    (((a1 - a0) / std::f64::consts::TAU) * profile::CIRCLE_SEGMENTS as f64).ceil().max(2.0) as usize;
                (0..=n)
                    .map(|i| {
                        let t = a0 + (a1 - a0) * i as f64 / n as f64;
                        c + DVec2::new(t.cos(), t.sin()) * r
                    })
                    .collect()
            }
            Entity::Ellipse { center, rx, ry, rotation } => {
                let c = self.point(*center);
                let (sr, cr) = rotation.sin_cos();
                (0..=CURVE_SEGMENTS * 2)
                    .map(|i| {
                        let t = i as f64 / (CURVE_SEGMENTS * 2) as f64 * std::f64::consts::TAU;
                        let (x, y) = (rx * t.cos(), ry * t.sin());
                        c + DVec2::new(x * cr - y * sr, x * sr + y * cr)
                    })
                    .collect()
            }
            Entity::Spline { points, closed } => {
                let p: Vec<DVec2> = points.iter().map(|&i| self.point(i)).collect();
                catmull_rom(&p, *closed, CURVE_SEGMENTS / 4)
            }
        }
    }

    /// Remove an entity, everything that depends on it, and every constraint
    /// that mentions any removed entity.
    pub fn remove_entity(&mut self, id: EntityId) {
        let mut doomed = vec![id];
        // Points used by lines/circles/arcs: removing a point removes its users.
        // Removing a line/circle/arc removes only itself (its points stay).
        let mut i = 0;
        while i < doomed.len() {
            let d = doomed[i];
            for (eid, e) in &self.entities {
                let uses = e.point_refs().contains(&d);
                if uses && !doomed.contains(&eid) {
                    doomed.push(eid);
                }
            }
            i += 1;
        }
        for d in &doomed {
            self.entities.remove(*d);
        }
        self.constraints.retain(|_, c| !c.refs().iter().any(|r| doomed.contains(r)));
    }

    /// Nearest entity within `tol` of `p`. Points win over curves.
    pub fn pick(&self, p: DVec2, tol: f64) -> Option<EntityId> {
        let mut best: Option<(f64, EntityId, bool)> = None;
        for (id, e) in &self.entities {
            let (d, is_point) = match e {
                Entity::Point { pos, .. } => ((*pos - p).length(), true),
                Entity::Line { a, b, .. } => (dist_to_segment(p, self.point(*a), self.point(*b)), false),
                Entity::Circle { center, radius } => (((self.point(*center) - p).length() - radius).abs(), false),
                Entity::Arc { .. } | Entity::Ellipse { .. } | Entity::Spline { .. } => {
                    let pts = self.sample(id);
                    let d = pts.windows(2).map(|w| dist_to_segment(p, w[0], w[1])).fold(f64::INFINITY, f64::min);
                    (d, false)
                }
            };
            if d > tol {
                continue;
            }
            let better = match best {
                None => true,
                Some((bd, _, bp)) => (is_point && !bp) || (is_point == bp && d < bd),
            };
            if better {
                best = Some((d, id, is_point));
            }
        }
        best.map(|b| b.1)
    }

    /// Degrees of freedom summary for the status bar.
    pub fn dof_report(&self) -> SolveReport {
        let mut s = self.clone();
        s.solve()
    }

    pub fn point(&self, id: EntityId) -> DVec2 {
        match &self.entities[id] {
            Entity::Point { pos, .. } => *pos,
            _ => panic!("entity {id:?} is not a point"),
        }
    }

    /// Run the constraint solver in place.
    pub fn solve(&mut self) -> SolveReport {
        solver::solve(self)
    }

    /// Extract closed profiles from the current (solved) geometry.
    pub fn profiles(&self) -> Vec<Profile> {
        profile::extract(self)
    }

    /// Open polylines, for sweep paths.
    pub fn open_chains(&self) -> Vec<Vec<DVec2>> {
        profile::extract_open(self)
    }
}

/// Catmull-Rom curve through `p` with `per_span` segments per span.
pub fn catmull_rom(p: &[DVec2], closed: bool, per_span: usize) -> Vec<DVec2> {
    let n = p.len();
    if n < 2 {
        return p.to_vec();
    }
    let get = |i: isize| -> DVec2 {
        if closed {
            p[i.rem_euclid(n as isize) as usize]
        } else {
            p[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * per_span + 1);
    for i in 0..spans {
        let (p0, p1, p2, p3) = (get(i as isize - 1), get(i as isize), get(i as isize + 1), get(i as isize + 2));
        for k in 0..per_span {
            let t = k as f64 / per_span as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push(
                ((p1 * 2.0)
                    + (p2 - p0) * t
                    + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
                    + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3)
                    * 0.5,
            );
        }
    }
    out.push(if closed { p[0] } else { p[n - 1] });
    out
}

pub fn dist_to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-18 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    (p - (a + ab * t)).length()
}

/// Centre and radius of the circle through three points.
pub fn circumcircle(a: DVec2, b: DVec2, c: DVec2) -> Option<(DVec2, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let a2 = a.length_squared();
    let b2 = b.length_squared();
    let c2 = c.length_squared();
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = DVec2::new(ux, uy);
    Some((center, (a - center).length()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_slot_and_arcs_make_closed_profiles() {
        let mut s = Sketch::new(Plane::XY);
        s.add_polygon(DVec2::ZERO, 10.0, 6, 0.0);
        s.add_slot(DVec2::new(30.0, 0.0), DVec2::new(50.0, 0.0), 8.0);
        s.add_arc_3pt(DVec2::new(0.0, 30.0), DVec2::new(10.0, 40.0), DVec2::new(0.0, 50.0)).unwrap();
        let p = s.profiles();
        assert_eq!(p.len(), 2, "hexagon and slot are closed; lone arc is open");
        let hex_area = 1.5 * 3f64.sqrt() * 100.0;
        assert!(p.iter().any(|q| (q.signed_area() - hex_area).abs() < 0.5));
        let slot_area = 20.0 * 8.0 + std::f64::consts::PI * 16.0;
        assert!(
            p.iter().any(|q| (q.signed_area() - slot_area).abs() < 1.0),
            "{:?}",
            p.iter().map(|q| q.signed_area()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rounded_rectangle_is_one_closed_profile() {
        let mut s = Sketch::new(Plane::XY);
        s.add_rounded_rectangle(0.0, 0.0, 85.6, 53.98, [12.7, 6.35, 12.7, 6.35]);
        let p = s.profiles();
        assert_eq!(p.len(), 1);
        let cut = |r: f64| r * r * (1.0 - std::f64::consts::FRAC_PI_4);
        let exact = 85.6 * 53.98 - 2.0 * cut(12.7) - 2.0 * cut(6.35);
        assert!((p[0].signed_area() - exact).abs() < 2.0, "{} vs {exact}", p[0].signed_area());
    }

    #[test]
    fn remove_point_removes_its_lines_and_constraints() {
        let mut s = Sketch::new(Plane::XY);
        let [l0, ..] = s.add_rectangle(0.0, 0.0, 4.0, 2.0);
        let p0 = match s.entities[l0] {
            Entity::Line { a, .. } => a,
            _ => unreachable!(),
        };
        s.remove_entity(p0);
        assert_eq!(s.entities.len(), 3 + 2, "3 points and 2 lines remain");
        assert_eq!(s.constraints.len(), 2);
    }

    #[test]
    fn pick_prefers_points() {
        let mut s = Sketch::new(Plane::XY);
        let [l0, ..] = s.add_rectangle(0.0, 0.0, 4.0, 2.0);
        let p0 = match s.entities[l0] {
            Entity::Line { a, .. } => a,
            _ => unreachable!(),
        };
        assert_eq!(s.pick(DVec2::new(0.1, 0.05), 0.5), Some(p0));
        assert_eq!(s.pick(DVec2::new(2.0, 0.05), 0.5), Some(l0));
        assert_eq!(s.pick(DVec2::new(2.0, 1.0), 0.5), None);
    }

    #[test]
    fn tangent_and_angle_constraints_solve() {
        let mut s = Sketch::new(Plane::XY);
        let c = s.add_point(0.0, 0.0);
        let circ = s.add_circle(c, 5.0);
        let a = s.add_point(-10.0, 6.0);
        let b = s.add_point(10.0, 6.5);
        let l = s.add_line(a, b);
        s.constrain(Constraint::Fix(c));
        s.constrain(Constraint::Radius(circ, 5.0));
        s.constrain(Constraint::Horizontal(l));
        s.constrain(Constraint::Tangent(l, circ));
        s.constrain(Constraint::FixX(a, -10.0));
        s.constrain(Constraint::FixX(b, 10.0));
        let rep = s.solve();
        assert_eq!(rep.status, SolveStatus::Converged, "{rep:?}");
        assert!((s.point(a).y.abs() - 5.0).abs() < 1e-7);

        // Two circles: outside tangent, then one inside the other.
        for (cx, r2, inside) in [(12.0, 4.0, false), (1.0, 2.0, true)] {
            let mut s = Sketch::new(Plane::XY);
            let c1 = s.add_point(0.0, 0.0);
            let k1 = s.add_circle(c1, 5.0);
            let c2 = s.add_point(cx, 0.5);
            let k2 = s.add_circle(c2, r2);
            s.constrain(Constraint::Fix(c1));
            s.constrain(Constraint::Radius(k1, 5.0));
            s.constrain(Constraint::Radius(k2, r2));
            assert_eq!(s.circles_nested(k1, k2), inside);
            s.constrain(Constraint::TangentCircles(k1, k2, inside));
            let rep = s.solve();
            assert_eq!(rep.status, SolveStatus::Converged, "{rep:?}");
            let d = s.point(c2).length();
            let want = if inside { 3.0 } else { 9.0 };
            assert!((d - want).abs() < 1e-7, "{d}");
        }

        let mut s = Sketch::new(Plane::XY);
        let o = s.add_point(0.0, 0.0);
        let x = s.add_point(10.0, 0.0);
        let y = s.add_point(8.0, 5.0);
        let l1 = s.add_line(o, x);
        let l2 = s.add_line(o, y);
        s.constrain(Constraint::Fix(o));
        s.constrain(Constraint::Fix(x));
        s.constrain(Constraint::Length(l2, 10.0));
        s.constrain(Constraint::Angle(l1, l2, 60.0));
        let rep = s.solve();
        assert_eq!(rep.status, SolveStatus::Converged, "{rep:?}");
        assert!((s.point(y) - DVec2::new(5.0, 8.660254)).length() < 1e-5, "{:?}", s.point(y));
    }
}
