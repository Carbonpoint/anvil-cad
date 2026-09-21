//! Line-art icons for ribbon buttons and the anvil logo.
//!
//! Every icon is drawn with `egui::Painter` shapes into a normalized 0..1
//! square mapped onto the button's icon rect, so icons stay crisp at any
//! DPI without shipping image files. `paint` returns false when an id has
//! no icon, and the caller falls back to text only.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

const MARGIN: f32 = 0.14;

/// Maps normalized 0..1 icon-space coordinates onto a screen rect and
/// draws simple line-art primitives into it.
struct Canvas<'a> {
    painter: &'a Painter,
    rect: Rect,
    stroke: Stroke,
    color: Color32,
}

impl<'a> Canvas<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let w = (rect.width().min(rect.height()) * 0.1).max(1.1);
        Canvas { painter, rect, stroke: Stroke::new(w, color), color }
    }

    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let span = 1.0 - 2.0 * MARGIN;
        Pos2::new(
            self.rect.left() + self.rect.width() * (MARGIN + x * span),
            self.rect.top() + self.rect.height() * (MARGIN + y * span),
        )
    }

    fn len(&self, r: f32) -> f32 {
        r * self.rect.width().min(self.rect.height()) * (1.0 - 2.0 * MARGIN)
    }

    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.painter.line_segment([self.pt(a.0, a.1), self.pt(b.0, b.1)], self.stroke);
    }

    fn line_w(&self, a: (f32, f32), b: (f32, f32), w: f32) {
        self.painter.line_segment([self.pt(a.0, a.1), self.pt(b.0, b.1)], Stroke::new(w, self.color));
    }

    fn dashed(&self, a: (f32, f32), b: (f32, f32), n: usize) {
        let pa = self.pt(a.0, a.1);
        let pb = self.pt(b.0, b.1);
        for i in 0..n {
            if i % 2 == 0 {
                let t0 = i as f32 / n as f32;
                let t1 = (i as f32 + 1.0) / n as f32;
                self.painter.line_segment([pa + (pb - pa) * t0, pa + (pb - pa) * t1], self.stroke);
            }
        }
    }

    fn poly(&self, pts: &[(f32, f32)]) {
        for w in pts.windows(2) {
            self.line(w[0], w[1]);
        }
    }

    fn poly_closed(&self, pts: &[(f32, f32)]) {
        self.poly(pts);
        if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
            self.line(last, first);
        }
    }

    fn ngon(&self, center: (f32, f32), r: f32, n: usize, rot: f32) {
        let pts: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let t = rot + i as f32 * TAU / n as f32;
                (center.0 + r * t.cos(), center.1 + r * t.sin())
            })
            .collect();
        self.poly_closed(&pts);
    }

    fn circle(&self, c: (f32, f32), r: f32) {
        self.painter.circle_stroke(self.pt(c.0, c.1), self.len(r), self.stroke);
    }

    fn rect(&self, min: (f32, f32), max: (f32, f32)) {
        self.poly_closed(&[(min.0, min.1), (max.0, min.1), (max.0, max.1), (min.0, max.1)]);
    }

    fn rect_fill(&self, min: (f32, f32), max: (f32, f32)) {
        self.painter.rect_filled(Rect::from_two_pos(self.pt(min.0, min.1), self.pt(max.0, max.1)), 0.0, self.color);
    }

    fn dot(&self, p: (f32, f32)) {
        self.painter.circle_filled(self.pt(p.0, p.1), self.stroke.width * 0.9, self.color);
    }

    fn ellipse_arc(&self, c: (f32, f32), rx: f32, ry: f32, a0: f32, a1: f32) {
        let center = self.pt(c.0, c.1);
        let rrx = self.len(rx);
        let rry = self.len(ry);
        let n = 20;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = a0 + (a1 - a0) * (i as f32 / n as f32);
                center + Vec2::new(t.cos() * rrx, t.sin() * rry)
            })
            .collect();
        for w in pts.windows(2) {
            self.painter.line_segment([w[0], w[1]], self.stroke);
        }
    }

    fn ellipse(&self, c: (f32, f32), rx: f32, ry: f32) {
        self.ellipse_arc(c, rx, ry, 0.0, TAU);
    }

    fn arc_points(&self, c: (f32, f32), r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
        let center = self.pt(c.0, c.1);
        let rr = self.len(r);
        let n = 20;
        (0..=n)
            .map(|i| {
                let t = a0 + (a1 - a0) * (i as f32 / n as f32);
                center + Vec2::new(t.cos(), t.sin()) * rr
            })
            .collect()
    }

    fn arc(&self, c: (f32, f32), r: f32, a0: f32, a1: f32) {
        let pts = self.arc_points(c, r, a0, a1);
        for w in pts.windows(2) {
            self.painter.line_segment([w[0], w[1]], self.stroke);
        }
    }

    fn arrowhead(&self, tip: Pos2, dir: Vec2) {
        let head = (self.stroke.width * 3.2).max(4.0);
        let back = tip - dir * head;
        let perp = Vec2::new(-dir.y, dir.x) * head * 0.5;
        self.painter.line_segment([tip, back + perp], self.stroke);
        self.painter.line_segment([tip, back - perp], self.stroke);
    }

    fn arrow(&self, from: (f32, f32), to: (f32, f32)) {
        let a = self.pt(from.0, from.1);
        let b = self.pt(to.0, to.1);
        self.painter.line_segment([a, b], self.stroke);
        let dir = (b - a).normalized();
        self.arrowhead(b, dir);
    }

    /// Draws an arrowhead pointing at `at`, aimed along `from -> at`,
    /// without drawing the shaft (used to cap a line built some other way).
    fn arrowhead_at(&self, at: (f32, f32), from: (f32, f32)) {
        let a = self.pt(from.0, from.1);
        let b = self.pt(at.0, at.1);
        let dir = (b - a).normalized();
        self.arrowhead(b, dir);
    }

    fn arc_arrow(&self, c: (f32, f32), r: f32, a0: f32, a1: f32) {
        let pts = self.arc_points(c, r, a0, a1);
        for w in pts.windows(2) {
            self.painter.line_segment([w[0], w[1]], self.stroke);
        }
        if pts.len() >= 2 {
            let tip = pts[pts.len() - 1];
            let dir = (tip - pts[pts.len() - 2]).normalized();
            self.arrowhead(tip, dir);
        }
    }
}

