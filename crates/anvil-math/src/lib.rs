//! Shared math types for Anvil.
//!
//! All geometry uses `f64`. The kernel uses one linear tolerance and one
//! angular tolerance everywhere. Do not invent local tolerances.

pub use glam::{DMat3, DMat4, DQuat, DVec2, DVec3};

/// Linear tolerance in model units (millimetres).
pub const LINEAR_TOL: f64 = 1e-7;
/// Angular tolerance in radians.
pub const ANGULAR_TOL: f64 = 1e-9;

/// A plane given by an origin and an orthonormal frame.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Plane {
    pub origin: DVec3,
    pub x_axis: DVec3,
    pub y_axis: DVec3,
}

impl Default for Plane {
    fn default() -> Self {
        Plane::XY
    }
}

impl Plane {
    pub const XY: Plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::X, y_axis: DVec3::Y };
    pub const XZ: Plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::X, y_axis: DVec3::Z };
    pub const YZ: Plane = Plane { origin: DVec3::ZERO, x_axis: DVec3::Y, y_axis: DVec3::Z };

    pub fn normal(&self) -> DVec3 {
        self.x_axis.cross(self.y_axis).normalize()
    }

    /// Map a 2D sketch point to 3D world space.
    pub fn to_world(&self, p: DVec2) -> DVec3 {
        self.origin + self.x_axis * p.x + self.y_axis * p.y
    }

    /// Project a 3D point onto the plane and return sketch coordinates.
    pub fn to_local(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.x_axis), d.dot(self.y_axis))
    }
}

/// An axis given by a point and a unit direction.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    pub origin: DVec3,
    pub dir: DVec3,
}

impl Axis {
    pub fn new(origin: DVec3, dir: DVec3) -> Self {
        Axis { origin, dir: dir.normalize() }
    }
    /// Rotate a point about this axis by `angle` radians.
    pub fn rotate(&self, p: DVec3, angle: f64) -> DVec3 {
        let q = DQuat::from_axis_angle(self.dir, angle);
        self.origin + q * (p - self.origin)
    }
}

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    pub fn empty() -> Self {
        Aabb { min: DVec3::splat(f64::INFINITY), max: DVec3::splat(f64::NEG_INFINITY) }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    pub fn include(&mut self, p: DVec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    pub fn diagonal(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            (self.max - self.min).length()
        }
    }
}

pub fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= LINEAR_TOL
}

/// Euler order used by Move/Copy: rotate about X, then Y, then Z (extrinsic).
pub fn glam_euler() -> glam::EulerRot {
    glam::EulerRot::ZYX
}

impl Default for Aabb {
    fn default() -> Self {
        Aabb::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plane_round_trip() {
        let p = Plane::XZ;
        let w = p.to_world(DVec2::new(2.0, 3.0));
        assert_eq!(w, DVec3::new(2.0, 0.0, 3.0));
        assert_eq!(p.to_local(w), DVec2::new(2.0, 3.0));
    }
    #[test]
    fn axis_rotate_quarter_turn() {
        let a = Axis::new(DVec3::ZERO, DVec3::Z);
        let r = a.rotate(DVec3::X, std::f64::consts::FRAC_PI_2);
        assert!((r - DVec3::Y).length() < 1e-12);
    }
}
