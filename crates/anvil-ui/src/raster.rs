//! CPU rasterizer with a depth buffer and an id buffer.
//!
//! Colour goes to an egui texture. The id buffer holds the triangle index
//! under every pixel, so picking is a single array read.

use crate::camera::Projector;
use crate::scene::Scene;
use anvil_math::DVec3;
use egui::{Color32, ColorImage};

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub color: Vec<Color32>,
    pub depth: Vec<f32>,
    /// Triangle index + 1, 0 = nothing.
    pub id: Vec<u32>,
}

pub struct Style {
    pub background: Color32,
    pub body: Color32,
    pub selected: Color32,
    pub hovered: Color32,
    pub edge: Color32,
    pub light: DVec3,
    pub draw_edges: bool,
    /// Section view: keep only the side where `normal . p <= w`.
    pub section: Option<(DVec3, f64)>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            background: Color32::from_rgb(236, 239, 243),
            body: Color32::from_rgb(140, 170, 205),
            selected: Color32::from_rgb(240, 180, 60),
            hovered: Color32::from_rgb(180, 205, 235),
            edge: Color32::from_rgb(35, 45, 60),
            light: DVec3::new(0.35, -0.45, 0.82).normalize(),
            draw_edges: true,
            section: None,
        }
    }
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Framebuffer { width, height, color: vec![Color32::BLACK; n], depth: vec![f32::INFINITY; n], id: vec![0; n] }
    }

    pub fn clear(&mut self, bg: Color32) {
        self.color.fill(bg);
        self.depth.fill(f32::INFINITY);
        self.id.fill(0);
    }

    pub fn tri_at(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let v = self.id[y * self.width + x];
        if v == 0 {
            None
        } else {
            Some(v as usize - 1)
        }
    }

    pub fn to_image(&self) -> ColorImage {
        ColorImage {
            size: [self.width, self.height],
            source_size: egui::Vec2::new(self.width as f32, self.height as f32),
            pixels: self.color.clone(),
        }
    }

    /// Draw the whole scene. `selected` and `hovered` are body indices.
    pub fn draw_scene(
        &mut self,
        scene: &Scene,
        proj: &Projector,
        style: &Style,
        selected: Option<u32>,
        hovered: Option<u32>,
    ) {
        self.draw_scene_faces(scene, proj, style, selected, hovered, None, None);
    }

    /// Like `draw_scene`, with a selected and a hovered face to tint.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scene_faces(
        &mut self,
        scene: &Scene,
        proj: &Projector,
        style: &Style,
        selected: Option<u32>,
        hovered: Option<u32>,
        selected_face: Option<(u32, anvil_kernel::FaceId)>,
        hovered_face: Option<(u32, anvil_kernel::FaceId)>,
    ) {
        let mesh = &scene.mesh;
        let projected: Vec<Option<(f64, f64, f64)>> = mesh.positions.iter().map(|&p| proj.project(p)).collect();
        for (t, idx) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            let (Some(a), Some(b), Some(c)) =
                (projected[idx[0] as usize], projected[idx[1] as usize], projected[idx[2] as usize])
            else {
                continue;
            };
            let pa = mesh.positions[idx[0] as usize];
            let pb = mesh.positions[idx[1] as usize];
            let pc = mesh.positions[idx[2] as usize];
            // Sliver triangles have a tiny cross product, so their normal is
            // unreliable. Fall back to the stored face normal.
            let cross = (pb - pa).cross(pc - pa);
            let n = if cross.length() > 1e-12 {
                cross.normalize()
            } else {
                mesh.normals.get(idx[0] as usize).copied().unwrap_or(DVec3::Z)
            };
            let dist = style.section.map(|(sn, w)| (sn.dot(pa) - w, sn.dot(pb) - w, sn.dot(pc) - w));
            if let Some((da, db, dc)) = dist {
                if da > 0.0 && db > 0.0 && dc > 0.0 {
                    continue;
                }
            }
            // Back-face cull against the view direction at the triangle. In a
            // section view back faces show the inside of the cut, tinted.
            let view_dir = if proj.ortho { proj.fwd } else { ((pa + pb + pc) / 3.0 - proj.eye).normalize() };
            let back = n.dot(view_dir) >= 0.0;
            if back && dist.is_none() {
                continue;
            }
            let body = scene.tri_body[t];
            let face = scene.mesh.face_of_tri.get(t).copied();
            let base = if face.is_some() && selected_face == face.map(|f| (body, f)) {
                style.selected
            } else if face.is_some() && hovered_face == face.map(|f| (body, f)) {
                style.hovered
            } else if Some(body) == selected {
                style.selected
            } else if Some(body) == hovered {
                style.hovered
            } else if let Some(Some(c)) = scene.colors.get(body as usize) {
                Color32::from_rgb(c[0], c[1], c[2])
            } else {
                style.body
            };
            let (base, shade) = if back {
                (Color32::from_rgb(200, 70, 60), 0.75)
            } else {
                (base, 0.30 + 0.70 * n.dot(style.light).max(0.0))
            };
            let color = Color32::from_rgb(
                (base.r() as f64 * shade) as u8,
                (base.g() as f64 * shade) as u8,
                (base.b() as f64 * shade) as u8,
            );
            self.triangle_clipped(a, b, c, color, t as u32 + 1, dist);
        }
        if style.draw_edges {
            for (_, [p, q]) in &scene.edges {
                if let Some((sn, w)) = style.section {
                    if sn.dot(*p) - w > 0.0 || sn.dot(*q) - w > 0.0 {
                        continue;
                    }
                }
                if let (Some(a), Some(b)) = (proj.project(*p), proj.project(*q)) {
                    self.line(a, b, style.edge, 0.9985);
                }
            }
        }
    }

    /// Fill a triangle, discarding pixels on the far side of the section
    /// plane. `dist` is the signed plane distance at each vertex.
    #[allow(clippy::too_many_arguments)]
    pub fn triangle_clipped(
        &mut self,
        a: (f64, f64, f64),
        b: (f64, f64, f64),
        c: (f64, f64, f64),
        color: Color32,
        id: u32,
        dist: Option<(f64, f64, f64)>,
    ) {
        let Some((da, db, dc)) = dist else {
            self.triangle(a, b, c, color, id);
            return;
        };
        let min_x = a.0.min(b.0).min(c.0).floor().max(0.0) as i64;
        let max_x = a.0.max(b.0).max(c.0).ceil().min(self.width as f64 - 1.0) as i64;
        let min_y = a.1.min(b.1).min(c.1).floor().max(0.0) as i64;
        let max_y = a.1.max(b.1).max(c.1).ceil().min(self.height as f64 - 1.0) as i64;
        if min_x > max_x || min_y > max_y {
            return;
        }
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-12 {
            return;
        }
        let inv = 1.0 / area;
        for y in min_y..=max_y {
            let py = y as f64 + 0.5;
            for x in min_x..=max_x {
                let px = x as f64 + 0.5;
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) * inv;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                if w0 * da + w1 * db + w2 * dc > 0.0 {
                    continue;
                }
                let z = (w0 * a.2 + w1 * b.2 + w2 * c.2) as f32;
                let i = y as usize * self.width + x as usize;
                if z < self.depth[i] {
                    self.depth[i] = z;
                    self.color[i] = color;
                    self.id[i] = id;
                }
            }
        }
    }

    /// Fill a triangle with depth test. Coordinates are (x, y, depth).
    pub fn triangle(&mut self, a: (f64, f64, f64), b: (f64, f64, f64), c: (f64, f64, f64), color: Color32, id: u32) {
        let min_x = a.0.min(b.0).min(c.0).floor().max(0.0) as i64;
        let max_x = a.0.max(b.0).max(c.0).ceil().min(self.width as f64 - 1.0) as i64;
        let min_y = a.1.min(b.1).min(c.1).floor().max(0.0) as i64;
        let max_y = a.1.max(b.1).max(c.1).ceil().min(self.height as f64 - 1.0) as i64;
        if min_x > max_x || min_y > max_y {
            return;
        }
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-12 {
            return;
        }
        let inv = 1.0 / area;
        for y in min_y..=max_y {
            let py = y as f64 + 0.5;
            for x in min_x..=max_x {
                let px = x as f64 + 0.5;
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) * inv;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = (w0 * a.2 + w1 * b.2 + w2 * c.2) as f32;
                let i = y as usize * self.width + x as usize;
                if z < self.depth[i] {
                    self.depth[i] = z;
                    self.color[i] = color;
                    self.id[i] = id;
                }
            }
        }
    }

    /// Depth-tested line. `bias` scales the depth so lines on a surface win.
    pub fn line(&mut self, a: (f64, f64, f64), b: (f64, f64, f64), color: Color32, bias: f64) {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let x = a.0 + dx * t;
            let y = a.1 + dy * t;
            if x < 0.0 || y < 0.0 || x >= self.width as f64 || y >= self.height as f64 {
                continue;
            }
            let z = ((a.2 + (b.2 - a.2) * t) * bias) as f32;
            let i = y as usize * self.width + x as usize;
            if z <= self.depth[i] {
                self.depth[i] = z;
                self.color[i] = color;
            }
        }
    }
}