/// A wireframe cube, used for box-shaped icons (primitive, iso view, ...).
fn draw_cube(c: &Canvas) {
    c.rect((0.16, 0.42), (0.58, 0.82));
    let (dx, dy) = (0.2, -0.2);
    c.line((0.16, 0.42), (0.16 + dx, 0.42 + dy));
    c.line((0.58, 0.42), (0.58 + dx, 0.42 + dy));
    c.line((0.58, 0.82), (0.58 + dx, 0.82 + dy));
    c.line((0.16 + dx, 0.42 + dy), (0.58 + dx, 0.42 + dy));
    c.line((0.58 + dx, 0.42 + dy), (0.58 + dx, 0.82 + dy));
    c.line((0.58 + dx, 0.82 + dy), (0.58, 0.82));
}

/// A corner made of two lines, rounded or chamfered where they meet.
fn draw_corner(c: &Canvas, rounded: bool) {
    c.line((0.25, 0.25), (0.25, 0.55));
    if rounded {
        c.arc((0.49, 0.55), 0.24, PI, PI * 1.5);
    } else {
        c.line((0.25, 0.55), (0.49, 0.79));
    }
    c.line((0.49, 0.79), (0.75, 0.79));
}

fn draw_kettle(c: &Canvas) {
    c.ellipse((0.45, 0.62), 0.3, 0.2);
    c.arc((0.45, 0.4), 0.3, PI, TAU);
    c.line((0.72, 0.48), (0.9, 0.34));
    c.arc((0.42, 0.32), 0.16, PI * 1.15, PI * 1.9);
}

