use crate::panels::{self, PanelState};
use crate::ribbon::{build_ribbon, ButtonKind, RibbonAction, RibbonTab};
use crate::viewport::{self, Camera};
use anvil_feature::features::{extrude::ExtrudeFeature, revolve::RevolveFeature, sketch::SketchFeature};
use anvil_feature::{descriptor, Document};
use anvil_kernel::TriMesh;
use std::path::PathBuf;

pub struct AnvilApp {
    doc: Document,
    ribbon: Vec<RibbonTab>,
    active_tab: usize,
    panels: PanelState,
    camera: Camera,
    mesh: TriMesh,
    mesh_dirty: bool,
    wireframe: bool,
    file_path: String,
    status: String,
    last_tris: usize,
}

impl AnvilApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = AnvilApp {
            doc: Document::new("Untitled"),
            ribbon: build_ribbon(),
            active_tab: 0,
            panels: PanelState::default(),
            camera: Camera::default(),
            mesh: TriMesh::default(),
            mesh_dirty: true,
            wireframe: true,
            file_path: "part.anvil".into(),
            status: "Ready".into(),
            last_tris: 0,
        };
        app.active_tab = app.ribbon.iter().position(|t| t.name == "Home").unwrap_or(0);
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
        // A ring: rectangle offset from the Y axis on the XZ plane, revolved.
        let mut ring = SketchFeature::rectangle("XZ", 8.0, 6.0);
        for e in ring.sketch.entities.values_mut() {
            if let anvil_sketch::Entity::Point { pos, .. } = e {
                pos.x += 45.0;
                pos.y += 25.0;
            }
        }
        doc.add_feature(Box::new(ring));
        doc.add_feature(Box::new(RevolveFeature { sketch: 2, axis: "Y".into(), angle_deg: "270".into() }));
        self.doc = doc;
        self.mesh_dirty = true;
        self.fit_after_mesh();
        self.status = "Demo part loaded".into();
    }

    fn fit_after_mesh(&mut self) {
        self.refresh_mesh();
        let mut b = anvil_math::Aabb::empty();
        for p in &self.mesh.positions {
            b.include(*p);
        }
        self.camera.fit(&b);
    }

    fn refresh_mesh(&mut self) {
        if self.mesh_dirty {
            self.mesh = anvil_io::document_mesh(&self.doc);
            self.mesh_dirty = false;
        }
    }

    fn run_action(&mut self, a: RibbonAction) {
        match a {
            RibbonAction::Undo => {
                self.doc.undo();
                self.mesh_dirty = true;
            }
            RibbonAction::Redo => {
                self.doc.redo();
                self.mesh_dirty = true;
            }
            RibbonAction::NewDocument => {
                self.doc = Document::new("Untitled");
                self.panels = PanelState::default();
                self.mesh_dirty = true;
                self.status = "New document".into();
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
                        self.mesh_dirty = true;
                        self.fit_after_mesh();
                        self.status = format!("Loaded {}", p.display());
                    }
                    Err(e) => self.status = format!("Load failed: {e}"),
                }
            }
            RibbonAction::ExportStl => {
                self.refresh_mesh();
                let p = PathBuf::from(&self.file_path).with_extension("stl");
                self.status = match anvil_io::write_stl(&self.mesh, &p) {
                    Ok(()) => format!("Wrote {} ({} triangles)", p.display(), self.mesh.triangle_count()),
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
            RibbonAction::FitView => self.fit_after_mesh(),
            RibbonAction::DemoPart => self.load_demo(),
        }
    }

    fn add_feature_by_id(&mut self, id: &str) {
        if let Some(d) = descriptor(id) {
            let idx = self.doc.add_feature((d.create)());
            self.panels.selected = Some(idx);
            self.mesh_dirty = true;
            self.status = format!("Added {}", d.label);
        }
    }

    fn ribbon_ui(&mut self, ui: &mut egui::Ui) {
        let mut clicked_feature: Option<&'static str> = None;
        let mut clicked_action: Option<RibbonAction> = None;
        ui.horizontal(|ui| {
            for (i, t) in self.ribbon.iter().enumerate() {
                if ui.selectable_label(self.active_tab == i, t.name).clicked() {
                    self.active_tab = i;
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if let Some(tab) = self.ribbon.get(self.active_tab) {
                for g in &tab.groups {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for b in &g.buttons {
                                let btn = ui.add(egui::Button::new(b.label).min_size(egui::vec2(64.0, 40.0)));
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
}

impl eframe::App for AnvilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keyboard shortcuts.
        let (undo, redo) = ctx.input(|i| {
            (i.modifiers.command && i.key_pressed(egui::Key::Z), i.modifiers.command && i.key_pressed(egui::Key::Y))
        });
        if undo {
            self.run_action(RibbonAction::Undo);
        }
        if redo {
            self.run_action(RibbonAction::Redo);
        }

        egui::TopBottomPanel::top("ribbon").show(ctx, |ui| self.ribbon_ui(ui));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.separator();
                ui.label("File:");
                ui.add(egui::TextEdit::singleline(&mut self.file_path).desired_width(220.0));
                ui.separator();
                ui.checkbox(&mut self.wireframe, "Edges");
                ui.separator();
                ui.label(format!("{} tris", self.last_tris));
            });
        });

        egui::TopBottomPanel::bottom("expressions").resizable(true).show(ctx, |ui| {
            if panels::expression_panel(ui, &mut self.doc, &mut self.panels) {
                self.mesh_dirty = true;
            }
        });

        egui::SidePanel::left("navigator").default_width(240.0).show(ctx, |ui| {
            if panels::part_navigator(ui, &mut self.doc, &mut self.panels) {
                self.mesh_dirty = true;
            }
        });

        egui::SidePanel::right("properties").default_width(260.0).show(ctx, |ui| {
            if panels::property_panel(ui, &mut self.doc, &mut self.panels) {
                self.mesh_dirty = true;
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.refresh_mesh();
            let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let rect = resp.rect;
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(235, 238, 242));
            if resp.dragged_by(egui::PointerButton::Primary) {
                let d = resp.drag_delta();
                self.camera.orbit(d.x as f64, d.y as f64);
            }
            if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
                let d = resp.drag_delta();
                self.camera.pan(d.x as f64, d.y as f64);
            }
            if resp.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    self.camera.zoom((-scroll as f64 * 0.002).exp());
                }
            }
            let stats = viewport::draw(&painter, rect, &self.camera, &self.mesh, self.wireframe);
            self.last_tris = stats.triangles_drawn;
        });
    }
}
