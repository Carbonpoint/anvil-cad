//! Render and export the paper towel stand:
//! `cargo run --release -p anvil-ui --example export_towel_stand -- <out_dir>`
fn write_ppm(path: &std::path::Path, img: &egui::ColorImage) {
    let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(path, bytes).unwrap();
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = anvil_io::towel_stand::towel_stand();
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            panic!("feature {i} {}: {e}", f.feature.name());
        }
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
    write_ppm(&dir.join("towel_top.ppm"), &anvil_ui::render_top(&doc, 1200, 1200));
    write_ppm(&dir.join("towel_iso.ppm"), &anvil_ui::render_view(&doc, 1400, 900, -2.15, 0.62));
    // Underside: look up from below.
    write_ppm(&dir.join("towel_bottom.ppm"), &anvil_ui::render_view(&doc, 1400, 900, -2.15, -0.75));
    let mesh = anvil_io::document_mesh(&doc);
    anvil_io::write_stl(&mesh, &dir.join("towel_stand.stl")).unwrap();
    let parts = anvil_io::write_3mf(&doc, &dir.join("towel_stand.3mf")).unwrap();
    anvil_io::save_document(&doc, &dir.join("towel_stand.anvil")).unwrap();
    let vol: f64 = doc.bodies().iter().map(|b| b.volume()).sum();
    let b = doc.bodies().iter().fold(anvil_math::Aabb::empty(), |mut acc, s| {
        let bb = s.bounds();
        acc.include(bb.min);
        acc.include(bb.max);
        acc
    });
    let d = b.max - b.min;
    println!(
        "bodies {} volume {:.1} cm3 size {:.1} x {:.1} x {:.2} mm triangles {} 3mf parts {parts}",
        doc.bodies().len(),
        vol / 1000.0,
        d.x,
        d.y,
        d.z,
        mesh.triangle_count()
    );
}
