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
    com_marker: Option<DVec3>,
    color_edit: [u8; 3],
    /// Extra selected features (Ctrl+click). `panels.selected` is the primary.
    multi: Vec<usize>,
    /// Selected face: (body index in scene, face id, triangle used to pick it).
    selected_face: Option<(u32, anvil_kernel::FaceId, usize)>,
    hovered_face: Option<(u32, anvil_kernel::FaceId)>,
    /// Model-mode box select in screen pixels.
    box_select: Option<(Pos2, Pos2)>,
    /// Edge under the cursor: (body index in scene, end points).
    hovered_edge: Option<(u32, [DVec3; 2])>,
    /// Picked edges for Fillet and Chamfer.
    selected_edges: Vec<(u32, [DVec3; 2])>,
    filter: SelectFilter,
    section_on: bool,
    section_axis: usize,
    section_offset: f64,
    section_flip: bool,
}

/// What a click in the model viewport may select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectFilter {
    All,
    Body,
    Face,
    Edge,
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
            com_marker: None,
            color_edit: [140, 170, 205],
            multi: Vec::new(),
            selected_face: None,
            hovered_face: None,
            box_select: None,
            hovered_edge: None,
            selected_edges: Vec::new(),
            filter: SelectFilter::All,
            section_on: false,
            section_axis: 0,
            section_offset: 0.0,
            section_flip: false,
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
        doc.add_feature(Box::new(ExtrudeFeature {
            sketch: 0,
            distance: "thk".into(),
            symmetric: false,
            ..Default::default()
        }));
        let mut ring = SketchFeature::on_datum("XZ");
        ring.sketch.add_rectangle(41.0, 22.0, 49.0, 28.0);
        doc.add_feature(Box::new(ring));
        doc.add_feature(Box::new(RevolveFeature {
            sketch: 2,
            axis: "Y".into(),
            angle_deg: "270".into(),
            ..Default::default()
        }));
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
            RibbonAction::DeleteFeature => {
                let mut all: Vec<usize> = self.multi.clone();
                if let Some(i) = self.panels.selected {
                    all.push(i);
                }
                all.sort_unstable();
                all.dedup();
                if all.is_empty() {
                    self.status = "Select a feature to delete".into();
                } else {
                    let mut broken = 0;
                    for &i in all.iter().rev() {
                        broken += self.doc.remove_feature(i).len();
                    }
                    self.panels.selected = None;
                    self.multi.clear();
                    self.selected_face = None;
                    self.invalidate();
                    self.status = if broken > 0 {
                        format!(
                            "Deleted {} feature(s). {broken} later feature(s) lost an input; see the red marks.",
                            all.len()
                        )
                    } else {
                        format!("Deleted {} feature(s)", all.len())
                    };
                }
            }
            RibbonAction::ComputeAll => {
                self.doc.regenerate();
                self.invalidate();
                let errors = self.doc.features.iter().filter(|f| f.error.is_some()).count();
                self.status = format!("Regenerated {} features, {errors} with errors", self.doc.features.len());
            }
            RibbonAction::CenterOfMass => match self.panels.selected.and_then(|i| self.doc.features[i].output.as_ref())
            {
                Some(out) if !out.bodies.is_empty() => {
                    let mut total = 0.0;
                    let mut c = DVec3::ZERO;
                    for b in &out.bodies {
                        let v = b.volume();
                        c += b.centroid() * v;
                        total += v;
                    }
                    let c = if total > 0.0 { c / total } else { DVec3::ZERO };
                    self.com_marker = Some(c);
                    self.status = format!(
                        "Centre of mass: {}, {}, {}",
                        self.doc.fmt_length(c.x),
                        self.doc.fmt_length(c.y),
                        self.doc.fmt_length(c.z)
                    );
                }
                _ => self.status = "Select a feature that produces a body".into(),
            },
            RibbonAction::BillOfMaterials => {
                let mut lines = Vec::new();
                let mut total_mass = 0.0;
                for (i, n) in self.doc.features.iter().enumerate() {
                    let Some(out) = &n.output else { continue };
                    if out.bodies.is_empty() {
                        continue;
                    }
                    let vol: f64 = out.bodies.iter().map(|b| b.volume()).sum();
                    let mass = self.doc.mass_of(i);
                    total_mass += mass.unwrap_or(0.0);
                    let mat =
                        self.doc.material.get(&i).map(|m| m.name.clone()).unwrap_or_else(|| "(no material)".into());
                    lines.push(format!(
                        "{i} {}: {} bodies, {}, {}, {}",
                        n.feature.name(),
                        out.bodies.len(),
                        self.doc.fmt_volume(vol),
                        mat,
                        mass.map(|m| format!("{m:.2} g")).unwrap_or_default()
                    ));
                }
                for l in &lines {
                    log::info!("BOM {l}");
                }
                self.status = format!(
                    "BOM: {} items, total mass {total_mass:.2} g. Full list in the log (RUST_LOG=info).",
                    lines.len()
                );
            }
            RibbonAction::Export3mf => {
                let p = PathBuf::from(&self.file_path).with_extension("3mf");
                self.status = match anvil_io::write_3mf(&self.doc, &p) {
                    Ok(n) => format!("Wrote {} with {n} objects (one per feature)", p.display()),
                    Err(e) => format!("3MF export failed: {e}"),
                };
            }
            RibbonAction::ExportStlParts => {
                let p = PathBuf::from(&self.file_path).with_extension("stl");
                self.status = match anvil_io::write_stl_parts(&self.doc, &p) {
                    Ok(files) => format!("Wrote {} STL files next to {}", files.len(), p.display()),
                    Err(e) => format!("STL export failed: {e}"),
                };
            }
            RibbonAction::SampleCard => {
                self.doc = anvil_io::business_card("Your Name", "https://www.linkedin.com/in/your-handle");
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.camera.fit(&self.scene.bounds);
                self.file_path = "business_card.anvil".into();
                self.status = "Business card loaded. Edit the Text and QR features, then File > 3MF (parts).".into();
            }
            RibbonAction::PressPull => self.press_pull(),
            RibbonAction::Interference => {
                let pair = match (self.panels.selected, self.multi.first()) {
                    (Some(a), Some(&b)) if a != b => Some((a, b)),
                    _ => None,
                };
                match pair {
                    None => self.status = "Select one feature, then Ctrl+click a second, then Interference".into(),
                    Some((a, b)) => {
                        let bodies = |i: usize| {
                            self.doc.features[i].output.as_ref().map(|o| o.bodies.clone()).unwrap_or_default()
                        };
                        let kernel = anvil_kernel::NativeKernel;
                        let mut vol = 0.0;
                        for x in bodies(a) {
                            for y in bodies(b) {
                                if let Ok(i) =
                                    anvil_kernel::Kernel::boolean(&kernel, &x, &y, anvil_kernel::BooleanOp::Intersect)
                                {
                                    vol += i.volume().max(0.0);
                                }
                            }
                        }
                        self.status = if vol > 1e-6 {
                            format!("Interference between features {a} and {b}: {}", self.doc.fmt_volume(vol))
                        } else {
                            format!("No interference between features {a} and {b}")
                        };
                    }
                }
            }
            RibbonAction::KettleGated => {
                self.doc = anvil_io::kettle::kettle_gated();
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.camera.fit(&self.scene.bounds);
                self.file_path = "kettle_gated.anvil".into();
                self.status = "Kettle with gating loaded. Gating features are on the Casting tab.".into();
            }
            RibbonAction::Kettle => {
                self.doc = anvil_io::kettle::kettle();
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.camera.fit(&self.scene.bounds);
                self.file_path = "kettle.anvil".into();
                self.status = "Kettle loaded. Pick the dome and add Pattern on face to change the dots.".into();
            }
            RibbonAction::Workbook(i) => {
                let (name, build) = anvil_io::workbook::EXERCISES[i as usize % 6];
                self.doc = build();
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.camera.fit(&self.scene.bounds);
                self.file_path = format!("workbook_{}.anvil", &name[..2]);
                self.status = format!("Workbook {name} loaded. Steps in docs/WORKBOOK.md.");
            }
            RibbonAction::ToggleUnits => {
                self.doc.unit = if self.doc.unit == "mm" { "in".into() } else { "mm".into() };
                self.status = format!("Display unit: {}", self.doc.unit);
            }
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
            if self.selected_face.is_some() {
                self.sketch_on_selected_face();
            } else {
                self.begin_pick_plane();
            }
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
            let mut placed = false;
            if !self.selected_edges.is_empty() {
                let body_fi = self.scene.bodies[self.selected_edges[0].0 as usize].0;
                let edges: Vec<[DVec3; 2]> = self.selected_edges.iter().map(|(_, e)| *e).collect();
                if f.set_edges(edges, body_fi) {
                    placed = true;
                    self.selected_edges.clear();
                }
            }
            if let (false, Some((_, fid, tri))) = (placed, self.selected_face) {
                // A curved surface first: features like Pattern on face
                // take every facet that shares the picked face's tag.
                if let (Some(solid), Some((body_fi, _))) =
                    (self.scene.solid_of_tri(&self.doc, tri), self.scene.body_of_tri(tri))
                {
                    if let Some(face) = solid.faces.get(fid) {
                        placed = f.place_on_surface(face.surface, body_fi);
                    }
                }
            }
            if let (false, Some((_, _, tri))) = (placed, self.selected_face) {
                if let (Some(plane), Some((body_fi, _))) =
                    (self.scene.face_plane(&self.doc, tri), self.scene.body_of_tri(tri))
                {
                    placed = f.place_on_face(plane, body_fi);
                }
            }
            let idx = self.doc.add_feature(f);
            if placed {
                self.selected_face = None;
            }
            self.panels.selected = Some(idx);
            self.invalidate();
            self.status = match &self.doc.features[idx].error {
                Some(e) => format!("Added {}: {e}", d.label),
                None => format!("Added {}. Edit its parameters in the Properties panel.", d.label),
            };
        }
    }

    fn press_pull(&mut self) {
        let Some((_, _, tri)) = self.selected_face else {
            self.status = "Click a face first, then Press Pull".into();
            return;
        };
        let Some((plane, outer, holes)) = self.scene.face_loops(&self.doc, tri) else {
            self.status = "Face not found".into();
            return;
        };
        let source = self.scene.body_of_tri(tri).map(|(fi, _)| fi).unwrap_or(0);
        let f =
            anvil_feature::features::emboss::FaceExtrudeFeature { plane, outer, holes, distance: "5".into(), source };
        let idx = self.doc.add_feature(Box::new(f));
        self.panels.selected = Some(idx);
        self.selected_face = None;
        self.invalidate();
        self.status = "Press Pull added as a new body. Set its distance in Properties (negative goes inward).".into();
    }

    fn sketch_on_selected_face(&mut self) {
        let Some((_, _, tri)) = self.selected_face else {
            self.begin_pick_plane();
            return;
        };
        if let Some(plane) = self.scene.face_plane(&self.doc, tri) {
            let segs: Vec<[DVec3; 2]> = self.scene.edges.iter().map(|(_, e)| *e).collect();
            let mut sk = SketchFeature::on_plane(plane);
            sk.sketch.project_segments(&segs, 1e-4);
            self.selected_face = None;
            self.start_sketch_on(Box::new(sk));
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
        let mut import_dxf = false;
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
                    ui.label("chamfer");
                    ui.add(egui::DragValue::new(&mut ed.chamfer).range(0.01..=1000.0).speed(0.1));
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
                    ui.add(egui::TextEdit::singleline(&mut ed.dxf_path).desired_width(110.0).hint_text("file.dxf"));
                    if ui
                        .button("Import DXF")
                        .on_hover_text("Add lines, arcs, circles, and polylines from a DXF file (mm)")
                        .clicked()
                    {
                        import_dxf = true;
                    }
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
        if import_dxf {
            ed.import_dxf(&mut self.doc);
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
        // Shortcuts are ignored while a text field has keyboard focus.
        let typing = ui.ctx().wants_keyboard_input();
        let esc = !typing && ui.input(|i| i.key_pressed(egui::Key::Escape));
        let del = !typing && ui.input(|i| i.key_pressed(egui::Key::Delete));
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
        self.hovered_face =
            self.hovered_tri.and_then(|t| Some((self.scene.tri_body.get(t).copied()?, self.scene.face_of_tri(t)?)));
        self.hovered_datum = None;
        let ctrl = ui.input(|i| i.modifiers.command);
        // Edge under the cursor: nearest visible feature edge within 6 px.
        self.hovered_edge = None;
        if matches!(self.mode, Mode::Model) && matches!(self.filter, SelectFilter::All | SelectFilter::Edge) {
            if let Some((px, py)) = pointer {
                let mut best: Option<(f64, u32, [DVec3; 2])> = None;
                for (body, [p, q]) in &self.scene.edges {
                    let (Some(a), Some(b)) = (proj.project(*p), proj.project(*q)) else { continue };
                    let (ax, ay, bx, by) = (a.0, a.1, b.0, b.1);
                    let (dx, dy) = (bx - ax, by - ay);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 < 1e-9 { 0.0 } else { (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0) };
                    let (cx, cy) = (ax + dx * t, ay + dy * t);
                    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                    if d > 6.0 {
                        continue;
                    }
                    // Visible if nothing in the depth buffer is clearly in front.
                    let (ix, iy) = (cx as usize, cy as usize);
                    if ix < self.fb.width && iy < self.fb.height {
                        let zb = self.fb.depth[iy * self.fb.width + ix];
                        let ze = (a.2 + (b.2 - a.2) * t) as f32;
                        if zb.is_finite() && ze > zb * 1.01 + 0.05 {
                            continue;
                        }
                    }
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, *body, [*p, *q]));
                    }
                }
                self.hovered_edge = best.map(|(_, b, e)| (b, e));
            }
        }

        let selected_body = self
            .panels
            .selected
            .and_then(|sel| self.scene.bodies.iter().position(|(fi, _)| *fi == sel))
            .map(|i| i as u32);

        match &mut self.mode {
            Mode::Model => {
                if del && resp.hovered() && (self.panels.selected.is_some() || !self.multi.is_empty()) {
                    self.run_action(RibbonAction::DeleteFeature);
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.any())
                    && !self.selected_edges.is_empty()
                {
                    self.add_feature_by_id("fillet");
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::Q)) && self.selected_face.is_some() {
                    self.press_pull();
                }
                if resp.drag_started_by(egui::PointerButton::Primary) && shift {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.box_select = Some((p, p));
                    }
                }
                if resp.dragged_by(egui::PointerButton::Primary) {
                    if let Some((a, _)) = self.box_select {
                        if let Some(p) = resp.interact_pointer_pos() {
                            self.box_select = Some((a, p));
                        }
                    } else {
                        let d = resp.drag_delta();
                        self.camera.orbit(d.x as f64, d.y as f64);
                    }
                }
                if resp.drag_stopped_by(egui::PointerButton::Primary) {
                    if let Some((a, b)) = self.box_select.take() {
                        // Select every body whose projected points all fall in the box.
                        let r = egui::Rect::from_two_pos(a, b);
                        let mut hits: Vec<usize> = Vec::new();
                        for (bi, (fi, _)) in self.scene.bodies.iter().enumerate() {
                            let mut all = true;
                            let mut any = false;
                            for (t, &tb) in self.scene.tri_body.iter().enumerate() {
                                if tb as usize != bi {
                                    continue;
                                }
                                for k in 0..3 {
                                    let v = self.scene.mesh.positions[self.scene.mesh.indices[t * 3 + k] as usize];
                                    match proj.project(v) {
                                        Some((x, y, _)) => {
                                            let p = Pos2::new(rect.left() + x as f32, rect.top() + y as f32);
                                            if r.contains(p) {
                                                any = true
                                            } else {
                                                all = false
                                            }
                                        }
                                        None => all = false,
                                    }
                                }
                            }
                            if any && all {
                                hits.push(*fi);
                            }
                        }
                        hits.dedup();
                        if let Some(first) = hits.first() {
                            self.panels.selected = Some(*first);
                            self.multi = hits[1..].to_vec();
                            self.status = format!("Selected {} feature(s)", hits.len());
                        }
                    }
                }
                if let (true, Some((b, e))) = (resp.clicked(), self.hovered_edge) {
                    let same = |x: &(u32, [DVec3; 2])| {
                        x.0 == b && (x.1[0] - e[0]).length() < 1e-9 && (x.1[1] - e[1]).length() < 1e-9
                    };
                    if let Some(k) = self.selected_edges.iter().position(same) {
                        self.selected_edges.remove(k);
                    } else {
                        if !ctrl && !shift {
                            self.selected_edges.clear();
                        }
                        self.selected_edges.push((b, e));
                    }
                    self.selected_face = None;
                    self.panels.selected = Some(self.scene.bodies[b as usize].0);
                    self.status = format!(
                        "{} edge(s) selected. Ctrl+click adds edges. Fillet (F) or Chamfer uses them.",
                        self.selected_edges.len()
                    );
                } else if resp.clicked() {
                    if !ctrl {
                        self.selected_edges.clear();
                    }
                    match self.hovered_body {
                        Some(b) => {
                            let (fi, _) = self.scene.bodies[b as usize];
                            if ctrl {
                                if self.panels.selected == Some(fi) {
                                    self.panels.selected = self.multi.pop();
                                } else if let Some(k) = self.multi.iter().position(|&m| m == fi) {
                                    self.multi.remove(k);
                                } else {
                                    if let Some(prev) = self.panels.selected {
                                        self.multi.push(prev);
                                    }
                                    self.panels.selected = Some(fi);
                                }
                            } else {
                                self.panels.selected = Some(fi);
                                self.multi.clear();
                            }
                            self.selected_face = if self.filter == SelectFilter::Body {
                                None
                            } else {
                                self.hovered_tri.and_then(|t| Some((b, self.scene.face_of_tri(t)?, t)))
                            };
                        }
                        None => {
                            if !ctrl {
                                self.panels.selected = None;
                                self.multi.clear();
                            }
                            self.selected_face = None;
                        }
                    }
                }
                // Right-click menu.
                let has_face = self.selected_face.is_some() || self.hovered_face.is_some();
                let mut act: Option<RibbonAction> = None;
                let mut sketch_here = false;
                resp.context_menu(|ui| {
                    if has_face {
                        if ui.button("Sketch on this face").clicked() {
                            sketch_here = true;
                            ui.close();
                        }
                        if ui.button("Press Pull this face (Q)").clicked() {
                            act = Some(RibbonAction::PressPull);
                            ui.close();
                        }
                        ui.separator();
                    }
                    if ui.button("Sketch (pick plane)").clicked() {
                        sketch_here = false;
                        act = None;
                        self.selected_face = None;
                        self.begin_pick_plane();
                        ui.close();
                    }
                    if ui.button("Measure").clicked() {
                        act = Some(RibbonAction::Measure);
                        ui.close();
                    }
                    if ui.button("Delete feature (Del)").clicked() {
                        act = Some(RibbonAction::DeleteFeature);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Fit view").clicked() {
                        act = Some(RibbonAction::FitView);
                        ui.close();
                    }
                    if ui.button("Isometric").clicked() {
                        act = Some(RibbonAction::ViewIso);
                        ui.close();
                    }
                });
                if sketch_here {
                    if self.selected_face.is_none() {
                        if let Some(t) = self.hovered_tri {
                            self.selected_face = self.hovered_face.map(|(b, f)| (b, f, t));
                        }
                    }
                    self.sketch_on_selected_face();
                }
                if let Some(a) = act {
                    self.run_action(a);
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
                if esc || (secondary_clicked && !ed.clicks.is_empty()) {
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
                let key = |k: egui::Key| !typing && ui.input(|i| i.key_pressed(k) && !i.modifiers.any());
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
                    ed.on_drag_end(&mut self.doc, shift || ctrl);
                    self.scene_dirty = true;
                }
                if resp.clicked() {
                    if let Some(p) = sp {
                        ed.on_click(p, shift || ctrl, &proj, &mut self.doc);
                        self.scene_dirty = true;
                    }
                }
                if ed.editing.is_some() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ed.commit_edit(&mut self.doc);
                    self.scene_dirty = true;
                }
                let mut pick: Option<Tool> = None;
                let mut finish = false;
                resp.context_menu(|ui| {
                    for t in [
                        Tool::Select,
                        Tool::Line,
                        Tool::Rect2,
                        Tool::CircleCenter,
                        Tool::Arc3,
                        Tool::Dimension,
                        Tool::Trim,
                        Tool::Fillet,
                    ] {
                        if ui.button(t.label()).clicked() {
                            pick = Some(t);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Finish Sketch").clicked() {
                        finish = true;
                        ui.close();
                    }
                });
                if let Some(t) = pick {
                    ed.set_tool(t);
                }
                if finish {
                    self.finish_sketch();
                }
            }
        }

        // Render.
        self.style.section = if self.section_on {
            let n = [DVec3::X, DVec3::Y, DVec3::Z][self.section_axis.min(2)];
            let n = if self.section_flip { -n } else { n };
            let w = n.dot([DVec3::X, DVec3::Y, DVec3::Z][self.section_axis.min(2)] * self.section_offset);
            Some((n, w))
        } else {
            None
        };
        self.fb.clear(self.style.background);
        let in_model = matches!(self.mode, Mode::Model | Mode::PickPlane);
        let hovered = if in_model { self.hovered_body } else { None };
        let sel_face = self.selected_face.map(|(b, f, _)| (b, f));
        let hov_face = if in_model { self.hovered_face } else { None };
        self.fb.draw_scene_faces(&self.scene, &proj, &self.style, selected_body, hovered, sel_face, hov_face);
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
        if let Some((a, b)) = self.box_select {
            painter.rect_stroke(
                egui::Rect::from_two_pos(a, b),
                0.0,
                Stroke::new(1.0f32, Color32::from_rgb(60, 100, 220)),
                egui::StrokeKind::Middle,
            );
        }
        if let Some(c) = self.com_marker {
            if let Some(p) = to_screen(c) {
                painter.circle_stroke(p, 7.0, Stroke::new(2.0f32, Color32::from_rgb(220, 40, 40)));
                painter.line_segment(
                    [p - egui::vec2(10.0, 0.0), p + egui::vec2(10.0, 0.0)],
                    Stroke::new(1.0f32, Color32::from_rgb(220, 40, 40)),
                );
                painter.line_segment(
                    [p - egui::vec2(0.0, 10.0), p + egui::vec2(0.0, 10.0)],
                    Stroke::new(1.0f32, Color32::from_rgb(220, 40, 40)),
                );
            }
        }
        for (_, [p, q]) in &self.selected_edges {
            if let (Some(a), Some(b)) = (to_screen(*p), to_screen(*q)) {
                painter.line_segment([a, b], Stroke::new(3.5f32, Color32::from_rgb(240, 150, 30)));
            }
        }
        if let Some((_, [p, q])) = self.hovered_edge {
            if let (Some(a), Some(b)) = (to_screen(p), to_screen(q)) {
                painter.line_segment([a, b], Stroke::new(3.0f32, Color32::from_rgb(60, 160, 240)));
            }
        }
        // What is under the cursor.
        if matches!(self.mode, Mode::Model | Mode::PickPlane) {
            let label = if let Some((b, [p, q])) = self.hovered_edge {
                let fi = self.scene.bodies[b as usize].0;
                Some(format!(
                    "Edge {} long, {}",
                    self.doc.fmt_length((q - p).length()),
                    self.doc.features[fi].feature.name()
                ))
            } else if let Some(b) = self.hovered_body {
                let fi = self.scene.bodies[b as usize].0;
                Some(format!("Face of {}", self.doc.features[fi].feature.name()))
            } else {
                None
            };
            if let Some(label) = label {
                painter.text(
                    rect.left_bottom() + egui::vec2(10.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    label,
                    egui::FontId::proportional(13.0),
                    Color32::from_rgb(50, 55, 65),
                );
            }
        }
        self.draw_triad(&painter, rect);
        // View buttons in the top right corner, like a simplified ViewCube.
        if !matches!(self.mode, Mode::Sketch(_)) {
            let views = [
                ("Top", RibbonAction::ViewTop),
                ("Front", RibbonAction::ViewFront),
                ("Right", RibbonAction::ViewRight),
                ("Iso", RibbonAction::ViewIso),
                ("Fit", RibbonAction::FitView),
            ];
            let mut clicked = None;
            for (k, (label, act)) in views.iter().enumerate() {
                let r = egui::Rect::from_min_size(
                    Pos2::new(rect.right() - 60.0, rect.top() + 8.0 + k as f32 * 26.0),
                    egui::vec2(52.0, 22.0),
                );
                if ui.put(r, egui::Button::new(*label).small()).clicked() {
                    clicked = Some(*act);
                }
            }
            if let Some(a) = clicked {
                self.run_action(a);
            }
        }
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
        let typing = ctx.wants_keyboard_input();
        let (undo, redo) = ctx.input(|i| {
            (i.modifiers.command && i.key_pressed(egui::Key::Z), i.modifiers.command && i.key_pressed(egui::Key::Y))
        });
        if undo && !typing {
            self.run_action(RibbonAction::Undo);
        }
        if redo && !typing {
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
                ui.separator();
                ui.label("Select:");
                for (f, label) in [
                    (SelectFilter::All, "All"),
                    (SelectFilter::Body, "Body"),
                    (SelectFilter::Face, "Face"),
                    (SelectFilter::Edge, "Edge"),
                ] {
                    ui.selectable_value(&mut self.filter, f, label);
                }
                ui.separator();
                ui.checkbox(&mut self.section_on, "Section");
                if self.section_on {
                    egui::ComboBox::from_id_salt("section_axis")
                        .width(40.0)
                        .selected_text(["X", "Y", "Z"][self.section_axis.min(2)])
                        .show_ui(ui, |ui| {
                            for (k, l) in ["X", "Y", "Z"].iter().enumerate() {
                                ui.selectable_value(&mut self.section_axis, k, *l);
                            }
                        });
                    ui.add(egui::DragValue::new(&mut self.section_offset).speed(0.5).suffix(" mm"));
                    ui.checkbox(&mut self.section_flip, "flip");
                }
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
            if let Some(sel) = self.panels.selected {
                let takes_edges =
                    self.doc.features.get(sel).map(|f| f.feature.clone_box().set_edges(Vec::new(), 0)).unwrap_or(false);
                if takes_edges {
                    ui.separator();
                    let n = self.selected_edges.len();
                    let btn = ui
                        .add_enabled(n > 0, egui::Button::new(format!("Use selected edges ({n})")))
                        .on_hover_text("Click edges in the viewport (Ctrl+click for more), then press this");
                    if btn.clicked() {
                        let edges: Vec<[DVec3; 2]> = self.selected_edges.iter().map(|(_, e)| *e).collect();
                        let picked = self.scene.bodies[self.selected_edges[0].0 as usize].0;
                        // Edges picked on this feature's own result belong to its input body.
                        let body = if picked == sel {
                            self.doc.features[sel]
                                .feature
                                .params()
                                .iter()
                                .find_map(|p| match p.value {
                                    anvil_feature::ParamValue::FeatureRef(i) => Some(i),
                                    _ => None,
                                })
                                .unwrap_or(picked)
                        } else {
                            picked
                        };
                        self.doc.edit_feature(sel, |f| {
                            f.set_edges(edges, body);
                        });
                        self.selected_edges.clear();
                        self.scene_dirty = true;
                    }
                }
                if self
                    .doc
                    .features
                    .get(sel)
                    .and_then(|f| f.output.as_ref())
                    .map(|o| !o.bodies.is_empty())
                    .unwrap_or(false)
                {
                    ui.separator();
                    ui.heading("Appearance");
                    ui.horizontal(|ui| {
                        if let Some(c) = self.doc.appearance.get(&sel) {
                            self.color_edit = *c;
                        }
                        if ui.color_edit_button_srgb(&mut self.color_edit).changed() {
                            self.doc.appearance.insert(sel, self.color_edit);
                            self.scene_dirty = true;
                        }
                        if ui.button("Reset").clicked() {
                            self.doc.appearance.remove(&sel);
                            self.scene_dirty = true;
                        }
                    });
                    ui.heading("Physical Material");
                    let current =
                        self.doc.material.get(&sel).map(|m| m.name.clone()).unwrap_or_else(|| "(none)".into());
                    egui::ComboBox::from_id_salt(("material", sel)).selected_text(&current).show_ui(ui, |ui| {
                        if ui.selectable_label(current == "(none)", "(none)").clicked() {
                            self.doc.material.remove(&sel);
                        }
                        for (name, density) in anvil_feature::MATERIALS {
                            if ui.selectable_label(current == *name, format!("{name} ({density} g/cm3)")).clicked() {
                                self.doc
                                    .material
                                    .insert(sel, anvil_feature::Material { name: name.to_string(), density: *density });
                            }
                        }
                    });
                    if let Some(m) = self.doc.mass_of(sel) {
                        ui.label(format!("Mass {m:.2} g"));
                    }
                }
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
