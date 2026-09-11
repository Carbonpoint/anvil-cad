//! Render every workbook exercise to PPM and save each as .anvil:
//! `cargo run -p anvil-ui --example render_workbook -- <out_dir>`
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, build) in anvil_io::workbook::EXERCISES {
        let doc = build();
        let stem = name[..2].to_string();
        let img = anvil_ui::render_offscreen(&doc, 900, 600);
        let mut bytes = b"P6 900 600 255\n".to_vec();
        for px in &img.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        std::fs::write(dir.join(format!("wb{stem}.ppm")), bytes).unwrap();
        anvil_io::save_document(&doc, &dir.join(format!("wb{stem}.anvil"))).unwrap();
        let vol: f64 = doc.bodies().iter().map(|b| b.volume()).sum();
        let mass = doc.features.iter().enumerate().filter_map(|(i, _)| doc.mass_of(i)).sum::<f64>();
        println!("{name}: volume {vol:.1} mm3, mass {mass:.2} g, {} bodies", doc.bodies().len());
    }
}
