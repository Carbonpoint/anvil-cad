//! Sketches in the model view: their curves after Finish Sketch, and the
//! regions that a feature such as Extrude or Revolve can pick.

use crate::camera::Projector;
use crate::raster::Framebuffer;
use anvil_feature::param::{ParamKind, ParamValue};
use anvil_feature::region_select::{self, Region};
use anvil_feature::Document;
use anvil_math::{DVec2, Plane};
use egui::{Color32, Pos2, Stroke};

/// The selected feature's region choice, ready to draw and to click.
pub struct RegionPick {
    /// Feature that owns the choice.
    pub feature: usize,
    /// Name of its `Profiles` parameter.
    pub param: &'static str,
    pub plane: Plane,
    pub regions: Vec<Region>,
    pub spec: String,
}

impl RegionPick {
    /// Region under a viewport pixel.
    pub fn region_at(&self, proj: &Projector, px: f64, py: f64) -> Option<usize> {
        let p = proj.pixel_to_plane(px, py, &self.plane)?;
        region_select::region_at(&self.regions, p)
    }

    /// The parameter text after a click on region `i`.
    pub fn toggled(&self, i: usize) -> String {
        region_select::toggle(&self.regions, &self.spec, i)
    }

    pub fn used_count(&self) -> usize {
        (0..self.regions.len()).filter(|&i| region_select::is_selected(&self.regions, &self.spec, i)).count()
    }
}

/// The region choice of feature `sel`, if it has one and its sketch has
/// regions.
pub fn region_pick(doc: &Document, sel: Option<usize>) -> Option<RegionPick> {
    let feature = sel?;
    let params = doc.features.get(feature)?.feature.params();
    let (param, sketch_param, spec) = params.iter().find_map(|p| match (&p.kind, &p.value) {
        (ParamKind::Regions { sketch }, ParamValue::Expr(s)) => Some((p.name, *sketch, s.clone())),
        _ => None,
    })?;
    let sketch = params.iter().find_map(|p| match &p.value {
        ParamValue::FeatureRef(i) if p.name == sketch_param => Some(*i),
        _ => None,
    })?;
    let out = doc.features.get(sketch)?.output.as_ref()?;
    let plane = out.plane?;
    let regions = region_select::regions(&out.profiles);
    if regions.is_empty() {
        return None;
    }
    Some(RegionPick { feature, param, plane, regions, spec })
}

/// Sketch features to draw in the model view. A sketch that a later
/// feature uses is hidden unless `show_used` is on, it is the selected
/// feature, or the selected feature uses it.
pub fn visible_sketches(doc: &Document, sel: Option<usize>, show_used: bool) -> Vec<usize> {
    let is_sketch = |i: usize| doc.features.get(i).is_some_and(|n| n.feature.kind() == "sketch");
    let mut used = vec![false; doc.features.len()];
    let mut used_by_sel = vec![false; doc.features.len()];
    for (fi, n) in doc.features.iter().enumerate() {
        if n.suppressed {
            continue;
        }
        for p in n.feature.params() {
            if let ParamValue::FeatureRef(i) = p.value {
                if i < fi && is_sketch(i) {
                    used[i] = true;
                    if Some(fi) == sel {
                        used_by_sel[i] = true;
                    }
                }
            }
        }
    }
    (0..doc.features.len())
        .filter(|&i| is_sketch(i) && !doc.features[i].suppressed && doc.features[i].output.is_some())
        .filter(|&i| show_used || !used[i] || used_by_sel[i] || sel == Some(i))
        .collect()
}

