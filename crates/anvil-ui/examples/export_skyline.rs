//! Render and export the Skyline SAR card:
//! `cargo run -p anvil-ui --example export_skyline -- <out_dir>`
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = anvil_io::logo::skyline_sar_card();
    for (name, img) in
        [("top", anvil_ui::render_top(&doc, 1400, 900)), ("iso", anvil_ui::render_view(&doc, 1400, 900, -2.15, 0.62))]
    {
        let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
        for px in &img.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        std::fs::write(dir.join(format!("skyline_{name}.ppm")), bytes).unwrap();
    }
    let stl = dir.join("skyline_sar_card.stl");
    let mesh = anvil_io::document_mesh(&doc);
    anvil_io::write_stl(&mesh, &stl).unwrap();
    let n = anvil_io::write_3mf(&doc, &dir.join("skyline_sar_card.3mf")).unwrap();
    let parts = anvil_io::write_stl_parts(&doc, &stl).unwrap();
    anvil_io::save_document(&doc, &dir.join("skyline_sar_card.anvil")).unwrap();
    let vol: f64 = doc.bodies().iter().map(|b| b.volume()).sum();
    println!(
        "bodies {} volume {vol:.1} mm3 triangles {} 3mf objects {n} stl parts {}",
        doc.bodies().len(),
        mesh.triangle_count(),
        parts.len()
    );
    for f in doc.features.iter() {
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
}