/// Draws the icon for `id` into `rect`. Returns false when there is no
/// icon for `id`, so the caller can fall back to a text-only button.
pub fn paint(id: &str, painter: &Painter, rect: Rect, color: Color32) -> bool {
    let c = Canvas::new(painter, rect, color);
    match id {
        // ---- solid features ----
        "extrude" => {
            c.rect((0.18, 0.62), (0.58, 0.82));
            c.arrow((0.5, 0.55), (0.5, 0.14));
        }
        "revolve" => {
            c.rect((0.5, 0.35), (0.75, 0.65));
            c.line((0.5, 0.08), (0.5, 0.92));
            c.arc_arrow((0.5, 0.5), 0.42, -1.0, 2.0);
        }
        "press_pull" => {
            c.poly_closed(&[(0.15, 0.78), (0.62, 0.78), (0.85, 0.6), (0.38, 0.6)]);
            c.arrow((0.5, 0.55), (0.5, 0.12));
        }
        "sweep" => {
            c.circle((0.22, 0.78), 0.1);
            c.arc_arrow((0.22, 0.78), 0.5, -1.5, -0.1);
        }
        "loft" => {
            c.rect((0.15, 0.62), (0.4, 0.8));
            c.circle((0.68, 0.25), 0.16);
            c.line((0.4, 0.62), (0.58, 0.36));
            c.line((0.15, 0.72), (0.53, 0.32));
        }
        "hole" => {
            c.rect((0.15, 0.62), (0.85, 0.8));
            c.circle((0.5, 0.71), 0.14);
            c.dot((0.5, 0.71));
        }
        "shell" => {
            c.rect((0.16, 0.16), (0.84, 0.84));
            c.rect((0.32, 0.32), (0.68, 0.68));
        }
        "fillet" => draw_corner(&c, true),
        "chamfer" => draw_corner(&c, false),
        "combine" => {
            c.circle((0.4, 0.5), 0.26);
            c.circle((0.62, 0.5), 0.26);
        }
        "mirror" => {
            c.rect((0.12, 0.3), (0.4, 0.7));
            c.dashed((0.5, 0.1), (0.5, 0.9), 8);
            c.rect((0.6, 0.3), (0.88, 0.7));
        }
        "move" => {
            c.rect((0.2, 0.55), (0.5, 0.82));
            c.arrow((0.35, 0.45), (0.65, 0.18));
        }
        "scale" => {
            c.rect((0.32, 0.32), (0.68, 0.68));
            c.arrow((0.68, 0.68), (0.87, 0.87));
            c.arrow((0.32, 0.32), (0.13, 0.13));
        }
        "rect_pattern" => {
            for (dx, dy) in [(0.22, 0.22), (0.5, 0.22), (0.78, 0.22), (0.22, 0.5), (0.5, 0.5), (0.78, 0.5)] {
                c.rect((dx - 0.08, dy - 0.08), (dx + 0.08, dy + 0.08));
            }
        }
        "circ_pattern" => {
            for i in 0..6 {
                let a = i as f32 * TAU / 6.0;
                let (x, y) = (0.5 + 0.32 * a.cos(), 0.5 + 0.32 * a.sin());
                c.rect((x - 0.07, y - 0.07), (x + 0.07, y + 0.07));
            }
        }
        "box" => draw_cube(&c),
        "cylinder" => {
            c.ellipse((0.5, 0.28), 0.32, 0.12);
            c.line((0.18, 0.28), (0.18, 0.72));
            c.line((0.82, 0.28), (0.82, 0.72));
            c.ellipse_arc((0.5, 0.72), 0.32, 0.12, 0.0, PI);
        }
        "sphere" => {
            c.circle((0.5, 0.5), 0.36);
            c.ellipse((0.5, 0.5), 0.36, 0.12);
        }
        "torus" => {
            c.ellipse((0.5, 0.55), 0.38, 0.16);
            c.ellipse((0.5, 0.55), 0.2, 0.08);
        }
        "offset_plane" => {
            c.poly_closed(&[(0.15, 0.6), (0.55, 0.6), (0.68, 0.42), (0.28, 0.42)]);
            c.poly_closed(&[(0.3, 0.8), (0.7, 0.8), (0.83, 0.62), (0.43, 0.62)]);
            c.arrow((0.5, 0.55), (0.5, 0.72));
        }
        "angle_plane" => {
            c.poly_closed(&[(0.15, 0.75), (0.55, 0.75), (0.68, 0.57), (0.28, 0.57)]);
            c.poly_closed(&[(0.25, 0.65), (0.65, 0.4), (0.78, 0.22), (0.38, 0.47)]);
            c.arc((0.4, 0.6), 0.2, -1.5, -0.3);
        }
        "plane_3pt" => {
            c.poly_closed(&[(0.15, 0.7), (0.6, 0.7), (0.8, 0.45), (0.35, 0.45)]);
            c.dot((0.2, 0.66));
            c.dot((0.55, 0.66));
            c.dot((0.7, 0.5));
        }
        "midplane" => {
            c.poly_closed(&[(0.12, 0.35), (0.45, 0.35), (0.55, 0.2), (0.22, 0.2)]);
            c.poly_closed(&[(0.45, 0.85), (0.78, 0.85), (0.88, 0.7), (0.55, 0.7)]);
            c.dashed((0.3, 0.55), (0.65, 0.5), 6);
        }
        "mesh" => {
            c.poly_closed(&[(0.2, 0.75), (0.5, 0.25), (0.8, 0.75)]);
            c.line((0.35, 0.5), (0.65, 0.5));
            c.line((0.5, 0.25), (0.5, 0.75));
        }
        "split_body" => {
            c.rect((0.18, 0.28), (0.82, 0.75));
            c.dashed((0.1, 0.5), (0.9, 0.5), 8);
        }
        "text" => {
            c.line((0.2, 0.3), (0.75, 0.3));
            c.line((0.2, 0.5), (0.6, 0.5));
            c.line((0.2, 0.7), (0.68, 0.7));
        }
        "qr" => {
            for (x, y) in [
                (0.15, 0.15),
                (0.42, 0.15),
                (0.69, 0.15),
                (0.15, 0.42),
                (0.69, 0.42),
                (0.15, 0.69),
                (0.42, 0.69),
                (0.69, 0.69),
            ] {
                c.rect_fill((x, y), (x + 0.16, y + 0.16));
            }
        }
        "texture" => {
            for gy in 0..3 {
                for gx in 0..3 {
                    c.dot((0.2 + gx as f32 * 0.3, 0.2 + gy as f32 * 0.3));
                }
            }
        }
        "coil" => {
            let mut prev: Option<(f32, f32)> = None;
            for i in 0..=24 {
                let t = i as f32 / 24.0;
                let p = (0.15 + t * 0.7, 0.5 + 0.28 * (t * TAU * 2.5).sin());
                if let Some(pp) = prev {
                    c.line(pp, p);
                }
                prev = Some(p);
            }
        }
        "pipe" => {
            c.ellipse((0.22, 0.5), 0.14, 0.3);
            c.line((0.22, 0.2), (0.85, 0.2));
            c.line((0.22, 0.8), (0.85, 0.8));
            c.ellipse_arc((0.85, 0.5), 0.14, 0.3, -PI / 2.0, PI / 2.0);
        }
        "sketch" => {
            c.poly_closed(&[(0.2, 0.75), (0.2, 0.35), (0.55, 0.2), (0.8, 0.4), (0.65, 0.75)]);
            c.dot((0.2, 0.75));
            c.dot((0.2, 0.35));
            c.dot((0.55, 0.2));
        }
        "aircraft" => {
            c.line((0.1, 0.5), (0.9, 0.5));
            c.line((0.35, 0.5), (0.15, 0.75));
            c.line((0.35, 0.5), (0.15, 0.25));
            c.line((0.75, 0.5), (0.85, 0.65));
        }
        "density_body" => {
            for gy in 0..3 {
                for gx in 0..3 {
                    let (x, y) = (0.18 + gx as f32 * 0.32, 0.18 + gy as f32 * 0.32);
                    if (gx + gy) % 2 == 0 {
                        c.rect_fill((x, y), (x + 0.24, y + 0.24));
                    } else {
                        c.rect((x, y), (x + 0.24, y + 0.24));
                    }
                }
            }
        }
        "lattice_fill" => {
            c.rect((0.15, 0.15), (0.85, 0.85));
            c.line((0.15, 0.15), (0.85, 0.85));
            c.line((0.85, 0.15), (0.15, 0.85));
            c.line((0.5, 0.15), (0.5, 0.85));
            c.line((0.15, 0.5), (0.85, 0.5));
        }
        "surface_pattern" => {
            c.arc((0.5, 1.1), 0.65, -2.4, -0.7);
            for i in 0..5 {
                let t = i as f32 / 4.0;
                c.dot((0.2 + t * 0.6, 0.45));
            }
        }
        "sprue" => {
            c.poly_closed(&[(0.25, 0.2), (0.75, 0.2), (0.6, 0.55), (0.4, 0.55)]);
            c.line((0.45, 0.55), (0.45, 0.85));
            c.line((0.55, 0.55), (0.55, 0.85));
        }
        "runner" => {
            c.rect((0.12, 0.42), (0.7, 0.58));
            c.arrow((0.55, 0.5), (0.85, 0.5));
        }
        "riser" => {
            c.arc((0.5, 0.16), 0.16, PI, TAU);
            c.ellipse((0.5, 0.3), 0.24, 0.09);
            c.line((0.26, 0.3), (0.26, 0.75));
            c.line((0.74, 0.3), (0.74, 0.75));
            c.ellipse_arc((0.5, 0.75), 0.24, 0.09, 0.0, PI);
        }
        "draft_check" => {
            c.poly_closed(&[(0.5, 0.15), (0.85, 0.8), (0.15, 0.8)]);
            c.line((0.5, 0.38), (0.5, 0.62));
            c.dot((0.5, 0.72));
        }
        "casting_check" => {
            c.circle((0.5, 0.5), 0.38);
            c.line((0.32, 0.5), (0.45, 0.65));
            c.line((0.45, 0.65), (0.72, 0.32));
        }
        "finish_sketch" => {
            c.line((0.15, 0.5), (0.4, 0.78));
            c.line((0.4, 0.78), (0.85, 0.2));
        }
        "offset" => {
            c.poly(&[(0.2, 0.75), (0.35, 0.3), (0.65, 0.3), (0.8, 0.75)]);
            c.dashed((0.12, 0.85), (0.3, 0.4), 4);
            c.dashed((0.7, 0.4), (0.88, 0.85), 4);
        }
        "template" => {
            c.dashed((0.2, 0.2), (0.8, 0.2), 6);
            c.dashed((0.8, 0.2), (0.8, 0.8), 6);
            c.dashed((0.8, 0.8), (0.2, 0.8), 6);
            c.dashed((0.2, 0.8), (0.2, 0.2), 6);
            c.dot((0.5, 0.5));
        }

        // ---- app actions ----
        "undo" => c.arc_arrow((0.62, 0.6), 0.32, 3.9, 1.25),
        "redo" => c.arc_arrow((0.38, 0.6), 0.32, -0.9, 1.65),
        "new_document" => {
            c.poly_closed(&[(0.28, 0.15), (0.6, 0.15), (0.75, 0.32), (0.75, 0.85), (0.28, 0.85)]);
            c.line((0.6, 0.15), (0.6, 0.32));
            c.line((0.6, 0.32), (0.75, 0.32));
            c.line((0.4, 0.55), (0.63, 0.55));
            c.line((0.4, 0.68), (0.63, 0.68));
        }
        "save" => {
            c.rect((0.18, 0.18), (0.82, 0.82));
            c.rect_fill((0.32, 0.18), (0.62, 0.35));
            c.rect((0.3, 0.5), (0.7, 0.78));
        }
        "save_as" => {
            c.rect((0.18, 0.18), (0.82, 0.82));
            c.rect_fill((0.32, 0.18), (0.62, 0.35));
            c.rect((0.3, 0.5), (0.7, 0.78));
            c.line((0.78, 0.1), (0.78, 0.24));
            c.line((0.71, 0.17), (0.85, 0.17));
        }
        "load" => {
            c.poly_closed(&[(0.12, 0.35), (0.4, 0.35), (0.48, 0.25), (0.7, 0.25), (0.7, 0.35)]);
            c.poly_closed(&[(0.12, 0.35), (0.85, 0.35), (0.75, 0.78), (0.2, 0.78)]);
        }
        "export" => {
            c.rect((0.18, 0.15), (0.55, 0.85));
            c.arrow((0.55, 0.5), (0.85, 0.5));
        }
        "fit_view" => {
            for (cx, cy, sx, sy) in
                [(0.15, 0.15, 1.0, 1.0), (0.85, 0.15, -1.0, 1.0), (0.15, 0.85, 1.0, -1.0), (0.85, 0.85, -1.0, -1.0)]
            {
                c.line((cx, cy), (cx + 0.18 * sx, cy));
                c.line((cx, cy), (cx, cy + 0.18 * sy));
            }
        }
        "demo_part" => {
            draw_cube(&c);
            c.dot((0.5, 0.6));
        }
        "export_gcode" => {
            c.rect((0.15, 0.15), (0.55, 0.85));
            c.line((0.6, 0.3), (0.7, 0.3));
            c.line((0.7, 0.3), (0.7, 0.5));
            c.line((0.7, 0.5), (0.85, 0.5));
            c.line((0.85, 0.5), (0.85, 0.7));
        }
        "edit_sketch" => {
            c.line((0.2, 0.8), (0.65, 0.35));
            c.line((0.65, 0.35), (0.8, 0.2));
            c.line((0.2, 0.8), (0.28, 0.68));
        }
        "measure" => {
            c.rect((0.15, 0.4), (0.85, 0.6));
            for i in 0..6 {
                let x = 0.2 + i as f32 * 0.12;
                c.line((x, 0.4), (x, 0.5));
            }
        }
        "toggle_edges" => {
            draw_cube(&c);
            c.line_w((0.16, 0.42), (0.58, 0.42), c.stroke.width * 1.9);
        }
        "toggle_perf" => {
            // A small bar chart: CPU, memory, frames.
            c.line((0.1, 0.9), (0.9, 0.9));
            c.rect_fill((0.18, 0.55), (0.32, 0.88));
            c.rect_fill((0.43, 0.3), (0.57, 0.88));
            c.rect_fill((0.68, 0.45), (0.82, 0.88));
        }
        "projection" => {
            // A box drawn straight, and the same box in perspective.
            c.rect((0.08, 0.3), (0.42, 0.72));
            c.poly_closed(&[(0.58, 0.24), (0.94, 0.36), (0.94, 0.66), (0.58, 0.78)]);
        }
        "up_axis" => {
            c.arrow((0.5, 0.9), (0.5, 0.12));
            c.line((0.2, 0.9), (0.8, 0.9));
        }
        "section_analysis" => {
            // A cut block with the cut face hatched.
            c.rect((0.12, 0.2), (0.88, 0.8));
            c.line((0.5, 0.2), (0.5, 0.8));
            for i in 0..3 {
                let t = 0.3 + i as f32 * 0.18;
                c.line((0.52, t + 0.1), (0.86, t - 0.08));
            }
        }
        "benchmark" => {
            // A stopwatch.
            c.circle((0.5, 0.56), 0.32);
            c.line((0.5, 0.56), (0.5, 0.3));
            c.line((0.38, 0.12), (0.62, 0.12));
            c.line((0.5, 0.12), (0.5, 0.24));
        }
        "toggle_gpu" => {
            // A graphics chip: a square die with pins.
            c.rect((0.24, 0.24), (0.76, 0.76));
            c.rect_fill((0.38, 0.38), (0.62, 0.62));
            for i in 0..3 {
                let t = 0.34 + i as f32 * 0.16;
                c.line((t, 0.1), (t, 0.24));
                c.line((t, 0.76), (t, 0.9));
                c.line((0.1, t), (0.24, t));
                c.line((0.76, t), (0.9, t));
            }
        }
        "view_quad" => {
            c.rect((0.12, 0.12), (0.88, 0.88));
            c.line((0.5, 0.12), (0.5, 0.88));
            c.line((0.12, 0.5), (0.88, 0.5));
        }
        "toggle_scroll" => {
            // A mouse with arrows both ways.
            c.rect((0.32, 0.16), (0.68, 0.84));
            c.line((0.5, 0.26), (0.5, 0.42));
            c.arrow((0.16, 0.6), (0.16, 0.3));
            c.arrow((0.84, 0.4), (0.84, 0.7));
        }
        "settings" => {
            c.circle((0.5, 0.5), 0.16);
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                let (x0, y0) = (0.5 + 0.24 * a.cos(), 0.5 + 0.24 * a.sin());
                let (x1, y1) = (0.5 + 0.4 * a.cos(), 0.5 + 0.4 * a.sin());
                c.line((x0, y0), (x1, y1));
            }
        }
        "view_iso" => draw_cube(&c),
        "view_top" => {
            c.rect((0.2, 0.2), (0.8, 0.8));
            c.arrow((0.5, 0.02), (0.5, 0.17));
        }
        "view_front" => {
            c.rect((0.2, 0.2), (0.8, 0.8));
            c.arrow((0.5, 0.98), (0.5, 0.83));
        }
        "view_right" => {
            c.rect((0.2, 0.2), (0.8, 0.8));
            c.arrow((0.98, 0.5), (0.83, 0.5));
        }
        "delete_feature" => {
            c.line((0.25, 0.28), (0.75, 0.28));
            c.line((0.35, 0.28), (0.35, 0.2));
            c.line((0.65, 0.28), (0.65, 0.2));
            c.line((0.35, 0.2), (0.65, 0.2));
            c.poly_closed(&[(0.3, 0.28), (0.7, 0.28), (0.65, 0.85), (0.35, 0.85)]);
            c.line((0.45, 0.4), (0.45, 0.75));
            c.line((0.55, 0.4), (0.55, 0.75));
        }
        "compute_all" => {
            c.arc_arrow((0.5, 0.5), 0.3, -0.3, 2.6);
            c.arc_arrow((0.5, 0.5), 0.3, 2.84, 5.7);
        }
        "center_of_mass" => {
            c.circle((0.5, 0.5), 0.32);
            c.circle((0.5, 0.5), 0.1);
            c.dot((0.5, 0.5));
            c.line((0.5, 0.1), (0.5, 0.22));
            c.line((0.5, 0.78), (0.5, 0.9));
            c.line((0.1, 0.5), (0.22, 0.5));
            c.line((0.78, 0.5), (0.9, 0.5));
        }
        "bill_of_materials" => {
            for i in 0..3 {
                let y = 0.28 + i as f32 * 0.24;
                c.dot((0.2, y));
                c.line((0.32, y), (0.82, y));
            }
        }
        "toggle_units" => {
            c.rect((0.12, 0.4), (0.45, 0.6));
            c.rect((0.55, 0.4), (0.88, 0.6));
            c.line((0.5, 0.3), (0.5, 0.7));
        }
        "sample_card" => {
            c.rect((0.12, 0.28), (0.88, 0.72));
            c.circle((0.28, 0.5), 0.1);
            c.line((0.45, 0.42), (0.78, 0.42));
            c.line((0.45, 0.58), (0.7, 0.58));
        }
        "workbook" => {
            c.line((0.5, 0.2), (0.5, 0.8));
            c.poly(&[(0.5, 0.2), (0.15, 0.28), (0.15, 0.78), (0.5, 0.8)]);
            c.poly(&[(0.5, 0.2), (0.85, 0.28), (0.85, 0.78), (0.5, 0.8)]);
        }
        "kettle" => draw_kettle(&c),
        "kettle_gated" => {
            draw_kettle(&c);
            c.poly_closed(&[(0.35, 0.06), (0.55, 0.06), (0.48, 0.2), (0.42, 0.2)]);
        }
        "kettle_mold" => {
            draw_kettle(&c);
            c.dashed((0.45, 0.15), (0.45, 0.85), 8);
        }
        "interference" => {
            c.circle((0.38, 0.5), 0.28);
            c.circle((0.62, 0.5), 0.28);
            c.line((0.45, 0.38), (0.55, 0.62));
            c.line((0.5, 0.35), (0.6, 0.58));
            c.line((0.4, 0.42), (0.5, 0.65));
        }

        // ---- sketch tools ----
        "tool_select" => {
            c.poly_closed(&[(0.2, 0.15), (0.2, 0.78), (0.4, 0.6), (0.5, 0.85), (0.6, 0.8), (0.5, 0.55), (0.75, 0.55)]);
        }
        "tool_point" => {
            c.circle((0.5, 0.5), 0.1);
            c.dot((0.5, 0.5));
        }
        "tool_line" => {
            c.line((0.18, 0.82), (0.82, 0.18));
            c.dot((0.18, 0.82));
            c.dot((0.82, 0.18));
        }
        "tool_midpoint_line" => {
            c.line((0.18, 0.82), (0.82, 0.18));
            c.dot((0.18, 0.82));
            c.dot((0.82, 0.18));
            c.dot((0.5, 0.5));
        }
        "tool_rect2" => {
            c.rect((0.18, 0.25), (0.82, 0.75));
            c.dot((0.18, 0.25));
            c.dot((0.82, 0.75));
        }
        "tool_rect_center" => {
            c.rect((0.22, 0.28), (0.78, 0.72));
            c.dot((0.5, 0.5));
            c.line((0.5, 0.45), (0.5, 0.55));
            c.line((0.45, 0.5), (0.55, 0.5));
        }
        "tool_rect3" => {
            c.poly_closed(&[(0.15, 0.7), (0.6, 0.82), (0.82, 0.35), (0.37, 0.2)]);
        }
        "tool_circle_center" => {
            c.circle((0.5, 0.5), 0.34);
            c.dot((0.5, 0.5));
            c.line((0.5, 0.5), (0.5, 0.16));
        }
        "tool_circle2" => {
            c.circle((0.5, 0.5), 0.34);
            c.dot((0.18, 0.5));
            c.dot((0.82, 0.5));
            c.line((0.18, 0.5), (0.82, 0.5));
        }
        "tool_circle3" => {
            c.circle((0.5, 0.5), 0.34);
            c.dot((0.5, 0.16));
            c.dot((0.2, 0.66));
            c.dot((0.8, 0.66));
        }
        "tool_arc3" => {
            c.arc((0.5, 0.95), 0.42, PI * 1.15, PI * 1.85);
            c.dot((0.16, 0.6));
            c.dot((0.5, 0.38));
            c.dot((0.84, 0.6));
        }
        "tool_arc_center" => {
            c.arc((0.5, 0.65), 0.32, PI * 1.1, PI * 1.9);
            c.dot((0.5, 0.65));
            c.line((0.5, 0.65), (0.22, 0.65));
            c.line((0.5, 0.65), (0.5, 0.33));
        }
        "tool_polygon" => c.ngon((0.5, 0.5), 0.36, 6, -PI / 2.0),
        "tool_polygon_inscribed" => {
            c.ngon((0.5, 0.5), 0.3, 6, -PI / 2.0);
            c.circle((0.5, 0.5), 0.4);
        }
        "tool_polygon_edge" => {
            c.ngon((0.5, 0.5), 0.34, 6, -PI / 2.0);
            c.dot((0.5, 0.16));
            c.dot((0.79, 0.34));
        }
        "tool_ellipse" => c.ellipse((0.5, 0.5), 0.38, 0.24),
        "tool_slot" => {
            c.line((0.3, 0.3), (0.7, 0.3));
            c.line((0.3, 0.7), (0.7, 0.7));
            c.arc((0.3, 0.5), 0.2, PI * 0.5, PI * 1.5);
            c.arc((0.7, 0.5), 0.2, -PI * 0.5, PI * 0.5);
        }
        "tool_slot_center" => {
            c.line((0.3, 0.3), (0.7, 0.3));
            c.line((0.3, 0.7), (0.7, 0.7));
            c.arc((0.3, 0.5), 0.2, PI * 0.5, PI * 1.5);
            c.arc((0.7, 0.5), 0.2, -PI * 0.5, PI * 0.5);
            c.dot((0.5, 0.5));
        }
        "tool_spline" => {
            let pts = [(0.15, 0.75), (0.35, 0.25), (0.6, 0.8), (0.85, 0.3)];
            c.poly(&pts);
            for p in pts {
                c.dot(p);
            }
        }
        "tool_fillet" => draw_corner(&c, true),
        "tool_chamfer" => draw_corner(&c, false),
        "tool_trim" => {
            c.line((0.15, 0.75), (0.38, 0.52));
            c.line((0.62, 0.48), (0.85, 0.25));
            c.line((0.4, 0.4), (0.6, 0.6));
            c.line((0.4, 0.6), (0.6, 0.4));
        }
        "tool_extend" => {
            c.line((0.15, 0.75), (0.5, 0.4));
            c.dashed((0.5, 0.4), (0.72, 0.18), 4);
            c.arrowhead_at((0.72, 0.18), (0.5, 0.4));
        }
        "tool_mirror" => {
            c.line((0.15, 0.7), (0.4, 0.3));
            c.dashed((0.5, 0.1), (0.5, 0.9), 8);
            c.line((0.6, 0.3), (0.85, 0.7));
        }
        "tool_move_copy" => {
            c.rect((0.15, 0.5), (0.4, 0.75));
            c.arrow((0.42, 0.55), (0.68, 0.32));
            c.rect((0.6, 0.25), (0.85, 0.5));
        }
        "tool_scale" => {
            c.rect((0.32, 0.32), (0.68, 0.68));
            c.arrow((0.68, 0.68), (0.87, 0.87));
            c.arrow((0.32, 0.32), (0.13, 0.13));
        }
        "tool_pattern_rect" => {
            for (dx, dy) in [(0.25, 0.25), (0.5, 0.25), (0.75, 0.25), (0.25, 0.5), (0.5, 0.5), (0.75, 0.5)] {
                c.dot((dx, dy));
            }
        }
        "tool_pattern_circ" => {
            for i in 0..6 {
                let a = i as f32 * TAU / 6.0;
                c.dot((0.5 + 0.32 * a.cos(), 0.5 + 0.32 * a.sin()));
            }
        }
        "tool_dimension" => {
            c.line((0.15, 0.5), (0.85, 0.5));
            c.line((0.15, 0.35), (0.15, 0.65));
            c.line((0.85, 0.35), (0.85, 0.65));
            c.arrowhead_at((0.15, 0.5), (0.35, 0.5));
            c.arrowhead_at((0.85, 0.5), (0.65, 0.5));
        }
        "tool_tangent_arc" => {
            c.line((0.15, 0.75), (0.45, 0.55));
            c.arc((0.6, 0.55), 0.2, PI, PI * 1.8);
            c.dot((0.45, 0.55));
        }

        // ---- sketch constraints ----
        "constraint_coincident" => {
            c.circle((0.5, 0.5), 0.28);
            c.dot((0.5, 0.5));
        }
        "constraint_collinear" => {
            c.line((0.1, 0.7), (0.9, 0.3));
            c.dot((0.3, 0.62));
            c.dot((0.7, 0.38));
        }
        "constraint_horizontal" => {
            c.line((0.15, 0.5), (0.85, 0.5));
            c.arrowhead_at((0.85, 0.5), (0.7, 0.5));
            c.arrowhead_at((0.15, 0.5), (0.3, 0.5));
        }
        "constraint_vertical" => {
            c.line((0.5, 0.15), (0.5, 0.85));
            c.arrowhead_at((0.5, 0.85), (0.5, 0.7));
            c.arrowhead_at((0.5, 0.15), (0.5, 0.3));
        }
        "constraint_parallel" => {
            c.line((0.2, 0.78), (0.45, 0.22));
            c.line((0.55, 0.78), (0.8, 0.22));
        }
        "constraint_perpendicular" => {
            c.line((0.2, 0.5), (0.8, 0.5));
            c.line((0.35, 0.2), (0.35, 0.8));
            c.line((0.35, 0.5), (0.44, 0.5));
            c.line((0.44, 0.5), (0.44, 0.41));
        }
        "constraint_equal" => {
            c.line((0.15, 0.32), (0.45, 0.32));
            c.line((0.15, 0.42), (0.45, 0.42));
            c.line((0.55, 0.62), (0.85, 0.62));
            c.line((0.55, 0.72), (0.85, 0.72));
        }
        "constraint_tangent" => {
            c.circle((0.5, 0.6), 0.26);
            c.line((0.15, 0.34), (0.85, 0.34));
        }
        "constraint_midpoint" => {
            c.line((0.15, 0.75), (0.85, 0.25));
            c.circle((0.5, 0.5), 0.08);
        }
        "constraint_concentric" => {
            c.circle((0.5, 0.5), 0.36);
            c.circle((0.5, 0.5), 0.18);
        }
        "constraint_symmetric" => {
            c.dashed((0.5, 0.12), (0.5, 0.88), 8);
            c.dot((0.28, 0.5));
            c.dot((0.72, 0.5));
            c.line((0.28, 0.5), (0.42, 0.5));
            c.line((0.58, 0.5), (0.72, 0.5));
        }
        "constraint_point_on_curve" => {
            c.arc((0.5, 0.9), 0.5, PI * 1.15, PI * 1.85);
            c.dot((0.5, 0.4));
        }
        "constraint_fix" => {
            c.circle((0.5, 0.3), 0.14);
            c.line((0.5, 0.44), (0.5, 0.75));
            c.line((0.35, 0.75), (0.65, 0.75));
            c.line((0.5, 0.75), (0.35, 0.9));
            c.line((0.5, 0.75), (0.65, 0.9));
        }

        // ---- dimension tools ----
        "dim_length" => {
            c.line((0.12, 0.5), (0.88, 0.5));
            c.line((0.12, 0.38), (0.12, 0.62));
            c.line((0.88, 0.38), (0.88, 0.62));
            c.arrowhead_at((0.12, 0.5), (0.3, 0.5));
            c.arrowhead_at((0.88, 0.5), (0.7, 0.5));
        }
        "dim_radius" => {
            c.circle((0.5, 0.5), 0.34);
            c.line((0.5, 0.5), (0.74, 0.26));
            c.arrowhead_at((0.74, 0.26), (0.6, 0.4));
        }
        "dim_angle" => {
            c.line((0.2, 0.75), (0.75, 0.75));
            c.line((0.2, 0.75), (0.6, 0.25));
            c.arc((0.2, 0.75), 0.25, -1.4, 0.0);
        }
        "dim_smart" => {
            c.line((0.15, 0.65), (0.85, 0.65));
            c.arrowhead_at((0.15, 0.65), (0.3, 0.65));
            c.arrowhead_at((0.85, 0.65), (0.7, 0.65));
            c.circle((0.5, 0.3), 0.12);
        }

        _ => return false,
    }
    true
}

