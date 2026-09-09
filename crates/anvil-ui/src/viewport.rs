//! CPU software viewport.
//!
//! Every frame: transform all triangles to camera space, drop back faces,
//! sort far to near, project, and hand egui filled polygons. Flat shading
//! from one directional light. This keeps the app GPU-free. It is fast enough
//! for tens of thousands of triangles on one core.

use anvil_kernel::TriMesh;
use anvil_math::{Aabb, DVec3};
use egui::{Color32, Pos2, Stroke};

pub struct Camera {
    pub target: DVec3,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub fov_y: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: DVec3::ZERO, yaw: 0.8, pitch: 0.5, distance: 150.0, fov_y: 40f64.to_radians() }
    }
}

impl Camera {
    pub fn eye(&self) -> DVec3 {
        let d = DVec3::new(self.yaw.cos() * self.pitch.cos(), self.yaw.sin() * self.pitch.cos(), self.pitch.sin());
        self.target + d * self.distance
    }

    /// Basis: (right, up, forward) unit vectors. Z is world up.
    fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let fwd = (self.target - self.eye()).normalize();
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
    }

    pub fn orbit(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-1.5, 1.5);
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        let (r, u, _) = self.basis();
        let s = self.distance * 0.0015;
        self.target -= r * dx * s;
        self.target += u * dy * s;
    }

    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(0.1, 1e6);
    }
}

struct Projector {
    eye: DVec3,
    right: DVec3,
    up: DVec3,
    fwd: DVec3,
    focal: f64,
    center: Pos2,
}

impl Projector {
    fn new(cam: &Camera, rect: egui::Rect) -> Self {
        let (right, up, fwd) = cam.basis();
        let focal = (rect.height() as f64 / 2.0) / (cam.fov_y / 2.0).tan();
        Projector { eye: cam.eye(), right, up, fwd, focal, center: rect.center() }
    }
    /// Returns (screen position, depth). Depth <= 0 is behind the camera.
    fn project(&self, p: DVec3) -> (Pos2, f64) {
        let d = p - self.eye;
        let z = d.dot(self.fwd);
        if z <= 1e-6 {
            return (self.center, z);
        }
        let x = d.dot(self.right) / z * self.focal;
        let y = d.dot(self.up) / z * self.focal;
        (Pos2::new(self.center.x + x as f32, self.center.y - y as f32), z)
    }
}

pub struct RenderStats {
    pub triangles_drawn: usize,
}

pub fn draw(painter: &egui::Painter, rect: egui::Rect, cam: &Camera, mesh: &TriMesh, wireframe: bool) -> RenderStats {
    let proj = Projector::new(cam, rect);
    let light = DVec3::new(0.4, -0.3, 0.85).normalize();
    let base = Color32::from_rgb(120, 160, 210);

    struct Tri {
        pts: [Pos2; 3],
        depth: f64,
        color: Color32,
    }
    let mut tris: Vec<Tri> = Vec::with_capacity(mesh.triangle_count());
    for t in mesh.indices.as_chunks::<3>().0 {
        let a = mesh.positions[t[0] as usize];
        let b = mesh.positions[t[1] as usize];
        let c = mesh.positions[t[2] as usize];
        let n = (b - a).cross(c - a).normalize_or_zero();
        let centroid = (a + b + c) / 3.0;
        // Back-face cull: skip triangles facing away from the eye.
        if n.dot(proj.eye - centroid) <= 0.0 {
            continue;
        }
        let (pa, za) = proj.project(a);
        let (pb, zb) = proj.project(b);
        let (pc, zc) = proj.project(c);
        if za <= 0.0 || zb <= 0.0 || zc <= 0.0 {
            continue;
        }
        let shade = 0.35 + 0.65 * n.dot(light).max(0.0);
        let color = Color32::from_rgb(
            (base.r() as f64 * shade) as u8,
            (base.g() as f64 * shade) as u8,
            (base.b() as f64 * shade) as u8,
        );
        tris.push(Tri { pts: [pa, pb, pc], depth: (za + zb + zc) / 3.0, color });
    }
    tris.sort_by(|x, y| y.depth.partial_cmp(&x.depth).unwrap_or(std::cmp::Ordering::Equal));

    let stroke = if wireframe { Stroke::new(0.7f32, Color32::from_rgb(40, 50, 70)) } else { Stroke::NONE };
    let drawn = tris.len();
    for t in tris {
        painter.add(egui::Shape::convex_polygon(t.pts.to_vec(), t.color, stroke));
    }
    draw_triad(painter, rect, cam);
    RenderStats { triangles_drawn: drawn }
}

fn draw_triad(painter: &egui::Painter, rect: egui::Rect, cam: &Camera) {
    let (r, u, f) = cam.basis();
    let origin = Pos2::new(rect.left() + 40.0, rect.bottom() - 40.0);
    let axes = [
        (DVec3::X, Color32::RED, "X"),
        (DVec3::Y, Color32::GREEN, "Y"),
        (DVec3::Z, Color32::from_rgb(80, 120, 255), "Z"),
    ];
    for (a, c, label) in axes {
        let sx = a.dot(r) as f32;
        let sy = a.dot(u) as f32;
        let _ = f;
        let end = Pos2::new(origin.x + sx * 28.0, origin.y - sy * 28.0);
        painter.line_segment([origin, end], Stroke::new(2.0f32, c));
        painter.text(end, egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(11.0), c);
    }
}
