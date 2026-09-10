//! Camera and projection shared by the renderer, the overlays, and picking.

use anvil_math::{Aabb, DVec3, Plane};

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
        }
    }
}

impl Camera {
    pub fn forward(&self) -> DVec3 {
        if let Some((f, _)) = self.locked_frame {
            return f;
        }
        -DVec3::new(self.yaw.cos() * self.pitch.cos(), self.yaw.sin() * self.pitch.cos(), self.pitch.sin())
    }

    pub fn eye(&self) -> DVec3 {
        self.target - self.forward() * self.distance
    }

    /// Basis: (right, up, forward) unit vectors. Z is world up in free orbit.
    pub fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let fwd = self.forward();
        if let Some((_, up)) = self.locked_frame {
            let right = fwd.cross(up).normalize();
            return (right, up, fwd);
        }
        let right = fwd.cross(DVec3::Z).normalize_or_zero();
        let right = if right.length_squared() < 1e-12 { DVec3::X } else { right };
        let up = right.cross(fwd).normalize();
        (right, up, fwd)
    }

    pub fn fit(&mut self, b: &Aabb) {
        if b.is_empty() {
            return;
        }
        self.target = b.center();
        self.distance = (b.diagonal() * 0.6 / (self.fov_y / 2.0).tan()).max(1.0);
        if self.ortho_height.is_some() {
            self.ortho_height = Some(b.diagonal().max(1.0) * 1.2);
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