/// Draws a button whose id has an icon, with the icon above the label.
/// Falls back to a centered text label when `id` has no icon.
pub fn icon_button(ui: &mut egui::Ui, id: &str, label: &str) -> egui::Response {
    button_core(ui, id, label, true, false, egui::vec2(58.0, 42.0))
}

/// Like `icon_button`, but drawn with the ribbon's selected styling when
/// `selected` is true (used for the active sketch tool).
pub fn icon_toggle(ui: &mut egui::Ui, id: &str, label: &str, selected: bool) -> egui::Response {
    button_core(ui, id, label, true, selected, egui::vec2(58.0, 42.0))
}

/// A compact, icon-only button with no visible label (the caller still
/// adds a tooltip with the full name). Used for dense rows such as sketch
/// constraints, where a label per button would not fit at 1280px wide.
pub fn icon_only(ui: &mut egui::Ui, id: &str, label: &str) -> egui::Response {
    button_core(ui, id, label, false, false, egui::vec2(30.0, 30.0))
}

/// Font size for the command name shown at the top of every icon button's
/// tooltip, clearly larger than the surrounding UI text.
const TOOLTIP_NAME_SIZE: f32 = 17.0;

fn button_core(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    show_label: bool,
    selected: bool,
    min_size: egui::Vec2,
) -> egui::Response {
    let icon_size = crate::settings::icon_size(ui.ctx());
    // The button grows with the icon, so a bigger icon is not squeezed
    // into a button sized for the old one.
    let min_size = min_size * (icon_size / crate::settings::BASE_ICON);
    let padding = ui.spacing().button_padding;
    let font_id = egui::TextStyle::Small.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let galley = show_label.then(|| ui.painter().layout_no_wrap(label.to_string(), font_id.clone(), text_color));
    let content = match &galley {
        Some(g) => egui::vec2(g.size().x.max(icon_size), icon_size + 3.0 + g.size().y),
        None => egui::vec2(icon_size, icon_size),
    };
    let desired = (content + padding * 2.0).max(min_size);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        ui.painter().rect(
            rect.expand(visuals.expansion),
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + padding.y + icon_size / 2.0),
            egui::vec2(icon_size, icon_size),
        );
        let drew = paint(id, ui.painter(), icon_rect, visuals.text_color());
        if let Some(g) = galley {
            let pos = Pos2::new(rect.center().x - g.size().x / 2.0, icon_rect.bottom() + 3.0);
            ui.painter().galley(pos, g, visuals.text_color());
        } else if !drew {
            let g = ui.painter().layout_no_wrap(label.to_string(), font_id, visuals.text_color());
            let pos = rect.center() - g.size() / 2.0;
            ui.painter().galley(pos, g, visuals.text_color());
        }
    }
    let name = label.to_string();
    response.on_hover_ui(move |ui| {
        ui.label(egui::RichText::new(&name).size(TOOLTIP_NAME_SIZE).strong());
    })
}