/// Draw a sketch's closed profiles and open paths. Parts hidden behind a
/// body are drawn faint, so a sketch on a face stays readable.
pub fn draw_sketch(
    painter: &egui::Painter,
    origin: Pos2,
    proj: &Projector,
    fb: &Framebuffer,
    doc: &Document,
    idx: usize,
    highlight: bool,
) {
    let Some(out) = doc.features.get(idx).and_then(|n| n.output.as_ref()) else { return };
    let Some(plane) = out.plane else { return };
    let base = if highlight { Color32::from_rgb(230, 130, 20) } else { Color32::from_rgb(20, 60, 170) };
    let vis = Stroke::new(if highlight { 2.4f32 } else { 1.8f32 }, base);
    let hid = Stroke::new(1.0f32, base.gamma_multiply(0.35));
    let polyline = |pts: &[DVec2], closed: bool| {
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for k in 0..segs {
            let (a, b) = (plane.to_world(pts[k]), plane.to_world(pts[(k + 1) % n]));
            let (Some(pa), Some(pb)) = (proj.project(a), proj.project(b)) else { continue };
            // Split long segments so each piece gets its own depth test.
            let len_px = ((pb.0 - pa.0).powi(2) + (pb.1 - pa.1).powi(2)).sqrt();
            let pieces = ((len_px / 6.0).ceil() as usize).clamp(1, 400);
            for j in 0..pieces {
                let (t0, t1) = (j as f64 / pieces as f64, (j + 1) as f64 / pieces as f64);
                let lerp = |t: f64| (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t, pa.2 + (pb.2 - pa.2) * t);
                let (q0, q1, qm) = (lerp(t0), lerp(t1), lerp((t0 + t1) * 0.5));
                let stroke = if hidden(fb, qm) { hid } else { vis };
                painter.line_segment(
                    [
                        Pos2::new(origin.x + q0.0 as f32, origin.y + q0.1 as f32),
                        Pos2::new(origin.x + q1.0 as f32, origin.y + q1.1 as f32),
                    ],
                    stroke,
                );
            }
        }
    };
    for p in &out.profiles {
        polyline(&p.points, true);
    }
    for p in &out.paths {
        polyline(p, false);
    }
}

/// True when the depth buffer holds a surface clearly in front of the
/// projected point `(x, y, z)`. A sketch lying on a face is not hidden by it.
fn hidden(fb: &Framebuffer, (x, y, z): (f64, f64, f64)) -> bool {
    let Some(zb) = fb.depth_at_px(x, y) else { return false };
    zb.is_finite() && (z as f32) > zb * 1.01 + 0.05
}

/// Fill every region of the pick: used regions orange, unused ones pale
/// blue, the hovered one stronger. Each used region gets its outline.
pub fn draw_regions(
    painter: &egui::Painter,
    origin: Pos2,
    proj: &Projector,
    pick: &RegionPick,
    hovered: Option<usize>,
) {
    let to_screen = |p: DVec2| {
        proj.project(pick.plane.to_world(p)).map(|(x, y, _)| Pos2::new(origin.x + x as f32, origin.y + y as f32))
    };
    for (i, Region { outer, holes, .. }) in pick.regions.iter().enumerate() {
        let used = region_select::is_selected(&pick.regions, &pick.spec, i);
        let hot = hovered == Some(i);
        let fill = match (used, hot) {
            (true, true) => Color32::from_rgba_unmultiplied(240, 150, 30, 150),
            (true, false) => Color32::from_rgba_unmultiplied(240, 150, 30, 95),
            (false, true) => Color32::from_rgba_unmultiplied(90, 150, 230, 110),
            (false, false) => Color32::from_rgba_unmultiplied(90, 140, 220, 35),
        };
        let mut pts: Vec<DVec2> = outer.clone();
        let mut hole_idx = Vec::new();
        for h in holes {
            hole_idx.push((pts.len()..pts.len() + h.len()).collect::<Vec<_>>());
            pts.extend_from_slice(h);
        }
        let screen: Vec<Option<Pos2>> = pts.iter().map(|&p| to_screen(p)).collect();
        if screen.iter().any(|p| p.is_none()) {
            continue;
        }
        let tris = anvil_kernel::mesh::ear_clip_with_holes(&pts, outer.len(), &hole_idx);
        let mut mesh = egui::Mesh::default();
        for p in screen.iter().flatten() {
            mesh.colored_vertex(*p, fill);
        }
        for t in tris.as_chunks::<3>().0 {
            mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
        }
        painter.add(egui::Shape::mesh(mesh));
        if used || hot {
            let stroke = Stroke::new(
                2.0f32,
                if used { Color32::from_rgb(220, 120, 10) } else { Color32::from_rgb(40, 110, 210) },
            );
            for lp in std::iter::once(outer).chain(holes.iter()) {
                let s: Vec<Pos2> = lp.iter().filter_map(|&p| to_screen(p)).collect();
                painter.add(egui::Shape::closed_line(s, stroke));
            }
        }
    }
}
