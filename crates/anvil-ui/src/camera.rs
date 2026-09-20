//! Camera and projection shared by the renderer, the overlays, and picking.

use anvil_math::{Aabb, DQuat, DVec3, Plane};

/// Which world axis stays vertical on screen while orbiting. `Free` lets
/// the view tumble in any direction, like a trackball.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpAxis {
    X,
    Y,
    Z,
    Free,
}

impl UpAxis {
    pub fn label(self) -> &'static str {
        match self {
            UpAxis::X => "X up",
            UpAxis::Y => "Y up",
            UpAxis::Z => "Z up",
            UpAxis::Free => "Free",
        }
    }

    /// (first horizontal axis, second horizontal axis, up). Free has none.
    fn frame(self) -> Option<(DVec3, DVec3, DVec3)> {
        match self {
            UpAxis::X => Some((DVec3::Y, DVec3::Z, DVec3::X)),
            UpAxis::Y => Some((DVec3::Z, DVec3::X, DVec3::Y)),
            UpAxis::Z => Some((DVec3::X, DVec3::Y, DVec3::Z)),
            UpAxis::Free => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub target: DVec3,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub fov_y: f64,
    /// Orthographic projection with this world-space view height. `None`
    /// means perspective.
    pub ortho_height: Option<f64>,
    /// Fixed view frame (forward, up) used while sketching. Orbit is
    /// disabled while set.
    pub locked_frame: Option<(DVec3, DVec3)>,
    /// World axis kept vertical while orbiting.
    pub up_axis: UpAxis,
    /// Orientation used when `up_axis` is `Free`: it maps the view frame
    /// (x right, y up, minus z forward) into the world.
    pub orient: DQuat,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            target: DVec3::ZERO,
            yaw: 0.8,
            pitch: 0.5,
            distance: 150.0,
            fov_y: 40f64.to_radians(),
            ortho_height: None,
            locked_frame: None,
            up_axis: UpAxis::Z,
            orient: DQuat::IDENTITY,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> DVec3 {
        if let Some((f, _)) = self.locked_frame {
            return f;
        }
        match self.up_axis.frame() {
            Some((e1, e2, up)) => {
                -(e1 * (self.yaw.cos() * self.pitch.cos())
                    + e2 * (self.yaw.sin() * self.pitch.cos())
                    + up * self.pitch.sin())
            }
            None => self.orient * DVec3::NEG_Z,
        }
    }

    pub fn eye(&self) -> DVec3 {
        self.target - self.forward() * self.distance
    }

    /// Basis: (right, up, forward) unit vectors.
    pub fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let fwd = self.forward();
        if let Some((_, up)) = self.locked_frame {
            let right = fwd.cross(up).normalize();
            return (right, up, fwd);
        }
        let Some((e1, _, world_up)) = self.up_axis.frame() else {
            return (self.orient * DVec3::X, self.orient * DVec3::Y, fwd);
        };
        let right = fwd.cross(world_up).normalize_or_zero();
        let right = if right.length_squared() < 1e-12 { e1 } else { right };
        let up = right.cross(fwd).normalize();
        (right, up, fwd)
    }

    /// Change which axis stays vertical, keeping the current view
    /// direction as closely as the new mode allows.
    pub fn set_up_axis(&mut self, axis: UpAxis) {
        if axis == self.up_axis {
            return;
        }
        let (right, up, fwd) = self.basis();
        self.up_axis = axis;
        match axis.frame() {
            Some((e1, e2, world_up)) => {
                // Read yaw and pitch back from the direction we look along.
                let d = -fwd;
                self.pitch = d.dot(world_up).clamp(-1.0, 1.0).asin();
                self.yaw = d.dot(e2).atan2(d.dot(e1));
            }
            None => {
                self.orient = DQuat::from_mat3(&anvil_math::DMat3::from_cols(right, up, -fwd));
            }
        }
    }

    /// A cheap value that changes whenever the view changes. Used to skip
    /// work when nothing moved.
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in [
            self.target.x,
            self.target.y,
            self.target.z,
            self.yaw,
            self.pitch,
            self.distance,
            self.fov_y,
            self.ortho_height.unwrap_or(f64::NAN),
            self.orient.x,
            self.orient.y,
            self.orient.z,
            self.orient.w,
        ] {
            v.to_bits().hash(&mut h);
        }
        (self.up_axis as u8).hash(&mut h);
        if let Some((f, u)) = self.locked_frame {
            for v in f.to_array().iter().chain(u.to_array().iter()) {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    }

    /// Orthographic if `on`, framed on a box of this diagonal.
    pub fn set_ortho(&mut self, on: bool, view_height: f64) {
        self.ortho_height = on.then(|| view_height.max(1.0));
    }

    pub fn is_ortho(&self) -> bool {
        self.ortho_height.is_some()
    }

    pub fn fit(&mut self, b: &Aabb) {
        self.fit_in(b, 1.0);
    }

    /// Frame the box in a view `aspect` wide per unit of height. A view
    /// narrower than it is tall moves the camera back so the sides fit too.
    pub fn fit_in(&mut self, b: &Aabb, aspect: f64) {
        if b.is_empty() {
            return;
        }
        let narrow = aspect.clamp(0.2, 1.0);
        self.target = b.center();
        self.distance = (b.diagonal() * 0.6 / (self.fov_y / 2.0).tan() / narrow).max(1.0);
        if self.ortho_height.is_some() {
            self.ortho_height = Some(b.diagonal().max(1.0) * 1.2 / narrow);
        }
    }

    /// Look straight at a plane, orthographic, for sketching.
    pub fn look_at_plane(&mut self, plane: &Plane, view_height: f64) {
        self.target = plane.origin;
        self.locked_frame = Some((-plane.normal(), plane.y_axis));
        self.ortho_height = Some(view_height.max(1.0));
        self.distance = view_height.max(1.0) * 2.0;
    }

    pub fn unlock(&mut self) {
        self.locked_frame = None;
        self.ortho_height = None;
    }

    pub fn orbit(&mut self, dx: f64, dy: f64) {
        if self.locked_frame.is_some() {
            return;
        }
        if self.up_axis == UpAxis::Free {
            // Trackball: turn about the axes of the view itself.
            let (right, up, _) = self.basis();
            let q = DQuat::from_axis_angle(up, -dx * 0.01) * DQuat::from_axis_angle(right, dy * 0.01);
            self.orient = (q * self.orient).normalize();
            return;
        }
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-1.5, 1.5);
    }

    pub fn pan(&mut self, dx: f64, dy: f64, view_height_px: f64) {
        let (r, u, _) = self.basis();
        let world_per_px = match self.ortho_height {
            Some(h) => h / view_height_px,
            None => 2.0 * self.distance * (self.fov_y / 2.0).tan() / view_height_px,
        };
        self.target -= r * dx * world_per_px;
        self.target += u * dy * world_per_px;
    }

    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(0.1, 1e6);
        if let Some(h) = self.ortho_height {
            self.ortho_height = Some((h * factor).clamp(0.01, 1e6));
        }
    }
}