// ---------------------------------------------------------------------
// Logo
// ---------------------------------------------------------------------

/// Outline of the anvil silhouette, in a normalized 0..1 square (x right,
/// y down), listed clockwise from the horn tip. Shared by `paint_logo`
/// (drawn with the Painter) and `logo_rgba` (rasterized for the window
/// icon), so both agree on the same shape.
const LOGO_POLY: &[(f32, f32)] = &[
    (0.05, 0.40),
    (0.30, 0.30),
    (0.86, 0.30),
    (0.90, 0.36),
    (0.72, 0.46),
    (0.64, 0.58),
    (0.82, 0.82),
    (0.82, 0.90),
    (0.18, 0.90),
    (0.18, 0.82),
    (0.36, 0.58),
    (0.30, 0.46),
    (0.20, 0.44),
];

/// A thin highlight band along the top face, for the "subtle highlight"
/// called for by the logo brief.
const LOGO_HIGHLIGHT: &[(f32, f32)] = &[(0.32, 0.305), (0.84, 0.305), (0.84, 0.325), (0.32, 0.325)];

const LOGO_DARK: Color32 = Color32::from_rgb(58, 62, 68);
const LOGO_LIGHT: Color32 = Color32::from_rgb(150, 155, 160);
const LOGO_OUTLINE: Color32 = Color32::from_rgb(30, 32, 36);

