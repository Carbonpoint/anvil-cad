//! The selected feature drawn inside the part.
//!
//! Select a feature and its own shape is outlined in the view, even when
//! a later feature covers it: the hidden part of every edge is drawn
//! faint, so the shape reads as a ghost through the solid. The geometry
//! is the feature's own result at its step in the history, so a feature
//! that a later boolean swallowed can still be seen.

use crate::camera::Projector;
use crate::raster::Framebuffer;
use anvil_feature::Document;
use anvil_math::DVec3;
use egui::{Pos2, Stroke};

/// Edges sharper than this many radians are drawn. The same value the
/// scene uses, so the ghost matches the shaded picture.
const EDGE_ANGLE: f64 = 0.35;

/// Feature edges of the selected feature's own bodies, in world space.
/// Empty when the feature makes no solid (a sketch, for example: those
/// are already drawn by `sketch_view`).
pub fn ghost_edges(doc: &Document, sel: usize) -> Vec<[DVec3; 2]> {
    let Some(out) = doc.features.get(sel).and_then(|n| n.output.as_ref()) else { return Vec::new() };
    let mut segs = Vec::new();
    for body in &out.bodies {
        segs.extend(body.feature_edges(EDGE_ANGLE));
    }
    segs
}

/// Draw the ghost. Visible edges take a solid accent line, hidden ones a
/// faint dashed line of the same colour.
pub fn draw_ghost(painter: &egui::Painter, origin: Pos2, proj: &Projector, fb: &Framebuffer, segs: &[[DVec3; 2]]) {
    let base = painter.ctx().style().visuals.warn_fg_color;
    let vis = Stroke::new(2.2f32, base);
    let hid = Stroke::new(1.4f32, base.gamma_multiply(0.45));
    for [p, q] in segs {
        let (Some(pa), Some(pb)) = (proj.project(*p), proj.project(*q)) else { continue };
        // Split long edges so each piece gets its own depth test: one
        // edge can be partly in front of the part and partly behind it.
        let len_px = ((pb.0 - pa.0).powi(2) + (pb.1 - pa.1).powi(2)).sqrt();
        let pieces = ((len_px / 6.0).ceil() as usize).clamp(1, 400);
        for j in 0..pieces {
            let (t0, t1) = (j as f64 / pieces as f64, (j + 1) as f64 / pieces as f64);
            let lerp = |t: f64| (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t, pa.2 + (pb.2 - pa.2) * t);
            let (q0, q1, qm) = (lerp(t0), lerp(t1), lerp((t0 + t1) * 0.5));
            let behind = crate::sketch_view::hidden(fb, qm);
            // A dash pattern on the hidden part: every third piece is
            // left out, which reads as "inside the part", not "on it".
            if behind && j % 3 == 2 {
                continue;
            }
            painter.line_segment(
                [
                    Pos2::new(origin.x + q0.0 as f32, origin.y + q0.1 as f32),
                    Pos2::new(origin.x + q1.0 as f32, origin.y + q1.1 as f32),
                ],
                if behind { hid } else { vis },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A feature that a later feature covers still has ghost edges.
    #[test]
    fn a_covered_feature_still_has_edges() {
        let mut app_doc = Document::new("t");
        app_doc.add_feature(Box::new(anvil_feature::features::sketch::SketchFeature::rectangle("XY", 40.0, 20.0)));
        app_doc.add_feature(Box::new(anvil_feature::features::extrude::ExtrudeFeature {
            sketch: 0,
            distance: "10".into(),
            ..Default::default()
        }));
        let segs = ghost_edges(&app_doc, 1);
        assert!(segs.len() >= 12, "a box has twelve edges, got {}", segs.len());
        // A sketch makes no solid, so it has no ghost of its own.
        assert!(ghost_edges(&app_doc, 0).is_empty());
    }
}
