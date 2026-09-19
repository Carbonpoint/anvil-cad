use crate::EntityId;

/// A geometric constraint between sketch entities.
///
/// Each variant maps to one or more scalar equations in `solver.rs`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Constraint {
    /// Two points share a location (2 equations).
    Coincident(EntityId, EntityId),
    /// A line is horizontal (1 equation).
    Horizontal(EntityId),
    /// A line is vertical (1 equation).
    Vertical(EntityId),
    /// Distance between two points equals `d` (1 equation).
    Distance(EntityId, EntityId, f64),
    /// Length of a line equals `d` (1 equation).
    Length(EntityId, f64),
    /// Two lines are parallel (1 equation).
    Parallel(EntityId, EntityId),
    /// Two lines lie on one infinite line (2 equations).
    Collinear(EntityId, EntityId),
    /// Two lines are perpendicular (1 equation).
    Perpendicular(EntityId, EntityId),
    /// Two lines have equal length (1 equation).
    EqualLength(EntityId, EntityId),
    /// A circle or arc radius equals `r` (1 equation).
    Radius(EntityId, f64),
    /// Two circles or arcs have equal radius (1 equation).
    EqualRadius(EntityId, EntityId),
    /// Two circles or arcs share a centre (2 equations).
    Concentric(EntityId, EntityId),
    /// A point lies on a line (1 equation).
    PointOnLine(EntityId, EntityId),
    /// A point lies on a circle or arc (1 equation).
    PointOnCircle(EntityId, EntityId),
    /// A point is the midpoint of a line (2 equations).
    Midpoint(EntityId, EntityId),
    /// A line is tangent to a circle or arc (1 equation).
    Tangent(EntityId, EntityId),
    /// Two circles or arcs touch (1 equation). `inside` is true when one
    /// lies inside the other; it is chosen from the geometry when the
    /// constraint is made.
    TangentCircles(EntityId, EntityId, bool),
    /// Angle between two lines equals `deg` degrees (1 equation).
    Angle(EntityId, EntityId, f64),
    /// Two points are mirror images across a line (2 equations).
    Symmetric(EntityId, EntityId, EntityId),
    /// Lock a point at its current location (2 equations).
    Fix(EntityId),
    /// Point x coordinate equals value (1 equation).
    FixX(EntityId, f64),
    /// Point y coordinate equals value (1 equation).
    FixY(EntityId, f64),
}

impl Constraint {
    /// Every entity this constraint refers to.
    pub fn refs(&self) -> Vec<EntityId> {
        use Constraint::*;
        match self {
            Coincident(a, b)
            | Distance(a, b, _)
            | Parallel(a, b)
            | Collinear(a, b)
            | Perpendicular(a, b)
            | EqualLength(a, b)
            | EqualRadius(a, b)
            | Concentric(a, b)
            | PointOnLine(a, b)
            | PointOnCircle(a, b)
            | Midpoint(a, b)
            | Tangent(a, b)
            | TangentCircles(a, b, _)
            | Angle(a, b, _) => vec![*a, *b],
            Symmetric(a, b, c) => vec![*a, *b, *c],
            Horizontal(a) | Vertical(a) | Length(a, _) | Radius(a, _) | Fix(a) | FixX(a, _) | FixY(a, _) => vec![*a],
        }
    }

    /// Short label for the navigator and the sketch overlay.
    pub fn label(&self) -> String {
        use Constraint::*;
        match self {
            Coincident(..) => "Coincident".into(),
            Horizontal(_) => "Horizontal".into(),
            Vertical(_) => "Vertical".into(),
            Distance(_, _, d) => format!("Distance {d:.3}"),
            Length(_, d) => format!("Length {d:.3}"),
            Parallel(..) => "Parallel".into(),
            Collinear(..) => "Collinear".into(),
            Perpendicular(..) => "Perpendicular".into(),
            EqualLength(..) => "Equal".into(),
            Radius(_, r) => format!("Radius {r:.3}"),
            EqualRadius(..) => "Equal radius".into(),
            Concentric(..) => "Concentric".into(),
            PointOnLine(..) => "Point on line".into(),
            PointOnCircle(..) => "Point on circle".into(),
            Midpoint(..) => "Midpoint".into(),
            Tangent(..) | TangentCircles(..) => "Tangent".into(),
            Angle(_, _, a) => format!("Angle {a:.2}"),
            Symmetric(..) => "Symmetric".into(),
            Fix(_) => "Fixed".into(),
            FixX(_, v) => format!("X = {v:.3}"),
            FixY(_, v) => format!("Y = {v:.3}"),
        }
    }

    /// The numeric value of a dimension constraint, if it has one.
    pub fn value(&self) -> Option<f64> {
        use Constraint::*;
        match self {
            Distance(_, _, v) | Length(_, v) | Radius(_, v) | Angle(_, _, v) | FixX(_, v) | FixY(_, v) => Some(*v),
            _ => None,
        }
    }

    pub fn set_value(&mut self, new: f64) {
        use Constraint::*;
        match self {
            Distance(_, _, v) | Length(_, v) | Radius(_, v) | Angle(_, _, v) | FixX(_, v) | FixY(_, v) => *v = new,
            _ => {}
        }
    }
}
