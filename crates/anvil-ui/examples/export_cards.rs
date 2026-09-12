//! Export one Skyline SAR card per cofounder, with previews.
//! `cargo run -p anvil-ui --example export_cards -- <out_dir>`
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in anvil_io::logo::COFOUNDERS {
        let doc = anvil_io::logo::skyline_sar_card_for(name);
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "{name} feature {i}: {:?}", f.error);
        }
        let stem = name.to_lowercase();
        let stl = dir.join(format!("skyline_sar_{stem}.stl"));
        let mesh = anvil_io::document_mesh(&doc);
        anvil_io::write_stl(&mesh, &stl).unwrap();
        let objects = anvil_io::write_3mf(&doc, &dir.join(format!("skyline_sar_{stem}.3mf"))).unwrap();
        anvil_io::save_document(&doc, &dir.join(format!("skyline_sar_{stem}.anvil"))).unwrap();
        let img = anvil_ui::render_top(&doc, 1100, 700);
        let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
        for px in &img.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        std::fs::write(dir.join(format!("skyline_sar_{stem}.ppm")), bytes).unwrap();
        println!(
            "{name}: {} triangles, {objects} parts, {:.1} mm3",
            mesh.triangle_count(),
            doc.bodies().iter().map(|b| b.volume()).sum::<f64>()
        );
    }
}
