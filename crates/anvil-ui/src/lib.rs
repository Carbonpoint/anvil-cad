//! Anvil desktop UI.
//!
//! Layout (top to bottom, left to right):
//! * **Ribbon**: tabs and groups built from `anvil_feature::descriptors()`,
//!   plus a contextual Sketch tab while a sketch is open.
//! * **Part Navigator** (left): the feature history.
//! * **Viewport** (center): CPU-rendered shaded view with a depth buffer.
//! * **Properties** (right): generic editor for the selected feature.
//! * **Expressions** (bottom): the named parameter table.
//!
//! The viewport is a pure software renderer (`raster.rs`). It needs no GPU.

mod app;
mod camera;
mod dxf;
mod panels;
mod raster;
mod ribbon;
mod scene;
mod sketch_editor;

pub use app::AnvilApp;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 950.0]).with_title("Anvil CAD"),
        ..Default::default()
    };
    eframe::run_native("Anvil CAD", options, Box::new(|cc| Ok(Box::new(AnvilApp::new(cc)))))
}

pub type Result<T> = eframe::Result<T>;

/// Render a document to an image without a window. Used by tests and the
/// `render_demo` example.
pub fn render_offscreen(doc: &anvil_feature::Document, width: usize, height: usize) -> egui::ColorImage {
    let scene = scene::Scene::build(doc);
    let mut cam = camera::Camera::default();
    cam.fit(&scene.bounds);
    let proj = camera::Projector::new(&cam, width as f64, height as f64);
    let mut fb = raster::Framebuffer::new(width, height);
    let style = raster::Style::default();
    fb.clear(style.background);
    fb.draw_scene(&scene, &proj, &style, None, None);
    fb.to_image()
}

/// Render a document looking straight down the Z axis, framed to fit.
pub fn render_top(doc: &anvil_feature::Document, width: usize, height: usize) -> egui::ColorImage {
    let scene = scene::Scene::build(doc);
    let mut cam = camera::Camera::default();
    let plane = anvil_math::Plane { origin: scene.bounds.center(), ..anvil_math::Plane::XY };
    let span = (scene.bounds.max - scene.bounds.min).max_element().max(1.0);
    cam.look_at_plane(&plane, span * 0.75);
    let proj = camera::Projector::new(&cam, width as f64, height as f64);
    let mut fb = raster::Framebuffer::new(width, height);
    let style = raster::Style::default();
    fb.clear(style.background);
    fb.draw_scene(&scene, &proj, &style, None, None);
    fb.to_image()
}
