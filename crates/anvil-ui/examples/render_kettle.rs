//! Offscreen renders of the kettle sample:
//! `cargo run -p anvil-ui --example render_kettle -- out_dir`
//! Writes kettle.ppm (three quarter view), kettle_side.ppm, kettle.stl,
//! kettle.3mf, and kettle.anvil.
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let t0 = std::time::Instant::now();
    let doc = anvil_io::kettle::kettle();
    eprintln!("built in {:.1} s", t0.elapsed().as_secs_f64());
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            eprintln!("feature {i} {}: {e}", f.feature.name());
        }
    }
    for b in doc.bodies() {
        eprintln!("body: {} faces, volume {:.0} mm3, open edges {:?}", b.faces.len(), b.volume(), b.open_edge_report());
    }
    let write = |name: &str, img: egui::ColorImage| {
        let [w, h] = img.size;
        let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
        for px in &img.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        std::fs::write(out.join(name), bytes).unwrap();
    };
    let t1 = std::time::Instant::now();
    write("kettle.ppm", anvil_ui::render_view(&doc, 1400, 1000, -0.6, 0.45));
    eprintln!("rendered in {:.2} s", t1.elapsed().as_secs_f64());
    write("kettle_side.ppm", anvil_ui::render_view(&doc, 1400, 1000, std::f64::consts::FRAC_PI_2, 0.05));
    let gated = anvil_io::kettle::kettle_gated();
    write("kettle_gated.ppm", anvil_ui::render_view(&gated, 1400, 1000, 2.6, 0.35));
    anvil_io::write_3mf(&doc, &out.join("kettle.3mf")).unwrap();
    anvil_io::write_stl_parts(&doc, &out.join("kettle.stl")).unwrap();
    anvil_io::save_document(&doc, &out.join("kettle.anvil")).unwrap();
}