/// Maps world points to framebuffer pixels and pixels back to rays.
#[derive(Clone, Debug)]
pub struct Projector {
    pub eye: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub fwd: DVec3,
    pub width: f64,
    pub height: f64,
    /// Perspective: focal length in pixels. Ortho: pixels per world unit.
    pub scale: f64,
    pub ortho: bool,
}

impl Projector {
    pub fn new(cam: &Camera, width: f64, height: f64) -> Self {
        let (right, up, fwd) = cam.basis();
        let (scale, ortho) = match cam.ortho_height {
            Some(h) => (height / h, true),
            None => ((height / 2.0) / (cam.fov_y / 2.0).tan(), false),
        };
        Projector { eye: cam.eye(), right, up, fwd, width, height, scale, ortho }
    }

    /// Pixel position and depth (distance along the view direction).
    /// Returns None when the point is behind the camera.
    pub fn project(&self, p: DVec3) -> Option<(f64, f64, f64)> {
        let d = p - self.eye;
        let z = d.dot(self.fwd);
        let (x, y) = if self.ortho {
            (d.dot(self.right) * self.scale, d.dot(self.up) * self.scale)
        } else {
            if z <= 1e-6 {
                return None;
            }
            (d.dot(self.right) / z * self.scale, d.dot(self.up) / z * self.scale)
        };
        Some((self.width / 2.0 + x, self.height / 2.0 - y, z))
    }

    /// World ray through a pixel: (origin, unit direction).
    pub fn ray(&self, px: f64, py: f64) -> (DVec3, DVec3) {
        let x = (px - self.width / 2.0) / self.scale;
        let y = (self.height / 2.0 - py) / self.scale;
        if self.ortho {
            (self.eye + self.right * x + self.up * y, self.fwd)
        } else {
            (self.eye, (self.fwd + self.right * x + self.up * y).normalize())
        }
    }

    /// Intersect the pixel ray with a plane. Returns plane coordinates.
    pub fn pixel_to_plane(&self, px: f64, py: f64, plane: &Plane) -> Option<anvil_math::DVec2> {
        let (o, d) = self.ray(px, py);
        let n = plane.normal();
        let denom = d.dot(n);
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = (plane.origin - o).dot(n) / denom;
        if t < 0.0 && !self.ortho {
            return None;
        }
        Some(plane.to_local(o + d * t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_axis_keeps_the_view_direction() {
        let mut c = Camera { yaw: 0.6, pitch: 0.4, ..Camera::default() };
        let before = c.forward();
        for axis in [UpAxis::X, UpAxis::Y, UpAxis::Free, UpAxis::Z] {
            c.set_up_axis(axis);
            assert!((c.forward() - before).length() < 1e-9, "{axis:?} moved the view");
            let (_, up, _) = c.basis();
            if let Some((_, _, world_up)) = axis.frame() {
                assert!(up.dot(world_up) > -1e-9, "{axis:?} is upside down");
            }
        }
    }

    #[test]
    fn free_orbit_tumbles_past_the_pole() {
        let mut c = Camera::default();
        c.set_up_axis(UpAxis::Free);
        let start = c.forward();
        for _ in 0..40 {
            c.orbit(0.0, 50.0);
        }
        let (_, up, fwd) = c.basis();
        assert!((fwd - start).length() > 0.5, "the view did not move");
        assert!(up.dot(DVec3::Z).abs() < 0.999, "free orbit should leave world up");
        assert!((fwd.length() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ortho_switches_on_and_off() {
        let mut c = Camera::default();
        assert!(!c.is_ortho());
        c.set_ortho(true, 50.0);
        assert_eq!(c.ortho_height, Some(50.0));
        c.set_ortho(false, 50.0);
        assert!(!c.is_ortho());
    }
}