/// x-intersections of the horizontal line `y` with a polygon's edges,
/// in the polygon's own coordinate space, sorted left to right.
fn scanline(poly: &[(f32, f32)], y: f32) -> Vec<f32> {
    let mut xs = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % n];
        if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
            let t = (y - y0) / (y1 - y0);
            xs.push(x0 + t * (x1 - x0));
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs
}

/// Standard ray-casting point-in-polygon test, in the polygon's own
/// coordinate space.
fn point_in_polygon(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) {
            let x_int = xi + (y - yi) / (yj - yi) * (xj - xi);
            if x < x_int {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Paints the anvil logo (dark steel with a highlighted top face) into
/// `rect`, filled with a scanline sweep so the silhouette (which is not
/// convex) renders correctly.
pub fn paint_logo(painter: &Painter, rect: Rect) {
    let margin = 0.08;
    let to_screen = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            rect.left() + rect.width() * (margin + x * (1.0 - 2.0 * margin)),
            rect.top() + rect.height() * (margin + y * (1.0 - 2.0 * margin)),
        )
    };
    let rows = 64usize;
    let fill = |poly: &[(f32, f32)], color: Color32| {
        for i in 0..rows {
            let y = (i as f32 + 0.5) / rows as f32;
            let xs = scanline(poly, y);
            let mut k = 0;
            while k + 1 < xs.len() {
                let top = to_screen(xs[k], y - 0.5 / rows as f32);
                let bot = to_screen(xs[k + 1], y + 0.5 / rows as f32);
                painter.rect_filled(Rect::from_two_pos(top, bot), 0.0, color);
                k += 2;
            }
        }
    };
    fill(LOGO_POLY, LOGO_DARK);
    fill(LOGO_HIGHLIGHT, LOGO_LIGHT);
    let stroke = Stroke::new((rect.width() * 0.02).max(1.0), LOGO_OUTLINE);
    let pts: Vec<Pos2> = LOGO_POLY.iter().map(|&(x, y)| to_screen(x, y)).collect();
    for i in 0..pts.len() {
        painter.line_segment([pts[i], pts[(i + 1) % pts.len()]], stroke);
    }
}

