//! The CAM window: make a toolpath from a sketch and save it as G-code
//! for one of the posts.
//!
//! Contour and pocket use the sketch's regions (outer loops with their
//! islands); drilling uses the centres of the sketch's circles. Sketch
//! coordinates are the machine's X and Y, and the top of the stock is the
//! sketch plane's height, so a sketch on XY or on a flat top face reads as
//! the part is set up on the machine.

use anvil_cam::ops::{ContourParams, DrillParams, PocketParams};
use anvil_cam::{Tool, Toolpath};
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::Document;
use anvil_math::DVec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ContourOutside,
    ContourInside,
    Pocket,
    Drill,
}

impl Operation {
    pub const ALL: [Operation; 4] =
        [Operation::ContourOutside, Operation::ContourInside, Operation::Pocket, Operation::Drill];
    pub fn label(self) -> &'static str {
        match self {
            Operation::ContourOutside => "Contour outside",
            Operation::ContourInside => "Contour inside",
            Operation::Pocket => "Pocket",
            Operation::Drill => "Drill circle centres",
        }
    }
}

/// Everything the CAM window asks for.
#[derive(Clone, Debug)]
pub struct CamState {
    pub sketch: Option<usize>,
    pub operation: Operation,
    pub tool: Tool,
    pub depth: f64,
    pub step_down: f64,
    /// Fraction of the tool diameter.
    pub stepover: f64,
    pub peck: f64,
    pub clearance: f64,
    /// Index into `anvil_cam::posts()`.
    pub post: usize,
    pub message: String,
}

impl Default for CamState {
    fn default() -> Self {
        CamState {
            sketch: None,
            operation: Operation::ContourOutside,
            tool: Tool {
                number: 1,
                name: "6mm endmill".into(),
                diameter: 6.0,
                rpm: 12000.0,
                feed: 800.0,
                plunge: 200.0,
            },
            depth: 5.0,
            step_down: 1.5,
            stepover: 0.4,
            peck: 2.0,
            clearance: 5.0,
            post: 0,
            message: String::new(),
        }
    }
}

