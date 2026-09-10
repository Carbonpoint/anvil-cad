use crate::camera::{Camera, Projector};
use crate::panels::{self, PanelState};
use crate::raster::{Framebuffer, Style};
use crate::ribbon::{build_ribbon, ButtonKind, RibbonAction, RibbonTab};
use crate::scene::Scene;
use crate::sketch_editor::{ConstraintTool, DimensionTool, SketchEditor, Tool};
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::features::{extrude::ExtrudeFeature, revolve::RevolveFeature};
use anvil_feature::{descriptor, Document};
use anvil_math::{DVec2, DVec3, Plane};
use egui::{Color32, Pos2, Stroke};
use std::path::PathBuf;

/// What the viewport is doing.
enum Mode {
    /// Free orbit; click selects a body.
    Model,
    /// Waiting for the user to click a datum plane or a face to sketch on.
    PickPlane,
    /// Editing a sketch.
    Sketch(Box<SketchEditor>),
}

pub struct AnvilApp {
    doc: Document,
    ribbon: Vec<RibbonTab>,
    active_tab: usize,
    panels: PanelState,
    camera: Camera,
    scene: Scene,
    scene_dirty: bool,
    style: Style,
    fb: Framebuffer,
    texture: Option<egui::TextureHandle>,
    mode: Mode,
    hovered_body: Option<u32>,
    hovered_tri: Option<usize>,
    hovered_datum: Option<&'static str>,
    file_path: String,
    status: String,
    saved_camera: Option<Camera>,
}

const DATUMS: [(&str, Plane); 3] = [("XY", Plane::XY), ("XZ", Plane::XZ), ("YZ", Plane::YZ)];

