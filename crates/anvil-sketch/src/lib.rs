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
}
