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
    /// Two lines are perpendicular (1 equation).
    Perpendicular(EntityId, EntityId),
    /// Two lines have equal length (1 equation).
    EqualLength(EntityId, EntityId),
    /// A circle or arc radius equals `r` (1 equation).
    Radius(EntityId, f64),
    /// A point lies on a line (1 equation).
    PointOnLine(EntityId, EntityId),
    /// A point lies on a circle (1 equation).
    PointOnCircle(EntityId, EntityId),
    /// Lock a point at its current location (2 equations).
    Fix(EntityId),
    /// Point x coordinate equals value (1 equation).
    FixX(EntityId, f64),
    /// Point y coordinate equals value (1 equation).
    FixY(EntityId, f64),
}
