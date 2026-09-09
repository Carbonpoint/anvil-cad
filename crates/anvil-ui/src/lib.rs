//! Anvil desktop UI.
//!
//! Layout (top to bottom, left to right):
//! * **Ribbon**: tabs and groups built from `anvil_feature::descriptors()`.
//! * **Part Navigator** (left): the feature history.
//! * **Viewport** (center): CPU-rendered shaded view of all bodies.
//! * **Properties** (right): generic editor for the selected feature.
//! * **Expressions** (bottom): the named parameter table.
//!
//! The viewport is a pure software renderer drawn through egui shapes. It
//! needs no GPU at all. A glow/wgpu renderer is the planned upgrade for large
//! models (see ADR 0003).

mod app;
mod panels;
mod ribbon;
mod viewport;

pub use app::AnvilApp;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]).with_title("Anvil CAD"),
        ..Default::default()
    };
    eframe::run_native("Anvil CAD", options, Box::new(|cc| Ok(Box::new(AnvilApp::new(cc)))))
}

pub type Result<T> = eframe::Result<T>;