/// Rasterizes the same anvil silhouette as `paint_logo` into a square
/// `size x size` RGBA buffer, for the window icon. Antialiased with 4x4
/// supersampling, no image crate involved.
pub fn logo_rgba(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut buf = vec![0u8; n * n * 4];
    let margin = 0.08_f32;
    let ss = 4usize;
    for py in 0..n {
        for px in 0..n {
            let mut cov = 0u32;
            let mut hi_cov = 0u32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let fx = (px as f32 + (sx as f32 + 0.5) / ss as f32) / n as f32;
                    let fy = (py as f32 + (sy as f32 + 0.5) / ss as f32) / n as f32;
                    let lx = (fx - margin) / (1.0 - 2.0 * margin);
                    let ly = (fy - margin) / (1.0 - 2.0 * margin);
                    if !(0.0..=1.0).contains(&lx) || !(0.0..=1.0).contains(&ly) {
                        continue;
                    }
                    if point_in_polygon(LOGO_POLY, lx, ly) {
                        cov += 1;
                        if point_in_polygon(LOGO_HIGHLIGHT, lx, ly) {
                            hi_cov += 1;
                        }
                    }
                }
            }
            let total = (ss * ss) as f32;
            let a = (cov as f32 / total * 255.0).round() as u8;
            let t = if cov > 0 { hi_cov as f32 / cov as f32 } else { 0.0 };
            let r = LOGO_DARK.r() as f32 + (LOGO_LIGHT.r() as f32 - LOGO_DARK.r() as f32) * t;
            let g = LOGO_DARK.g() as f32 + (LOGO_LIGHT.g() as f32 - LOGO_DARK.g() as f32) * t;
            let b = LOGO_DARK.b() as f32 + (LOGO_LIGHT.b() as f32 - LOGO_DARK.b() as f32) * t;
            let idx = (py * n + px) * 4;
            buf[idx] = r.round() as u8;
            buf[idx + 1] = g.round() as u8;
            buf[idx + 2] = b.round() as u8;
            buf[idx + 3] = a;
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::build_ribbon;
    use crate::sketch_editor::{ConstraintTool, DimensionTool, Tool};

    fn test_painter() -> Painter {
        Painter::new(
            egui::Context::default(),
            egui::LayerId::debug(),
            Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 1000.0)),
        )
    }

    /// Ids that intentionally have no icon (falls back to text). Empty
    /// today: every ribbon button and sketch tool below has one.
    const EXCEPTIONS: &[&str] = &[];

    #[test]
    fn every_ribbon_button_has_an_icon() {
        let painter = test_painter();
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(20.0, 20.0));
        let mut missing = Vec::new();
        for tab in build_ribbon() {
            for g in tab.groups {
                for b in g.buttons {
                    let id = b.kind.icon_id();
                    if EXCEPTIONS.contains(&id) {
                        continue;
                    }
                    if !paint(id, &painter, rect, Color32::WHITE) {
                        missing.push(id.to_string());
                    }
                }
            }
        }
        assert!(missing.is_empty(), "ribbon buttons missing icons: {missing:?}");
    }

    #[test]
    fn every_sketch_tool_has_an_icon() {
        let painter = test_painter();
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(20.0, 20.0));
        let mut missing = Vec::new();
        for t in Tool::ALL {
            let id = t.icon_id();
            if !EXCEPTIONS.contains(&id) && !paint(id, &painter, rect, Color32::WHITE) {
                missing.push(id.to_string());
            }
        }
        for c in ConstraintTool::ALL {
            let id = c.icon_id();
            if !EXCEPTIONS.contains(&id) && !paint(id, &painter, rect, Color32::WHITE) {
                missing.push(id.to_string());
            }
        }
        for d in DimensionTool::ALL {
            let id = d.icon_id();
            if !EXCEPTIONS.contains(&id) && !paint(id, &painter, rect, Color32::WHITE) {
                missing.push(id.to_string());
            }
        }
        assert!(missing.is_empty(), "sketch tools missing icons: {missing:?}");
    }

    #[test]
    fn unknown_id_falls_back_to_text() {
        let painter = test_painter();
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(20.0, 20.0));
        assert!(!paint("not_a_real_icon", &painter, rect, Color32::WHITE));
    }

    #[test]
    fn logo_rgba_has_visible_pixels_and_correct_size() {
        let size = 32u32;
        let buf = logo_rgba(size);
        assert_eq!(buf.len(), (size * size * 4) as usize);
        let opaque = buf.chunks(4).filter(|px| px[3] > 200).count();
        // The anvil should cover a meaningful fraction of the square, not
        // be empty or fill the whole canvas.
        let total = (size * size) as usize;
        assert!(opaque > total / 10, "logo looks empty: {opaque}/{total} opaque pixels");
        assert!(opaque < total * 9 / 10, "logo looks like a solid block: {opaque}/{total} opaque pixels");
    }
}
