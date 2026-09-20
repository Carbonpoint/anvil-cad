use crate::camera::{Camera, Projector, UpAxis};
use crate::drag_handle::{self, DragHandle};
use crate::icons;
use crate::panels::{self, PanelState};
use crate::raster::{Framebuffer, Style};
use crate::ribbon::{build_ribbon, ButtonKind, RibbonAction, RibbonTab};
use crate::scene::Scene;
use crate::sketch_editor::{ConstraintTool, DimensionTool, SketchEditor, Tool};
use crate::sketch_view;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::features::{extrude::ExtrudeFeature, revolve::RevolveFeature};
use anvil_feature::{descriptor, Document};
use anvil_math::{DVec2, DVec3, Plane};
use egui::{Color32, Pos2, Stroke};
use std::path::PathBuf;

/// A named direction to look from. The six box views are orthographic,
/// like a drawing; Angled and Perspective look from a corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    Angled,
    Perspective,
}

impl StandardView {
    pub const ALL: [StandardView; 8] = [
        StandardView::Top,
        StandardView::Bottom,
        StandardView::Front,
        StandardView::Back,
        StandardView::Left,
        StandardView::Right,
        StandardView::Angled,
        StandardView::Perspective,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StandardView::Top => "Top",
            StandardView::Bottom => "Bottom",
            StandardView::Front => "Front",
            StandardView::Back => "Back",
            StandardView::Left => "Left",
            StandardView::Right => "Right",
            StandardView::Angled => "Angled",
            StandardView::Perspective => "Perspective",
        }
    }

    /// (yaw, pitch, orthographic).
    fn angles(self) -> (f64, f64, bool) {
        use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        match self {
            StandardView::Top => (-FRAC_PI_2, 1.5699, true),
            StandardView::Bottom => (-FRAC_PI_2, -1.5699, true),
            StandardView::Front => (-FRAC_PI_2, 0.0, true),
            StandardView::Back => (FRAC_PI_2, 0.0, true),
            StandardView::Left => (PI, 0.0, true),
            StandardView::Right => (0.0, 0.0, true),
            StandardView::Angled => (-FRAC_PI_4, FRAC_PI_4 * 0.8, true),
            StandardView::Perspective => (-FRAC_PI_4, FRAC_PI_4 * 0.8, false),
        }
    }
}

/// One pane of the four pane layout: its camera and its own framebuffer.
struct Pane {
    camera: Camera,
    view: StandardView,
    fb: Framebuffer,
    texture: Option<egui::TextureHandle>,
}