impl AnvilApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let ribbon = build_ribbon();
        let active_tab = ribbon.iter().position(|t| t.name == "Solid").unwrap_or(0);
        let mut app = AnvilApp {
            doc: Document::new("Untitled"),
            ribbon,
            active_tab,
            panels: PanelState::default(),
            camera: Camera::default(),
            scene: Scene::default(),
            scene_dirty: true,
            style: Style::default(),
            fb: Framebuffer::new(8, 8),
            texture: None,
            mode: Mode::Model,
            hovered_body: None,
            hovered_tri: None,
            hovered_datum: None,
            file_path: "part.anvil".into(),
            status: "Ready".into(),
            saved_camera: None,
        };
        app.load_demo();
        app
    }

    fn load_demo(&mut self) {
        let mut doc = Document::new("Demo");
        doc.set_expression("w", "60").ok();
        doc.set_expression("h", "w / 2").ok();
        doc.set_expression("thk", "12").ok();
        doc.add_feature(Box::new(SketchFeature::rectangle("XY", 60.0, 30.0)));
        doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "thk".into(), symmetric: false }));
        let mut ring = SketchFeature::on_datum("XZ");
        ring.sketch.add_rectangle(41.0, 22.0, 49.0, 28.0);
        doc.add_feature(Box::new(ring));
        doc.add_feature(Box::new(RevolveFeature { sketch: 2, axis: "Y".into(), angle_deg: "270".into() }));
        self.doc = doc;
        self.panels = PanelState::default();
        self.mode = Mode::Model;
        self.camera = Camera::default();
        self.invalidate();
        self.refresh_scene();
        self.camera.fit(&self.scene.bounds);
        self.status = "Demo part loaded. Click Sketch, then a plane or a face, to draw.".into();
    }

    fn invalidate(&mut self) {
        self.scene_dirty = true;
    }

    fn refresh_scene(&mut self) {
        if self.scene_dirty {
            self.scene = Scene::build(&self.doc);
            self.scene_dirty = false;
        }
    }

    fn scene_size(&self) -> f64 {
        let d = self.scene.bounds.diagonal();
        if d > 1e-6 {
            d
        } else {
            100.0
        }
    }

    // ---------------- mode transitions ----------------

    fn begin_pick_plane(&mut self) {
        self.mode = Mode::PickPlane;
        self.status = "Click a datum plane (XY, XZ, YZ) or a face to sketch on. Esc cancels.".into();
    }

    fn start_sketch_on(&mut self, feature: Box<dyn Feature>) {
        let idx = self.doc.add_feature(feature);
        self.invalidate();
        self.open_sketch(idx);
    }

    fn open_sketch(&mut self, idx: usize) {
        self.refresh_scene();
        match SketchEditor::open(&self.doc, idx) {
            Some(mut ed) => {
                ed.set_tool(Tool::Line);
                let plane = ed.sketch.plane;
                self.saved_camera = Some(self.camera.clone());
                self.camera.look_at_plane(&plane, self.scene_size().max(60.0) * 1.3);
                self.panels.selected = Some(idx);
                self.mode = Mode::Sketch(Box::new(ed));
                self.status =
                    "Sketch mode. Line tool: click points, click an existing point to close. Finish Sketch when done."
                        .into();
            }
            None => self.status = "Select a sketch feature first".into(),
        }
    }

    fn finish_sketch(&mut self) {
        if let Mode::Sketch(ed) = &mut self.mode {
            ed.commit(&mut self.doc);
        }
        self.mode = Mode::Model;
        if let Some(c) = self.saved_camera.take() {
            self.camera = c;
        } else {
            self.camera.unlock();
        }
        self.invalidate();
        self.status = "Sketch finished".into();
    }

    // ---------------- actions ----------------

    fn run_action(&mut self, a: RibbonAction) {
        match a {
            RibbonAction::Undo => {
                if let Mode::Sketch(_) = self.mode {
                    self.finish_sketch();
                }
                self.doc.undo();
                self.invalidate();
            }
            RibbonAction::Redo => {
                if let Mode::Sketch(_) = self.mode {
                    self.finish_sketch();
                }
                self.doc.redo();
                self.invalidate();
            }
            RibbonAction::NewDocument => {
                self.doc = Document::new("Untitled");
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.status = "New document. Click Sketch to begin.".into();
            }
            RibbonAction::Save => {
                let p = PathBuf::from(&self.file_path);
                self.status = match anvil_io::save_document(&self.doc, &p) {
                    Ok(()) => format!("Saved {}", p.display()),
                    Err(e) => format!("Save failed: {e}"),
                };
            }
            RibbonAction::Load => {
                let p = PathBuf::from(&self.file_path);
                match anvil_io::load_document(&p) {
                    Ok(d) => {
                        self.doc = d;
                        self.panels = PanelState::default();
                        self.mode = Mode::Model;
                        self.camera.unlock();
                        self.invalidate();
                        self.refresh_scene();
                        self.camera.fit(&self.scene.bounds);
                        self.status = format!("Loaded {}", p.display());
                    }
                    Err(e) => self.status = format!("Load failed: {e}"),
                }
            }
            RibbonAction::ExportStl => {
                self.refresh_scene();
                let p = PathBuf::from(&self.file_path).with_extension("stl");
                self.status = match anvil_io::write_stl(&self.scene.mesh, &p) {
                    Ok(()) => format!("Wrote {} ({} triangles)", p.display(), self.scene.mesh.triangle_count()),
                    Err(e) => format!("STL export failed: {e}"),
                };
            }
            RibbonAction::ExportGcode => {
                let profile = self
                    .doc
                    .features
                    .iter()
                    .filter_map(|n| n.output.as_ref())
                    .flat_map(|o| o.profiles.iter())
                    .next()
                    .cloned();
                match profile {
                    Some(prof) => {
                        let tool = anvil_cam::Tool {
                            number: 1,
                            name: "6mm endmill".into(),
                            diameter: 6.0,
                            rpm: 12000.0,
                            feed: 800.0,
                            plunge: 200.0,
                        };
                        let params = anvil_cam::ops::ContourParams {
                            top_z: 0.0,
                            depth: 10.0,
                            step_down: 2.5,
                            clearance: 5.0,
                            outside: true,
                        };
                        let tp = anvil_cam::ops::contour(&prof.points, &tool, &params);
                        let text = anvil_cam::Post::emit(
                            &anvil_cam::GenericGcode { program_name: self.doc.name.clone() },
                            &tp,
                        );
                        let p = PathBuf::from(&self.file_path).with_extension("nc");
                        self.status = match std::fs::write(&p, text) {
                            Ok(()) => format!("Wrote {} ({:.0} mm of cutting)", p.display(), tp.cut_length()),
                            Err(e) => format!("G-code export failed: {e}"),
                        };
                    }
                    None => self.status = "No sketch profile to contour".into(),
                }
            }
            RibbonAction::FitView => {
                self.refresh_scene();
                self.camera.fit(&self.scene.bounds);
            }
            RibbonAction::DemoPart => self.load_demo(),
            RibbonAction::EditSketch => match self.panels.selected {
                Some(i) => self.open_sketch(i),
                None => self.status = "Select a sketch in the Part Navigator first".into(),
            },
            RibbonAction::Measure => {
                self.refresh_scene();
                match self.panels.selected.and_then(|i| self.doc.features[i].output.as_ref()) {
                    Some(out) if !out.bodies.is_empty() => {
                        let vol: f64 = out.bodies.iter().map(|b| b.volume()).sum();
                        let mut bb = anvil_math::Aabb::empty();
                        for b in &out.bodies {
                            let bounds = b.bounds();
                            bb.include(bounds.min);
                            bb.include(bounds.max);
                        }
                        let d = bb.max - bb.min;
                        self.status = format!(
                            "Volume {vol:.2} mm3, bounds {:.2} x {:.2} x {:.2} mm, {} bodies",
                            d.x,
                            d.y,
                            d.z,
                            out.bodies.len()
                        );
                    }
                    _ => self.status = "Select a feature that produces a body".into(),
                }
            }
            RibbonAction::ToggleEdges => self.style.draw_edges = !self.style.draw_edges,
            RibbonAction::ViewIso => {
                self.camera.unlock();
                self.camera.yaw = 0.8;
                self.camera.pitch = 0.5;
            }
            RibbonAction::ViewTop => {
                self.camera.unlock();
                self.camera.yaw = -std::f64::consts::FRAC_PI_2;
                self.camera.pitch = 1.49;
            }
            RibbonAction::ViewFront => {
                self.camera.unlock();
                self.camera.yaw = -std::f64::consts::FRAC_PI_2;
                self.camera.pitch = 0.0;
            }
            RibbonAction::ViewRight => {
                self.camera.unlock();
                self.camera.yaw = 0.0;
                self.camera.pitch = 0.0;
            }
        }
    }

    fn add_feature_by_id(&mut self, id: &str) {
        if id == "sketch" {
            self.begin_pick_plane();
            return;
        }
        if let Some(d) = descriptor(id) {
            let mut f = (d.create)();
            // Sensible defaults: point body refs at the selected feature and
            // sketch refs at the latest sketch.
            if let Some(sel) = self.panels.selected {
                let sel_type = self.doc.features[sel].feature.kind();
                for p in f.params() {
                    if let anvil_feature::param::ParamKind::FeatureRef { accepts } = &p.kind {
                        if accepts.contains(&sel_type) {
                            let _ = f.set_param(p.name, anvil_feature::ParamValue::FeatureRef(sel));
                        }
                    }
                }
            }
            let idx = self.doc.add_feature(f);
            self.panels.selected = Some(idx);
            self.invalidate();
            self.status = match &self.doc.features[idx].error {
                Some(e) => format!("Added {}: {e}", d.label),
                None => format!("Added {}. Edit its parameters in the Properties panel.", d.label),
            };
        }
    }

    // ---------------- ribbon ----------------

    fn ribbon_ui(&mut self, ui: &mut egui::Ui) {
        if let Mode::Sketch(_) = self.mode {
            self.sketch_ribbon(ui);
            return;
        }
        let mut clicked_feature: Option<&'static str> = None;
        let mut clicked_action: Option<RibbonAction> = None;
        ui.horizontal(|ui| {
            if ui.add_enabled(self.doc.can_undo(), egui::Button::new("Undo")).clicked() {
                clicked_action = Some(RibbonAction::Undo);
            }
            if ui.add_enabled(self.doc.can_redo(), egui::Button::new("Redo")).clicked() {
                clicked_action = Some(RibbonAction::Redo);
            }
            ui.separator();
            for (i, t) in self.ribbon.iter().enumerate() {
                if ui.selectable_label(self.active_tab == i, t.name).clicked() {
                    self.active_tab = i;
                }
            }
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if let Some(tab) = self.ribbon.get(self.active_tab) {
                for g in &tab.groups {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for b in &g.buttons {
                                let btn = ui.add(egui::Button::new(b.label).min_size(egui::vec2(60.0, 36.0)));
                                if btn.on_hover_text(b.tooltip).clicked() {
                                    match b.kind {
                                        ButtonKind::Feature(id) => clicked_feature = Some(id),
                                        ButtonKind::Action(a) => clicked_action = Some(a),
                                    }
                                }
                            }
                        });
                        ui.label(egui::RichText::new(g.name).small().weak());
                    });
                    ui.separator();
                }
            }
        });
        if let Some(id) = clicked_feature {
            self.add_feature_by_id(id);
        }
        if let Some(a) = clicked_action {
            self.run_action(a);
        }
    }

    fn sketch_ribbon(&mut self, ui: &mut egui::Ui) {
        let mut finish = false;
        let mut new_tool: Option<Tool> = None;
        let mut constraint: Option<ConstraintTool> = None;
        let mut dimension: Option<DimensionTool> = None;
        let mut delete = false;
        let mut construction = false;
        let mut clear_constraints = false;
        let mut undo = false;
        let mut offset = false;
        let mut project = false;
        let mut look_at = false;
        let Mode::Sketch(ed) = &mut self.mode else { return };
        ui.horizontal(|ui| {
            if ui.button("Undo").clicked() {
                undo = true;
            }
            ui.separator();
            ui.label(egui::RichText::new("SKETCH").strong());
            ui.separator();
            if ui.add(egui::Button::new("Finish Sketch").fill(Color32::from_rgb(70, 140, 90))).clicked() {
                finish = true;
            }
            ui.separator();
            ui.label(egui::RichText::new(format!("{}: {}", ed.tool.label(), ed.tool.hint())).weak());
            if !ed.message.is_empty() {
                ui.separator();
                ui.label(&ed.message);
            }
        });
        ui.separator();
        let tool_button = |ui: &mut egui::Ui, t: Tool, current: Tool, out: &mut Option<Tool>| {
            if ui.add(egui::Button::new(t.label()).selected(current == t)).on_hover_text(t.hint()).clicked() {
                *out = Some(t);
                ui.close();
            }
        };
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new("Select")
                                .selected(ed.tool == Tool::Select)
                                .min_size(egui::vec2(52.0, 34.0)),
                        )
                        .clicked()
                    {
                        new_tool = Some(Tool::Select);
                    }
                    for (menu, tools) in Tool::MENUS {
                        let active = tools.contains(&ed.tool);
                        let title = if active { ed.tool.label() } else { menu };
                        let mut resp = None;
                        ui.scope(|ui| {
                            if active {
                                ui.visuals_mut().widgets.inactive.weak_bg_fill = ui.visuals().selection.bg_fill;
                            }
                            resp = Some(ui.menu_button(format!("{title} v"), |ui| {
                                for &t in tools {
                                    tool_button(ui, t, ed.tool, &mut new_tool);
                                }
                            }));
                        });
                    }
                    ui.label("sides");
                    ui.add(egui::DragValue::new(&mut ed.polygon_sides).range(3..=32));
                    ui.label("slot w");
                    ui.add(egui::DragValue::new(&mut ed.slot_width).range(0.1..=1000.0).speed(0.5));
                });
                ui.label(egui::RichText::new("Create").small().weak());
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    for t in Tool::MODIFY {
                        if ui.add(egui::Button::new(t.label()).selected(ed.tool == t)).on_hover_text(t.hint()).clicked()
                        {
                            new_tool = Some(t);
                        }
                    }
                    if ui.button("Offset").on_hover_text("Offset the selection by the value field").clicked() {
                        offset = true;
                    }
                    if ui
                        .button("Project")
                        .on_hover_text("Project body edges lying in this plane into the sketch")
                        .clicked()
                    {
                        project = true;
                    }
                    if ui.button("Delete").on_hover_text("Delete selected entities (Del)").clicked() {
                        delete = true;
                    }
                    if ui.button("Construction").on_hover_text("Toggle construction on selected lines").clicked() {
                        construction = true;
                    }
                    ui.checkbox(&mut ed.copy, "copy");
                    ui.label("n1");
                    ui.add(egui::DragValue::new(&mut ed.count1).range(1..=200));
                    ui.label("n2");
                    ui.add(egui::DragValue::new(&mut ed.count2).range(1..=200));
                    ui.label("angle");
                    ui.add(egui::DragValue::new(&mut ed.pattern_angle).range(-360.0..=360.0));
                });
                ui.label(egui::RichText::new("Modify").small().weak());
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    for c in ConstraintTool::ALL {
                        if ui.add(egui::Button::new(c.label())).clicked() {
                            constraint = Some(c);
                        }
                    }
                    if ui.button("Clear").on_hover_text("Remove constraints on the selection").clicked() {
                        clear_constraints = true;
                    }
                });
                ui.label(egui::RichText::new("Constraints (select first)").small().weak());
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut ed.dim_value).desired_width(60.0).hint_text("value"));
                    if ui
                        .button("Dimension")
                        .on_hover_text("Length, distance, radius, or angle from the selection (D)")
                        .clicked()
                    {
                        dimension = Some(DimensionTool::Smart);
                    }
                    if ui.button("Length").clicked() {
                        dimension = Some(DimensionTool::Length);
                    }
                    if ui.button("Radius").clicked() {
                        dimension = Some(DimensionTool::Radius);
                    }
                    if ui.button("Angle").clicked() {
                        dimension = Some(DimensionTool::Angle);
                    }
                });
                ui.label(egui::RichText::new("Dimension (value also sets fillet, scale, offset)").small().weak());
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("Look At").clicked() {
                        look_at = true;
                    }
                    ui.checkbox(&mut ed.show_grid, "Grid");
                    ui.checkbox(&mut ed.snap_grid, "Snap grid");
                    ui.checkbox(&mut ed.snap_curves, "Snap curves");
                    ui.add(egui::DragValue::new(&mut ed.grid).range(0.0..=1000.0).speed(0.5).prefix("grid "));
                    ui.checkbox(&mut ed.show_points, "Points");
                    ui.checkbox(&mut ed.show_constraints, "Constraints");
                });
                ui.label(egui::RichText::new("Palette").small().weak());
            });
        });
        if let Some(t) = new_tool {
            ed.set_tool(t);
        }
        if let Some(c) = constraint {
            ed.apply_constraint(c, &mut self.doc);
            self.scene_dirty = true;
        }
        if let Some(d) = dimension {
            ed.apply_dimension(d, &mut self.doc);
            self.scene_dirty = true;
        }
        if delete {
            ed.delete_selection(&mut self.doc);
            self.scene_dirty = true;
        }
        if construction {
            ed.toggle_construction(&mut self.doc);
            self.scene_dirty = true;
        }
        if clear_constraints {
            ed.remove_constraints_on_selection(&mut self.doc);
            self.scene_dirty = true;
        }
        if offset {
            ed.offset_selection(&mut self.doc);
            self.scene_dirty = true;
        }
        if project {
            let segs: Vec<[DVec3; 2]> = self.scene.edges.iter().map(|(_, e)| *e).collect();
            ed.project(&segs, &mut self.doc);
            self.scene_dirty = true;
        }
        if look_at {
            let plane = ed.sketch.plane;
            self.camera.look_at_plane(&plane, self.camera.ortho_height.unwrap_or(100.0));
        }
        if undo {
            let idx = ed.feature;
            self.doc.undo();
            let tool = ed.tool;
            if let Some(mut ned) = SketchEditor::open(&self.doc, idx) {
                ned.set_tool(tool);
                self.mode = Mode::Sketch(Box::new(ned));
            }
            self.scene_dirty = true;
            return;
        }
        if finish {
            self.finish_sketch();
        }
    }

    // ---------------- viewport ----------------

    fn viewport(&mut self, ui: &mut egui::Ui) {
        self.refresh_scene();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        let w = (rect.width().max(8.0)) as usize;
        let h = (rect.height().max(8.0)) as usize;
        if self.fb.width != w || self.fb.height != h {
            self.fb = Framebuffer::new(w, h);
        }
        let proj = Projector::new(&self.camera, w as f64, h as f64);
        let pointer = resp.hover_pos().map(|p| ((p.x - rect.left()) as f64, (p.y - rect.top()) as f64));
        let shift = ui.input(|i| i.modifiers.shift);
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let del = ui.input(|i| i.key_pressed(egui::Key::Delete));
        let secondary_clicked = resp.secondary_clicked();

        // Camera navigation (all modes). Left drag orbits only in Model mode
        // with nothing under the cursor at drag start; otherwise it belongs
        // to the tool.
        let in_sketch = matches!(self.mode, Mode::Sketch(_));
        if resp.dragged_by(egui::PointerButton::Middle)
            || (resp.dragged_by(egui::PointerButton::Secondary) && !in_sketch)
        {
            let d = resp.drag_delta();
            self.camera.pan(d.x as f64, d.y as f64, h as f64);
        }
        if resp.dragged_by(egui::PointerButton::Secondary) && in_sketch {
            let d = resp.drag_delta();
            self.camera.pan(d.x as f64, d.y as f64, h as f64);
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                self.camera.zoom((-scroll as f64 * 0.002).exp());
            }
        }

        // Hover picking from the previous frame's id buffer.
        self.hovered_tri = pointer.and_then(|(x, y)| self.fb.tri_at(x as usize, y as usize));
        self.hovered_body = self.hovered_tri.and_then(|t| self.scene.tri_body.get(t).copied());
        self.hovered_datum = None;

        let selected_body = self
            .panels
            .selected
            .and_then(|sel| self.scene.bodies.iter().position(|(fi, _)| *fi == sel))
            .map(|i| i as u32);

        match &mut self.mode {
            Mode::Model => {
                if resp.dragged_by(egui::PointerButton::Primary) {
                    let d = resp.drag_delta();
                    self.camera.orbit(d.x as f64, d.y as f64);
                }
                if resp.clicked() {
                    match self.hovered_body {
                        Some(b) => {
                            let (fi, _) = self.scene.bodies[b as usize];
                            self.panels.selected = Some(fi);
                        }
                        None => self.panels.selected = None,
                    }
                }
            }
            Mode::PickPlane => {
                if esc {
                    self.mode = Mode::Model;
                    self.status = "Cancelled".into();
                } else {
                    // Datum squares take priority over faces when hovered.
                    let size = self.scene_size() * 0.6;
                    if let Some((x, y)) = pointer {
                        for (name, plane) in DATUMS {
                            if let Some(p) = proj.pixel_to_plane(x, y, &plane) {
                                if p.x.abs() <= size && p.y.abs() <= size {
                                    self.hovered_datum = Some(name);
                                    break;
                                }
                            }
                        }
                    }
                    if resp.dragged_by(egui::PointerButton::Primary) {
                        let d = resp.drag_delta();
                        self.camera.orbit(d.x as f64, d.y as f64);
                    }
                    if resp.clicked() {
                        if let Some(name) = self.hovered_datum {
                            self.start_sketch_on(Box::new(SketchFeature::on_datum(name)));
                        } else if let Some(t) = self.hovered_tri {
                            if let Some(plane) = self.scene.face_plane(&self.doc, t) {
                                let segs: Vec<[DVec3; 2]> = self.scene.edges.iter().map(|(_, e)| *e).collect();
                                let mut sk = SketchFeature::on_plane(plane);
                                sk.sketch.project_segments(&segs, 1e-4);
                                self.start_sketch_on(Box::new(sk));
                            }
                        }
                    }
                }
            }
            Mode::Sketch(ed) => {
                let plane = ed.sketch.plane;
                let sp = pointer.and_then(|(x, y)| proj.pixel_to_plane(x, y, &plane));
                ed.on_hover(sp, &proj);
                if esc || secondary_clicked {
                    if ed.clicks.is_empty() && ed.tool != Tool::Select {
                        ed.set_tool(Tool::Select);
                    } else if ed.cancel(&mut self.doc) {
                        self.scene_dirty = true;
                    }
                }
                if del {
                    ed.delete_selection(&mut self.doc);
                    self.scene_dirty = true;
                }
                let key = |k: egui::Key| ui.input(|i| i.key_pressed(k) && !i.modifiers.any());
                if key(egui::Key::L) {
                    ed.set_tool(Tool::Line);
                }
                if key(egui::Key::R) {
                    ed.set_tool(Tool::Rect2);
                }
                if key(egui::Key::C) {
                    ed.set_tool(Tool::CircleCenter);
                }
                if key(egui::Key::D) {
                    ed.apply_dimension(DimensionTool::Smart, &mut self.doc);
                    self.scene_dirty = true;
                }
                if resp.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(p) = sp {
                        ed.on_drag_start(p, &proj);
                    }
                }
                if resp.dragged_by(egui::PointerButton::Primary) {
                    if let Some(p) = sp {
                        ed.on_drag(p);
                    }
                }
                if resp.drag_stopped_by(egui::PointerButton::Primary) {
                    ed.on_drag_end(&mut self.doc);
                    self.scene_dirty = true;
                }
                if resp.clicked() {
                    if let Some(p) = sp {
                        ed.on_click(p, shift, &proj, &mut self.doc);
                        self.scene_dirty = true;
                    }
                }
            }
        }

        // Render.
        self.fb.clear(self.style.background);
        let hovered = if matches!(self.mode, Mode::Model | Mode::PickPlane) { self.hovered_body } else { None };
        self.fb.draw_scene(&self.scene, &proj, &self.style, selected_body, hovered);
        let image = self.fb.to_image();
        let tex = match &mut self.texture {
            Some(t) => {
                t.set(image, egui::TextureOptions::NEAREST);
                t.id()
            }
            None => {
                let t = ui.ctx().load_texture("viewport", image, egui::TextureOptions::NEAREST);
                let id = t.id();
                self.texture = Some(t);
                id
            }
        };
        painter.image(tex, rect, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

        // Overlays.
        let origin = rect.min;
        let to_screen = |p: DVec3| proj.project(p).map(|(x, y, _)| Pos2::new(origin.x + x as f32, origin.y + y as f32));
        if let Mode::PickPlane = self.mode {
            let size = self.scene_size() * 0.6;
            for (name, plane) in DATUMS {
                let corners = [
                    DVec2::new(-size, -size),
                    DVec2::new(size, -size),
                    DVec2::new(size, size),
                    DVec2::new(-size, size),
                ];
                let pts: Vec<Pos2> = corners.iter().filter_map(|&c| to_screen(plane.to_world(c))).collect();
                if pts.len() == 4 {
                    let hot = self.hovered_datum == Some(name);
                    let fill = if hot {
                        Color32::from_rgba_unmultiplied(255, 190, 60, 90)
                    } else {
                        Color32::from_rgba_unmultiplied(90, 140, 220, 40)
                    };
                    let stroke = Stroke::new(if hot { 2.0f32 } else { 1.0f32 }, Color32::from_rgb(60, 100, 180));
                    painter.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
                    painter.text(
                        pts[2],
                        egui::Align2::RIGHT_BOTTOM,
                        name,
                        egui::FontId::proportional(13.0),
                        Color32::from_rgb(40, 70, 140),
                    );
                }
            }
            if self.hovered_datum.is_none() {
                if let Some(t) = self.hovered_tri {
                    if let Some(plane) = self.scene.face_plane(&self.doc, t) {
                        let n = plane.normal();
                        if let (Some(a), Some(b)) =
                            (to_screen(plane.origin), to_screen(plane.origin + n * self.scene_size() * 0.15))
                        {
                            painter.arrow(a, b - a, Stroke::new(2.0f32, Color32::from_rgb(240, 150, 30)));
                        }
                    }
                }
            }
        }
        if let Mode::Sketch(ed) = &self.mode {
            ed.draw(&painter, origin, &proj);
            if let Some(r) = &ed.report {
                let txt = match r.status {
                    anvil_sketch::SolveStatus::Converged if r.dof == 0 => "Fully constrained".to_string(),
                    anvil_sketch::SolveStatus::Converged => format!("{} degrees of freedom", r.dof),
                    anvil_sketch::SolveStatus::Trivial => format!("{} degrees of freedom", r.dof.max(0)),
                    anvil_sketch::SolveStatus::NotConverged => "Over-constrained or conflicting".to_string(),
                };
                let col = if r.status == anvil_sketch::SolveStatus::NotConverged {
                    Color32::from_rgb(200, 50, 40)
                } else {
                    Color32::from_rgb(40, 90, 50)
                };
                painter.text(
                    rect.left_top() + egui::vec2(10.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    txt,
                    egui::FontId::proportional(13.0),
                    col,
                );
                if let Some(c) = ed.cursor {
                    painter.text(
                        rect.left_bottom() + egui::vec2(10.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("x {:.2}  y {:.2}", c.x, c.y),
                        egui::FontId::monospace(12.0),
                        Color32::from_rgb(60, 60, 60),
                    );
                }
            }
        }
        self.draw_triad(&painter, rect);
    }

    fn draw_triad(&self, painter: &egui::Painter, rect: egui::Rect) {
        let (r, u, _) = self.camera.basis();
        let origin = Pos2::new(rect.right() - 45.0, rect.bottom() - 45.0);
        for (a, c, label) in [
            (DVec3::X, Color32::from_rgb(200, 50, 50), "X"),
            (DVec3::Y, Color32::from_rgb(40, 150, 60), "Y"),
            (DVec3::Z, Color32::from_rgb(60, 100, 230), "Z"),
        ] {
            let sx = a.dot(r) as f32;
            let sy = a.dot(u) as f32;
            let end = Pos2::new(origin.x + sx * 28.0, origin.y - sy * 28.0);
            painter.line_segment([origin, end], Stroke::new(2.0f32, c));
            painter.text(end, egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(11.0), c);
        }
    }
}

use anvil_feature::Feature;

impl eframe::App for AnvilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (undo, redo) = ctx.input(|i| {
            (i.modifiers.command && i.key_pressed(egui::Key::Z), i.modifiers.command && i.key_pressed(egui::Key::Y))
        });
        if undo {
            self.run_action(RibbonAction::Undo);
        }
        if redo {
            self.run_action(RibbonAction::Redo);
        }
        if let Some(i) = self.panels.open_requested.take() {
            if self.doc.features.get(i).map(|f| f.feature.kind() == "sketch").unwrap_or(false) {
                self.open_sketch(i);
            }
        }

        egui::TopBottomPanel::top("ribbon").show(ctx, |ui| self.ribbon_ui(ui));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.separator();
                ui.label("File:");
                ui.add(egui::TextEdit::singleline(&mut self.file_path).desired_width(200.0));
                ui.separator();
                ui.label(format!("{} tris", self.scene.mesh.triangle_count()));
            });
        });

        egui::TopBottomPanel::bottom("expressions").resizable(true).show(ctx, |ui| {
            if panels::expression_panel(ui, &mut self.doc, &mut self.panels) {
                self.invalidate();
            }
        });

        egui::SidePanel::left("navigator").default_width(240.0).show(ctx, |ui| {
            if panels::part_navigator(ui, &mut self.doc, &mut self.panels) {
                self.invalidate();
            }
        });

        egui::SidePanel::right("properties").default_width(270.0).show(ctx, |ui| {
            if panels::property_panel(ui, &mut self.doc, &mut self.panels) {
                self.invalidate();
            }
            if let Mode::Sketch(ed) = &self.mode {
                ui.separator();
                ui.heading("Sketch");
                ui.label(format!("{} entities, {} constraints", ed.sketch.entities.len(), ed.sketch.constraints.len()));
                ui.label(format!("Selected: {}", ed.selection.len()));
                ui.label("Left drag on a point moves it. Right drag pans. Scroll zooms.");
            }
        });

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| self.viewport(ui));
    }
}
