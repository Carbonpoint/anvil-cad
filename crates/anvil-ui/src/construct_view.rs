//! Construction geometry in the model view: the plane, axis or point of
//! every construction feature, drawn faint, and bold when selected.
//! Sketches are drawn by `sketch_view`; a sketch's plane is not repeated
//! here.

use crate::camera::Projector;
use anvil_feature::Document;
use anvil_math::{DVec2, DVec3};
use egui::{Color32, Pos2, Stroke};

/// Feature kinds whose plane, axis or point is drawn.
const KINDS: [&str; 6] = ["offset_plane", "angle_plane", "plane_3pt", "midplane", "axis", "point"];

/// Draw the construction features. `size` is about the size of the model,
/// for the length of axes and the size of planes.
pub fn draw(
    painter: &egui::Painter,
    origin: Pos2,
    proj: &Projector,
    doc: &Document,
    selected: Option<usize>,
    size: f64,
) {
    let to_screen = |p: DVec3| proj.project(p).map(|(x, y, _)| Pos2::new(origin.x + x as f32, origin.y + y as f32));
    for (i, node) in doc.features.iter().enumerate() {
        if !KINDS.contains(&node.feature.kind()) {
            continue;
        }
        let Some(out) = node.output.as_ref() else { continue };
        let hot = selected == Some(i);
        let alpha = if hot { 230 } else { 110 };
        let col = Color32::from_rgba_unmultiplied(230, 140, 40, alpha);
        let stroke = Stroke::new(if hot { 2.0f32 } else { 1.0f32 }, col);
        let label = |at: Pos2| {
            painter.text(
                at + egui::vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{i}"),
                egui::FontId::proportional(11.0),
                col,
            );
        };
        if let Some(pl) = out.plane {
            let h = size * 0.3;
            let corners = [DVec2::new(-h, -h), DVec2::new(h, -h), DVec2::new(h, h), DVec2::new(-h, h)];
            let pts: Vec<Pos2> = corners.iter().filter_map(|c| to_screen(pl.to_world(*c))).collect();
            if pts.len() == 4 {
                let fill = Color32::from_rgba_unmultiplied(230, 140, 40, if hot { 40 } else { 14 });
                painter.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
                label(pts[2]);
            }
        }
        if let Some(ax) = out.axis {
            let (a, b) = (ax.origin - ax.dir * size, ax.origin + ax.dir * size);
            if let (Some(p), Some(q)) = (to_screen(a), to_screen(b)) {
                // Dashed: a construction line, not an edge.
                let n = 24;
                for k in (0..n).step_by(2) {
                    let (s, e) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                    painter.line_segment([p + (q - p) * s, p + (q - p) * e], stroke);
                }
                label(q);
            }
        }
        if let Some(pt) = out.point {
            if let Some(p) = to_screen(pt) {
                let r = if hot { 6.0 } else { 4.0 };
                painter.line_segment([p - egui::vec2(r, 0.0), p + egui::vec2(r, 0.0)], stroke);
                painter.line_segment([p - egui::vec2(0.0, r), p + egui::vec2(0.0, r)], stroke);
                painter.circle_filled(p, 2.0, col);
                label(p);
            }
        }
    }
}