/// The view each pane starts in, in pane order.
const PANE_VIEWS: [StandardView; 4] =
    [StandardView::Front, StandardView::Right, StandardView::Top, StandardView::Perspective];

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
    /// Also draw sketches that a later feature already uses.
    show_used_sketches: bool,
    /// A distance drag in progress: (feature, parameter, distance at the
    /// start, the text the parameter had, pointer position at the start).
    handle_drag: Option<(usize, &'static str, f64, String, Pos2)>,
    /// The pane the view commands act on (the last one under the pointer).
    active_pane: usize,
    /// Four panes instead of one (Front, Right, Top, and an angled view).
    quad: bool,
    panes: Vec<Pane>,
    /// Reverse the mouse wheel zoom direction.
    invert_scroll: bool,
    /// CPU, memory, and fps overlay (View > Settings > Performance).
    show_perf: bool,
    perf: crate::perf::PerfMonitor,
    /// UI scale and text size (View > Settings > Settings), persisted.
    settings: crate::settings::UiSettings,
    settings_window_open: bool,
    /// In sketch mode: the Sketch tab is showing (not another ribbon tab).
    sketch_tab: bool,
    /// Where the 3D view was drawn last frame, in points.
    view_rect: egui::Rect,
    section_axis: usize,
    section_offset: f64,
    section_flip: bool,
    /// Export dialog open, with the format picked so far.
    export_dialog: Option<anvil_io::export::Format>,
    /// Last format used, the default next time.
    export_format: anvil_io::export::Format,
    /// A write waiting for the user to confirm an overwrite.
    pending_write: Option<(PathBuf, anvil_io::export::Format)>,
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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::new_headless();
        if let Some(storage) = cc.storage {
            if let Some(s) =
                eframe::get_value::<crate::settings::UiSettings>(storage, crate::settings::UiSettings::STORAGE_KEY)
            {
                app.settings = s;
            }
        }
        app.settings.apply(&cc.egui_ctx);
        app
    }

    /// The app without a window, for layout tests.
    pub fn new_headless() -> Self {
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
            export_dialog: None,
            export_format: anvil_io::export::Format::Stl,
            pending_write: None,
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
            show_used_sketches: false,
            view_rect: egui::Rect::NOTHING,
            sketch_tab: true,
            handle_drag: None,
            invert_scroll: false,
            quad: false,
            panes: Vec::new(),
            active_pane: 0,
            show_perf: false,
            perf: Default::default(),
            settings: crate::settings::UiSettings::default(),
            settings_window_open: false,
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
        self.fit_view();
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

    /// The camera the view commands act on: in the four pane layout it is
    /// the pane under the pointer, otherwise the single camera.
    fn active_camera(&mut self) -> &mut Camera {
        match (self.quad, self.panes.get_mut(self.active_pane)) {
            (true, Some(p)) => &mut p.camera,
            _ => &mut self.camera,
        }
    }

    /// Point the active camera at a named view.
    fn set_view(&mut self, view: StandardView) {
        self.refresh_scene();
        let bounds = self.scene.bounds;
        let height = self.scene_size().max(1.0) * 1.2;
        let quad = self.quad;
        let pane = self.active_pane;
        let (yaw, pitch, ortho) = view.angles();
        let cam = self.active_camera();
        cam.unlock();
        cam.yaw = yaw;
        cam.pitch = pitch;
        cam.set_ortho(ortho, height);
        cam.fit_in(&bounds, 1.0);
        if quad {
            if let Some(p) = self.panes.get_mut(pane) {
                p.view = view;
            }
        }
        self.status = format!("{} view", view.label());
    }

    /// Build the four panes: Front, Right, Top, and an angled view down
    /// the (1, 1, 1) direction, each framed on the model.
    fn make_panes(&mut self) {
        self.refresh_scene();
        let bounds = self.scene.bounds;
        let height = self.scene_size().max(1.0) * 1.2;
        self.panes = PANE_VIEWS
            .iter()
            .map(|&view| {
                let (yaw, pitch, ortho) = view.angles();
                let mut camera = Camera { yaw, pitch, ..self.camera.clone() };
                camera.unlock();
                camera.set_ortho(ortho, height);
                camera.fit_in(&bounds, 1.0);
                Pane { camera, view, fb: Framebuffer::new(8, 8), texture: None }
            })
            .collect();
    }

    /// Frame every body in the 3D view.
    fn fit_view(&mut self) {
        self.refresh_scene();
        if self.quad {
            let bounds = self.scene.bounds;
            for p in &mut self.panes {
                p.camera.fit_in(&bounds, 1.0);
            }
        }
        let r = self.view_rect;
        let aspect = if r.height() > 1.0 && r.width() > 1.0 { (r.width() / r.height()) as f64 } else { 1.0 };
        self.camera.fit_in(&self.scene.bounds, aspect);
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
                self.sketch_tab = true;
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
            RibbonAction::SaveAs => {
                self.choose_file(anvil_io::export::Format::Anvil);
            }
            RibbonAction::Export => {
                self.export_dialog = Some(self.export_format);
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
                        self.fit_view();
                        self.status = format!("Loaded {}", p.display());
                    }
                    Err(e) => self.status = format!("Load failed: {e}"),
                }
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
                self.fit_view();
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
            RibbonAction::ToggleQuadView => {
                self.quad = !self.quad;
                if self.quad {
                    self.make_panes();
                    self.status =
                        "Four views: Front, Right, Top, and an angled view. Each pane orbits on its own.".into();
                } else {
                    self.status = "One view".into();
                }
            }
            RibbonAction::TogglePerf => self.show_perf = !self.show_perf,
            RibbonAction::ToggleScrollDir => {
                self.invert_scroll = !self.invert_scroll;
                self.status = if self.invert_scroll {
                    "Scroll direction reversed".into()
                } else {
                    "Scroll direction normal".into()
                };
            }
            RibbonAction::ToggleSettings => self.settings_window_open = !self.settings_window_open,
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
            RibbonAction::SampleCard => {
                self.doc = anvil_io::business_card("Your Name", "https://www.linkedin.com/in/your-handle");
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.fit_view();
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
            RibbonAction::KettleMold => {
                self.doc = anvil_io::kettle::kettle_mold();
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.fit_view();
                self.file_path = "kettle_mold.anvil".into();
                self.status = "Kettle mold loaded: pattern halves, core, and core box halves.".into();
            }
            RibbonAction::KettleGated => {
                self.doc = anvil_io::kettle::kettle_gated();
                self.panels = PanelState::default();
                self.mode = Mode::Model;
                self.camera.unlock();
                self.invalidate();
                self.refresh_scene();
                self.fit_view();
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
                self.fit_view();
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
                self.fit_view();
                self.file_path = format!("workbook_{}.anvil", &name[..2]);
                self.status = format!("Workbook {name} loaded. Steps in docs/WORKBOOK.md.");
            }
            RibbonAction::ToggleUnits => {
                self.doc.unit = if self.doc.unit == "mm" { "in".into() } else { "mm".into() };
                self.status = format!("Display unit: {}", self.doc.unit);
            }
            RibbonAction::ViewIso => self.set_view(StandardView::Angled),
            RibbonAction::ViewTop => self.set_view(StandardView::Top),
            RibbonAction::ViewFront => self.set_view(StandardView::Front),
            RibbonAction::ViewRight => self.set_view(StandardView::Right),
            RibbonAction::ToggleProjection => {
                self.refresh_scene();
                let h = self.scene_size().max(1.0) * 1.2;
                let cam = self.active_camera();
                let on = !cam.is_ortho();
                cam.set_ortho(on, h);
                self.status = if on { "Orthographic view".into() } else { "Perspective view".into() };
            }
            RibbonAction::SetUpAxis(a) => {
                let axis = [UpAxis::X, UpAxis::Y, UpAxis::Z, UpAxis::Free][(a as usize).min(3)];
                self.camera.set_up_axis(axis);
                for p in &mut self.panes {
                    p.camera.set_up_axis(axis);
                }
                self.status = match axis {
                    UpAxis::Free => "Free orbit: no axis is held vertical".into(),
                    a => format!("{} on screen", a.label()),
                };
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
            // Sketch refs start at the latest sketch in the history.
            if let Some(last) = self.doc.features.iter().rposition(|n| n.feature.kind() == "sketch") {
                for p in f.params() {
                    if let anvil_feature::param::ParamKind::FeatureRef { accepts } = &p.kind {
                        if accepts.contains(&"sketch") {
                            let _ = f.set_param(p.name, anvil_feature::ParamValue::FeatureRef(last));
                        }
                    }
                }
            }
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
                None => match sketch_view::region_pick(&self.doc, Some(idx)) {
                    Some(p) if p.regions.len() > 1 => format!(
                        "Added {} on {} of {} sketch regions. Click a region in the view to add or remove it.",
                        d.label,
                        p.used_count(),
                        p.regions.len()
                    ),
                    _ => format!("Added {}. Edit its parameters in the Properties panel.", d.label),
                },
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
        // While a sketch is open the other tabs stay reachable: a solid
        // command such as Extrude finishes the sketch and uses it.
        let in_sketch = matches!(self.mode, Mode::Sketch(_));
        if in_sketch {
            ui.horizontal(|ui| {
                let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
                icons::paint_logo(ui.painter(), logo_rect);
                ui.separator();
                if ui.selectable_label(self.sketch_tab, "Sketch").clicked() {
                    self.sketch_tab = true;
                }
                for (i, t) in self.ribbon.iter().enumerate() {
                    if ui.selectable_label(!self.sketch_tab && self.active_tab == i, t.name).clicked() {
                        self.active_tab = i;
                        self.sketch_tab = false;
                    }
                }
                if !self.sketch_tab {
                    ui.separator();
                    ui.label(egui::RichText::new("The sketch stays open. A solid command finishes it.").weak());
                }
            });
            if self.sketch_tab {
                self.sketch_ribbon(ui);
                return;
            }
        }
        let mut clicked_feature: Option<&'static str> = None;
        let mut clicked_action: Option<RibbonAction> = None;
        ui.horizontal(|ui| {
            if in_sketch {
                return;
            }
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            icons::paint_logo(ui.painter(), logo_rect);
            ui.separator();
            ui.add_enabled_ui(self.doc.can_undo(), |ui| {
                if icons::icon_button(ui, "undo", "Undo").clicked() {
                    clicked_action = Some(RibbonAction::Undo);
                }
            });
            ui.add_enabled_ui(self.doc.can_redo(), |ui| {
                if icons::icon_button(ui, "redo", "Redo").clicked() {
                    clicked_action = Some(RibbonAction::Redo);
                }
            });
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
                                let btn = icons::icon_button(ui, b.kind.icon_id(), b.label);
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
            if in_sketch {
                self.finish_sketch();
            }
            self.add_feature_by_id(id);
        }
        if let Some(a) = clicked_action {
            let view_only = matches!(
                a,
                RibbonAction::FitView
                    | RibbonAction::ViewIso
                    | RibbonAction::ViewTop
                    | RibbonAction::ViewFront
                    | RibbonAction::ViewRight
                    | RibbonAction::ToggleEdges
                    | RibbonAction::TogglePerf
                    | RibbonAction::ToggleScrollDir
                    | RibbonAction::ToggleQuadView
                    | RibbonAction::ToggleSettings
            );
            if in_sketch && !view_only {
                self.finish_sketch();
            }
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
            let (finish_icon, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            icons::paint("finish_sketch", ui.painter(), finish_icon, Color32::WHITE);
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
            if icons::icon_toggle(ui, t.icon_id(), t.label(), current == t).on_hover_text(t.hint()).clicked() {
                *out = Some(t);
                ui.close();
            }
        };
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if icons::icon_toggle(ui, "tool_select", "Select", ed.tool == Tool::Select).clicked() {
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
                        if icons::icon_toggle(ui, t.icon_id(), t.label(), ed.tool == t)
                            .on_hover_text(t.hint())
                            .clicked()
                        {
                            new_tool = Some(t);
                        }
                    }
                    ui.label("chamfer");
                    ui.add(egui::DragValue::new(&mut ed.chamfer).range(0.01..=1000.0).speed(0.1));
                    if icons::icon_only(ui, "offset", "Offset")
                        .on_hover_text("Offset the selection by the value field")
                        .clicked()
                    {
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
                        if icons::icon_only(ui, c.icon_id(), c.label()).on_hover_text(c.label()).clicked() {
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
                    if icons::icon_only(ui, DimensionTool::Smart.icon_id(), "Dimension")
                        .on_hover_text("Dimension: length, distance, radius, or angle from the selection (D)")
                        .clicked()
                    {
                        dimension = Some(DimensionTool::Smart);
                    }
                    if icons::icon_only(ui, DimensionTool::Length.icon_id(), "Length").on_hover_text("Length").clicked()
                    {
                        dimension = Some(DimensionTool::Length);
                    }
                    if icons::icon_only(ui, DimensionTool::Radius.icon_id(), "Radius").on_hover_text("Radius").clicked()
                    {
                        dimension = Some(DimensionTool::Radius);
                    }
                    if icons::icon_only(ui, DimensionTool::Angle.icon_id(), "Angle").on_hover_text("Angle").clicked() {
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

    /// The 3D view. In the four pane layout this runs once per pane, with
    /// that pane's camera and framebuffer swapped in.
    fn viewport(&mut self, ui: &mut egui::Ui) {
        self.refresh_scene();
        if !self.quad {
            self.viewport_inner(ui, true);
            return;
        }
        let full = ui.available_rect_before_wrap();
        let (w, h) = (full.width() * 0.5, full.height() * 0.5);
        let pointer = ui.ctx().pointer_latest_pos();
        let mut pick: Option<(usize, StandardView)> = None;
        for pane in 0..4 {
            let min = full.min + egui::vec2(if pane % 2 == 0 { 0.0 } else { w }, if pane < 2 { 0.0 } else { h });
            let rect = egui::Rect::from_min_size(min, egui::vec2(w - 1.0, h - 1.0));
            if pointer.is_some_and(|p| rect.contains(p)) {
                self.active_pane = pane;
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            std::mem::swap(&mut self.camera, &mut self.panes[pane].camera);
            std::mem::swap(&mut self.fb, &mut self.panes[pane].fb);
            std::mem::swap(&mut self.texture, &mut self.panes[pane].texture);
            self.viewport_inner(&mut child, false);
            std::mem::swap(&mut self.camera, &mut self.panes[pane].camera);
            std::mem::swap(&mut self.fb, &mut self.panes[pane].fb);
            std::mem::swap(&mut self.texture, &mut self.panes[pane].texture);
            // The name of the view is a menu: click it to look from
            // another direction.
            let label_rect = egui::Rect::from_min_size(rect.left_top() + egui::vec2(6.0, 4.0), egui::vec2(110.0, 20.0));
            let mut label_ui = ui.new_child(
                egui::UiBuilder::new().max_rect(label_rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let name = self.panes[pane].view.label();
            label_ui
                .menu_button(name, |ui| {
                    for v in StandardView::ALL {
                        if ui.button(v.label()).clicked() {
                            pick = Some((pane, v));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Look from another direction");
        }
        if let Some((pane, v)) = pick {
            self.active_pane = pane;
            self.set_view(v);
        }
        // Lines between the panes.
        let mid = full.center();
        let line = Stroke::new(1.0f32, Color32::from_rgb(150, 155, 165));
        ui.painter().line_segment([Pos2::new(mid.x, full.top()), Pos2::new(mid.x, full.bottom())], line);
        ui.painter().line_segment([Pos2::new(full.left(), mid.y), Pos2::new(full.right(), mid.y)], line);
    }

    fn viewport_inner(&mut self, ui: &mut egui::Ui, single: bool) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        if single || resp.contains_pointer() {
            self.view_rect = rect;
        }
        let w = (rect.width().max(8.0)) as usize;
        let h = (rect.height().max(8.0)) as usize;
        if self.fb.width != w || self.fb.height != h {
            self.fb = Framebuffer::new(w, h);
        }
        let proj = Projector::new(&self.camera, w as f64, h as f64);
        let pointer = resp.hover_pos().map(|p| ((p.x - rect.left()) as f64, (p.y - rect.top()) as f64));
        let shift = ui.input(|i| i.modifiers.shift);
        // Shortcuts are ignored while a text field has keyboard focus.
        // Shortcuts belong to the pane under the pointer, and are ignored
        // while a text field has keyboard focus.
        let typing = ui.ctx().wants_keyboard_input() || !(single || resp.contains_pointer());
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
                let sign = if self.invert_scroll { 1.0 } else { -1.0 };
                self.camera.zoom((sign * scroll as f64 * 0.002).exp());
            }
            // Touchpad pinch and Ctrl+scroll arrive as a zoom factor.
            let pinch = ui.input(|i| i.zoom_delta()) as f64;
            if (pinch - 1.0).abs() > 1e-4 {
                self.camera.zoom(1.0 / pinch);
            }
        }
        // Keyboard zoom for laptops without a wheel: Home fits, + and - zoom.
        if !typing {
            let (home, plus, minus) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Home),
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                )
            });
            if home && !matches!(self.mode, Mode::Sketch(_)) {
                self.fit_view();
            }
            if plus {
                self.camera.zoom(0.8);
            }
            if minus {
                self.camera.zoom(1.25);
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

        // Regions of the selected feature's sketch, for Extrude, Revolve and
        // other features that pick regions.
        let pick = match self.mode {
            Mode::Model => sketch_view::region_pick(&self.doc, self.panels.selected),
            _ => None,
        };
        let hovered_region = pick.as_ref().zip(pointer).and_then(|(pk, (x, y))| pk.region_at(&proj, x, y));
        // The arrow that drags a distance, for the selected feature.
        let handle = match self.mode {
            Mode::Model => drag_handle::handle_for(&self.doc, self.panels.selected),
            _ => None,
        };
        let handle_screen = handle.as_ref().and_then(|h| {
            let o = proj.project(h.origin)?;
            let t = proj.project(h.tip())?;
            let u = proj.project(h.origin + h.dir)?;
            // Pixels per document unit along the arrow.
            let px = ((u.0 - o.0).powi(2) + (u.1 - o.1).powi(2)).sqrt();
            Some((Pos2::new(o.0 as f32, o.1 as f32), Pos2::new(t.0 as f32, t.1 as f32), (u.0 - o.0, u.1 - o.1), px))
        });
        let on_handle = handle_screen
            .zip(pointer)
            .is_some_and(|((_, t, _, _), (x, y))| ((x as f32 - t.x).powi(2) + (y as f32 - t.y).powi(2)).sqrt() < 12.0);
        if hovered_region.is_some() || on_handle || self.handle_drag.is_some() {
            self.hovered_edge = None;
        }

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
                if resp.drag_started_by(egui::PointerButton::Primary) && on_handle {
                    if let (Some(h), Some(p)) = (&handle, resp.interact_pointer_pos()) {
                        let expr = self.doc.features[h.feature]
                            .feature
                            .params()
                            .into_iter()
                            .find(|q| q.name == h.param)
                            .map(|q| match q.value {
                                anvil_feature::ParamValue::Expr(e) => e,
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        self.handle_drag = Some((h.feature, h.param, h.distance, expr, p));
                    }
                }
                if let Some((fi, name, start, _, from)) = self.handle_drag.clone() {
                    if resp.dragged_by(egui::PointerButton::Primary) {
                        if let (Some(p), Some((_, _, (ax, ay), px))) = (resp.interact_pointer_pos(), handle_screen) {
                            if px > 1e-6 {
                                let (dx, dy) = ((p.x - from.x) as f64, (p.y - from.y) as f64);
                                let along = (dx * ax + dy * ay) / (ax * ax + ay * ay).sqrt();
                                let d = DragHandle::snap(start + along / px, shift);
                                let txt = format!("{d}");
                                self.doc.edit_feature_quiet(fi, |f| {
                                    let _ = f.set_param(name, anvil_feature::ParamValue::Expr(txt));
                                });
                                self.panels.drafts.retain(|(i, _), _| *i != fi);
                                self.scene_dirty = true;
                                self.status = format!("Distance {}", self.doc.fmt_length(d));
                            }
                        }
                    }
                    if resp.drag_stopped_by(egui::PointerButton::Primary) {
                        // One undo step for the whole drag.
                        let (_, _, _, orig, _) = self.handle_drag.take().unwrap();
                        let now = self.doc.features[fi]
                            .feature
                            .params()
                            .into_iter()
                            .find(|q| q.name == name)
                            .map(|q| match q.value {
                                anvil_feature::ParamValue::Expr(e) => e,
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        self.doc.edit_feature_quiet(fi, |f| {
                            let _ = f.set_param(name, anvil_feature::ParamValue::Expr(orig));
                        });
                        self.doc.edit_feature(fi, |f| {
                            let _ = f.set_param(name, anvil_feature::ParamValue::Expr(now));
                        });
                        self.scene_dirty = true;
                    }
                }
                if resp.drag_started_by(egui::PointerButton::Primary) && shift && self.handle_drag.is_none() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.box_select = Some((p, p));
                    }
                }
                if resp.dragged_by(egui::PointerButton::Primary) && self.handle_drag.is_none() {
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
                if let (true, Some(pk), Some(r)) = (resp.clicked() && !shift, &pick, hovered_region) {
                    let spec = pk.toggled(r);
                    let (fi, name) = (pk.feature, pk.param);
                    self.doc.edit_feature(fi, |f| {
                        let _ = f.set_param(name, anvil_feature::ParamValue::Expr(spec));
                    });
                    self.panels.drafts.retain(|(i, _), _| *i != fi);
                    self.invalidate();
                    let now = sketch_view::region_pick(&self.doc, Some(fi));
                    self.status = match (now, &self.doc.features[fi].error) {
                        (_, Some(e)) => format!("{}: {e}", self.doc.features[fi].feature.name()),
                        (Some(p), None) => format!(
                            "{} of {} sketch regions used. Click a region to add or remove it.",
                            p.used_count(),
                            p.regions.len()
                        ),
                        (None, None) => String::new(),
                    };
                } else if let (true, Some((b, e))) = (resp.clicked(), self.hovered_edge) {
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
        if matches!(self.mode, Mode::Model | Mode::PickPlane) {
            let sel = self.panels.selected;
            for i in sketch_view::visible_sketches(&self.doc, sel, self.show_used_sketches) {
                sketch_view::draw_sketch(&painter, origin, &proj, &self.fb, &self.doc, i, sel == Some(i));
            }
            if let Some(pk) = &pick {
                sketch_view::draw_regions(&painter, origin, &proj, pk, hovered_region);
            }
        }
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
            let label = if let (Some(pk), Some(r)) = (&pick, hovered_region) {
                let used = anvil_feature::region_select::is_selected(&pk.regions, &pk.spec, r);
                Some(format!(
                    "Region {} of {}: click to {}",
                    r + 1,
                    pk.regions.len(),
                    if used { "leave it out" } else { "use it" }
                ))
            } else if let Some((b, [p, q])) = self.hovered_edge {
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
        if let (Some(h), Some((o, t, _, _))) = (&handle, handle_screen) {
            let col = if on_handle || self.handle_drag.is_some() {
                Color32::from_rgb(255, 170, 40)
            } else {
                Color32::from_rgb(210, 120, 20)
            };
            let (o, t) = (o + rect.min.to_vec2(), t + rect.min.to_vec2());
            painter.line_segment([o, t], Stroke::new(2.5f32, col));
            let dir = (t - o).normalized();
            if dir.length() > 0.5 {
                let side = egui::vec2(-dir.y, dir.x) * 6.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![t + dir * 12.0, t - dir * 4.0 + side, t - dir * 4.0 - side],
                    col,
                    Stroke::NONE,
                ));
            }
            painter.circle_filled(t, 5.0, col);
            painter.text(
                t + egui::vec2(10.0, -14.0),
                egui::Align2::LEFT_CENTER,
                self.doc.fmt_length(h.distance),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(70, 50, 10),
            );
        }
        self.draw_triad(&painter, rect);
        if self.show_perf {
            let lines = self.perf.lines();
            let font = egui::FontId::monospace(11.0);
            let pos = Pos2::new(rect.right() - 70.0, rect.top() + 8.0);
            let galley = painter.layout_no_wrap(lines.join("\n"), font, Color32::from_rgb(230, 235, 240));
            let r = egui::Rect::from_min_size(pos - egui::vec2(galley.size().x, 0.0), galley.size()).expand(5.0);
            painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(30, 35, 45, 190));
            painter.galley(r.min + egui::vec2(5.0, 5.0), galley, Color32::WHITE);
        }
        // View buttons in the top right corner, like a simplified ViewCube.
        if !matches!(self.mode, Mode::Sketch(_)) && single {
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

impl AnvilApp {
    /// Open the native save dialog for `format`, starting from the current
    /// document name. An existing file asks for confirmation first.
    fn choose_file(&mut self, format: anvil_io::export::Format) {
        let stem = PathBuf::from(&self.file_path);
        let stem = stem.file_stem().and_then(|s| s.to_str()).unwrap_or("part").to_string();
        let dir = PathBuf::from(&self.file_path).parent().map(|p| p.to_path_buf()).filter(|p| p.is_dir());
        let mut dialog = rfd::FileDialog::new()
            .set_title(format!("Export as {}", format.label()))
            .add_filter(format.label(), &[format.extension()])
            .set_file_name(format!("{stem}.{}", format.extension()));
        if let Some(d) = dir {
            dialog = dialog.set_directory(d);
        }
        let Some(mut path) = dialog.save_file() else {
            self.status = "Export cancelled".into();
            return;
        };
        if path.extension().is_none() {
            path.set_extension(format.extension());
        }
        if format != anvil_io::export::Format::Anvil {
            self.export_format = format;
        }
        self.export_dialog = None;
        if path.exists() {
            self.pending_write = Some((path, format));
        } else {
            self.write_file(path, format);
        }
    }

    fn write_file(&mut self, path: PathBuf, format: anvil_io::export::Format) {
        self.refresh_scene();
        self.status = match anvil_io::export::write(&self.doc, format, &path) {
            Ok(note) => note,
            Err(e) => format!("{} export failed: {e}", format.label()),
        };
        if format == anvil_io::export::Format::Anvil {
            self.file_path = path.display().to_string();
        }
    }

    /// The Export dialog (format list) and the overwrite confirmation.
    fn export_windows(&mut self, ctx: &egui::Context) {
        use anvil_io::export::Format;
        if let Some(mut format) = self.export_dialog {
            let mut open = true;
            let mut choose = false;
            let mut cancel = false;
            egui::Window::new("Export").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
                ui.label("Format:");
                for f in Format::ALL {
                    ui.radio_value(&mut format, f, f.label()).on_hover_text(f.hint());
                }
                ui.separator();
                ui.label(format.hint());
                ui.horizontal(|ui| {
                    if ui.button("Choose file and export").clicked() {
                        choose = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            self.export_dialog = if open && !cancel { Some(format) } else { None };
            if choose {
                self.choose_file(format);
            }
        }
        if let Some((path, format)) = self.pending_write.clone() {
            let mut decided: Option<bool> = None;
            egui::Window::new("Replace file?").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(format!("{} already exists.", path.display()));
                ui.label("Replace it with this export?");
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        decided = Some(true);
                    }
                    if ui.button("Keep the old file").clicked() {
                        decided = Some(false);
                    }
                });
            });
            match decided {
                Some(true) => {
                    self.pending_write = None;
                    self.write_file(path, format);
                }
                Some(false) => {
                    self.pending_write = None;
                    self.status = format!("Kept {}", path.display());
                }
                None => {}
            }
        }
    }

    /// The UI scale and text size window (View > Settings > Settings).
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_window_open {
            return;
        }
        if crate::settings::window(ctx, &mut self.settings_window_open, &mut self.settings) {
            self.settings.apply(ctx);
        }
    }
}

impl eframe::App for AnvilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_ui(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, crate::settings::UiSettings::STORAGE_KEY, &self.settings);
    }
}

impl AnvilApp {
    /// One frame of the whole window.
    fn frame_ui(&mut self, ctx: &egui::Context) {
        let started = std::time::Instant::now();
        self.frame_ui_inner(ctx);
        if self.show_perf {
            self.perf.tick(started.elapsed());
            // Keep the numbers moving while nothing else redraws.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn frame_ui_inner(&mut self, ctx: &egui::Context) {
        self.export_windows(ctx);
        self.settings_window(ctx);
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
                ui.checkbox(&mut self.show_used_sketches, "Used sketches")
                    .on_hover_text("Also show sketches that a feature already uses");
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
                ui.separator();
                // Last, and cut to the space left, so a long message never
                // widens the window.
                ui.add(egui::Label::new(&self.status).truncate());
            });
        });

        // Panels are capped to a share of the window so the 3D view always
        // keeps most of the screen, even on a laptop.
        let screen = ctx.content_rect();
        egui::TopBottomPanel::bottom("expressions")
            .resizable(true)
            .default_height(70.0)
            .max_height((screen.height() * 0.25).max(60.0))
            .show(ctx, |ui| {
                if panels::expression_panel(ui, &mut self.doc, &mut self.panels) {
                    self.invalidate();
                }
            });

        let side_max = (screen.width() * 0.22).max(160.0);
        egui::SidePanel::left("navigator").default_width(230.0f32.min(side_max)).width_range(140.0..=side_max).show(
            ctx,
            |ui| {
                if panels::part_navigator(ui, &mut self.doc, &mut self.panels) {
                    self.invalidate();
                }
            },
        );

        egui::SidePanel::right("properties").default_width(260.0f32.min(side_max)).width_range(160.0..=side_max).show(
            ctx,
            |ui| {
                egui::ScrollArea::vertical().id_salt("properties_scroll").show(ui, |ui| {
                    if panels::property_panel(ui, &mut self.doc, &mut self.panels) {
                        self.invalidate();
                    }
                    if let Some(sel) = self.panels.selected {
                        let takes_edges = self
                            .doc
                            .features
                            .get(sel)
                            .map(|f| f.feature.clone_box().set_edges(Vec::new(), 0))
                            .unwrap_or(false);
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
                                    if ui
                                        .selectable_label(current == *name, format!("{name} ({density} g/cm3)"))
                                        .clicked()
                                    {
                                        self.doc.material.insert(
                                            sel,
                                            anvil_feature::Material { name: name.to_string(), density: *density },
                                        );
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
                        ui.label(format!(
                            "{} entities, {} constraints",
                            ed.sketch.entities.len(),
                            ed.sketch.constraints.len()
                        ));
                        ui.label(format!("Selected: {}", ed.selection.len()));
                        ui.label("Left drag on a point moves it. Right drag pans. Scroll zooms.");
                    }
                });
            },
        );

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| self.viewport(ui));
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Lay out the whole window at a size and return the 3D view.
    fn view_after(app: &mut AnvilApp, w: f32, h: f32, select: Option<usize>) -> egui::Rect {
        view_after_ctx(&egui::Context::default(), app, w, h, select)
    }

    /// Like `view_after`, but on a caller-supplied context (so a test can
    /// apply settings, such as UI scale, to it first).
    fn view_after_ctx(ctx: &egui::Context, app: &mut AnvilApp, w: f32, h: f32, select: Option<usize>) -> egui::Rect {
        app.panels.selected = select;
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h))),
            ..Default::default()
        };
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| app.frame_ui(ctx));
        }
        app.view_rect
    }

    #[test]
    fn box_views_are_orthographic_and_the_panes_start_apart() {
        for v in StandardView::ALL {
            let (_, _, ortho) = v.angles();
            assert_eq!(ortho, v != StandardView::Perspective, "{v:?}");
        }
        let mut app = AnvilApp::new_headless();
        app.run_action(RibbonAction::ToggleQuadView);
        let views: Vec<StandardView> = app.panes.iter().map(|p| p.view).collect();
        assert_eq!(views, PANE_VIEWS.to_vec());
        assert!(app.panes[..3].iter().all(|p| p.camera.is_ortho()));
        assert!(!app.panes[3].camera.is_ortho());
        // A view command acts on the pane under the pointer.
        app.active_pane = 1;
        app.run_action(RibbonAction::ViewTop);
        assert_eq!(app.panes[1].view, StandardView::Top);
        assert!(app.panes[1].camera.is_ortho());
    }

    #[test]
    fn four_panes_split_the_view() {
        let ctx = egui::Context::default();
        let mut app = AnvilApp::new_headless();
        app.run_action(RibbonAction::ToggleQuadView);
        assert!(app.quad);
        assert_eq!(app.panes.len(), 4);
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1366.0, 768.0))),
            ..Default::default()
        };
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| app.frame_ui(ctx));
        }
        // Each pane holds about a quarter of the view.
        let single = view_after(&mut AnvilApp::new_headless(), 1366.0, 768.0, None);
        assert!(app.view_rect.width() < single.width() * 0.6, "{:?}", app.view_rect);
        // The four cameras look from different directions.
        let yaws: Vec<f64> = app.panes.iter().map(|p| p.camera.yaw).collect();
        assert!(yaws.windows(2).any(|w| (w[0] - w[1]).abs() > 0.1), "{yaws:?}");
    }

    #[test]
    fn samples_leave_room_for_the_view_on_a_laptop() {
        let cases = [
            ("start", None),
            // The mold has the longest history and the most expressions.
            ("kettle mold", Some(RibbonAction::KettleMold)),
        ];
        let mut report = String::new();
        let mut bad = false;
        for (name, a) in cases {
            let mut app = AnvilApp::new_headless();
            if let Some(a) = a {
                app.run_action(a);
            }
            let last = app.doc.features.len().saturating_sub(1);
            for (w, h) in [(1280.0, 720.0), (1366.0, 768.0), (1024.0, 640.0)] {
                for sel in [None, Some(0), Some(last)] {
                    let r = view_after(&mut app, w, h, sel);
                    report += &format!(
                        "{name} {w}x{h} sel {sel:?}: view {:.0} x {:.0} at {:?}\n",
                        r.width(),
                        r.height(),
                        r.min
                    );
                    if r.width() < w * 0.45 || r.height() < h * 0.45 {
                        bad = true;
                    }
                }
            }
        }
        println!("{report}");
        assert!(!bad, "{report}");
    }

    /// A 2.0 UI scale and a bigger base text size must not make the ribbon
    /// crowd out the 3D view on a 1366x768 window.
    #[test]
    fn ui_scale_still_leaves_room_for_the_view() {
        let settings = crate::settings::UiSettings { scale: 2.0, base_text: 18.0 };
        let ctx = egui::Context::default();
        let mut app = AnvilApp::new_headless();
        let (w, h) = (1366.0, 768.0);
        // Warm up at the default zoom first, so egui has a known previous
        // frame's screen size before we change the zoom factor (a fresh
        // context has none, and egui's zoom-change logic needs one), then
        // one more pass to let the change settle before measuring.
        let _ = view_after_ctx(&ctx, &mut app, w, h, None);
        app.settings = settings;
        settings.apply(&ctx);
        let _ = view_after_ctx(&ctx, &mut app, w, h, None);
        let r = view_after_ctx(&ctx, &mut app, w, h, None);
        println!("2x scale {w:.0}x{h:.0}: view {:.0} x {:.0}", r.width(), r.height());
        assert!(r.width() >= w * 0.45 && r.height() >= h * 0.45, "view too small at 2x scale: {r:?} in {w}x{h}");
    }
}