/// The toolpath for the settings, from the sketch feature `idx`.
pub fn toolpath(doc: &Document, idx: usize, s: &CamState) -> Result<Toolpath, String> {
    let node = doc.features.get(idx).ok_or("no such sketch")?;
    let out = node.output.as_ref().ok_or("the sketch has not computed")?;
    let plane = out.plane.ok_or("that feature is not a sketch")?;
    if plane.normal().cross(anvil_math::DVec3::Z).length() > 1e-6 {
        return Err("the sketch must lie flat (parallel to XY) for 2.5D machining".into());
    }
    let top_z = plane.origin.z;
    // Sketch coordinates to machine X and Y.
    let to_xy = |p: DVec2| {
        let w = plane.to_world(p);
        DVec2::new(w.x, w.y)
    };
    let mut tp = Toolpath::default();
    match s.operation {
        Operation::Drill => {
            let sk = node.feature.downcast_ref::<SketchFeature>().ok_or("that feature is not a sketch")?;
            let centres: Vec<DVec2> = sk
                .sketch
                .entities
                .values()
                .filter_map(|e| match e {
                    anvil_sketch::Entity::Circle { center, .. } => match sk.sketch.entities.get(*center) {
                        Some(anvil_sketch::Entity::Point { pos, .. }) => Some(to_xy(*pos)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            if centres.is_empty() {
                return Err("the sketch has no circles to drill".into());
            }
            let p = DrillParams { top_z, depth: s.depth, peck: s.peck, retract: 2.0, clearance: s.clearance };
            tp = anvil_cam::ops::drill(&centres, &s.tool, &p);
        }
        op => {
            let regions = anvil_feature::region_select::select(&out.profiles, "").map_err(|e| e.to_string())?;
            if regions.is_empty() {
                return Err("the sketch has no closed profile".into());
            }
            for (outer, holes) in regions {
                let outer: Vec<DVec2> = outer.into_iter().map(to_xy).collect();
                let holes: Vec<Vec<DVec2>> = holes.into_iter().map(|h| h.into_iter().map(to_xy).collect()).collect();
                let part = match op {
                    Operation::Pocket => anvil_cam::ops::pocket(
                        &outer,
                        &holes,
                        &s.tool,
                        &PocketParams {
                            top_z,
                            depth: s.depth,
                            step_down: s.step_down,
                            stepover: s.stepover,
                            clearance: s.clearance,
                        },
                    ),
                    _ => anvil_cam::ops::contour(
                        &outer,
                        &s.tool,
                        &ContourParams {
                            top_z,
                            depth: s.depth,
                            step_down: s.step_down,
                            clearance: s.clearance,
                            outside: op == Operation::ContourOutside,
                        },
                    ),
                };
                tp.moves.extend(part.moves);
            }
        }
    }
    Ok(tp)
}

/// Draw the window. Returns a status line when a file was written.
pub fn window(
    ctx: &egui::Context,
    doc: &Document,
    selected: Option<usize>,
    open: &mut bool,
    s: &mut CamState,
) -> Option<String> {
    let mut status = None;
    let sketches: Vec<usize> =
        (0..doc.features.len()).filter(|&i| doc.features[i].feature.kind() == "sketch").collect();
    if s.sketch.is_none_or(|i| !sketches.contains(&i)) {
        s.sketch = selected.filter(|i| sketches.contains(i)).or_else(|| sketches.last().copied());
    }
    let posts = anvil_cam::posts();
    egui::Window::new("CAM").collapsible(false).resizable(false).open(open).show(ctx, |ui| {
        egui::Grid::new("cam_grid").num_columns(2).show(ui, |ui| {
            ui.label("Sketch");
            let current =
                s.sketch.map(|i| format!("{i}: {}", doc.features[i].feature.name())).unwrap_or("(none)".into());
            egui::ComboBox::from_id_salt("cam_sketch").selected_text(current).show_ui(ui, |ui| {
                for &i in &sketches {
                    ui.selectable_value(&mut s.sketch, Some(i), format!("{i}: {}", doc.features[i].feature.name()));
                }
            });
            ui.end_row();
            ui.label("Operation");
            egui::ComboBox::from_id_salt("cam_op").selected_text(s.operation.label()).show_ui(ui, |ui| {
                for op in Operation::ALL {
                    ui.selectable_value(&mut s.operation, op, op.label());
                }
            });
            ui.end_row();
            let num = |ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>| {
                ui.label(label);
                ui.add(egui::DragValue::new(v).range(range).speed(0.1));
                ui.end_row();
            };
            num(ui, "Tool diameter mm", &mut s.tool.diameter, 0.1..=100.0);
            num(ui, "Spindle rpm", &mut s.tool.rpm, 100.0..=60000.0);
            num(ui, "Feed mm/min", &mut s.tool.feed, 1.0..=20000.0);
            num(ui, "Plunge mm/min", &mut s.tool.plunge, 1.0..=10000.0);
            num(ui, "Depth mm", &mut s.depth, 0.01..=500.0);
            match s.operation {
                Operation::Drill => num(ui, "Peck mm (0 = none)", &mut s.peck, 0.0..=100.0),
                _ => num(ui, "Step down mm", &mut s.step_down, 0.01..=100.0),
            }
            if s.operation == Operation::Pocket {
                num(ui, "Stepover (of diameter)", &mut s.stepover, 0.05..=0.95);
            }
            num(ui, "Clearance mm", &mut s.clearance, 0.5..=200.0);
            ui.label("Post");
            egui::ComboBox::from_id_salt("cam_post").selected_text(posts[s.post.min(posts.len() - 1)].name()).show_ui(
                ui,
                |ui| {
                    for (k, p) in posts.iter().enumerate() {
                        ui.selectable_value(&mut s.post, k, p.name());
                    }
                },
            );
            ui.end_row();
        });
        if ui.button("Save G-code...").clicked() {
            match s.sketch.ok_or_else(|| "add a sketch first".to_string()).and_then(|i| toolpath(doc, i, s)) {
                Err(e) => s.message = e,
                Ok(tp) => {
                    let name = format!("{}.nc", doc.name.replace([' ', '/', '\\'], "_"));
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("G-code", &["nc", "ngc", "gcode", "tap"])
                        .set_file_name(name)
                        .save_file()
                    {
                        let text = posts[s.post.min(posts.len() - 1)].emit(&tp);
                        s.message = match std::fs::write(&path, text) {
                            Ok(()) => {
                                let m = format!("Wrote {} ({:.0} mm of cutting)", path.display(), tp.cut_length());
                                status = Some(m.clone());
                                m
                            }
                            Err(e) => format!("G-code: {e}"),
                        };
                    }
                }
            }
        }
        if !s.message.is_empty() {
            ui.label(egui::RichText::new(&s.message).small());
        }
    });
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sketch_with_a_hole_pockets_around_it_and_drills_it() {
        let mut doc = Document::new("cam");
        let mut sk = SketchFeature::on_datum("XY");
        sk.sketch.add_rectangle(0.0, 0.0, 40.0, 30.0);
        let c = sk.sketch.add_point(20.0, 15.0);
        sk.sketch.add_circle(c, 5.0);
        doc.add_feature(Box::new(sk));
        let mut s = CamState { operation: Operation::Pocket, ..Default::default() };
        let tp = toolpath(&doc, 0, &s).unwrap();
        assert!(tp.cut_length() > 100.0);
        s.operation = Operation::Drill;
        let tp = toolpath(&doc, 0, &s).unwrap();
        let holes = tp.moves.iter().filter(|m| matches!(m, anvil_cam::Move::Drill { .. })).count();
        assert_eq!(holes, 1);
    }
}
