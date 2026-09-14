//! Export one logo business card per name, as STL and 3MF, with previews.
//!
//! `cargo run --release -p anvil-ui --example export_logo_cards -- <out_dir> [names...]`
//!
//! With no names, placeholder names are used.
fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| "cards".into()));
    let mut names: Vec<String> = args.collect();
    if names.is_empty() {
        names = anvil_io::logo_card::SAMPLE_NAMES.iter().map(|s| s.to_string()).collect();
    }
    std::fs::create_dir_all(&dir).unwrap();
    for name in &names {
        let doc = anvil_io::logo_card::skyline_sar_card_for(name);
        for (i, f) in doc.features.iter().enumerate() {
            if let Some(e) = &f.error {
                panic!("{name}: feature {i}: {e}");
            }
        }
        let stem: String =
            name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let mesh = anvil_io::document_mesh(&doc);
        anvil_io::write_stl(&mesh, &dir.join(format!("card_{stem}.stl"))).unwrap();
        let parts = anvil_io::write_3mf(&doc, &dir.join(format!("card_{stem}.3mf"))).unwrap();
        anvil_io::save_document(&doc, &dir.join(format!("card_{stem}.anvil"))).unwrap();
        let img = anvil_ui::render_top(&doc, 1100, 700);
        let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
        for px in &img.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        std::fs::write(dir.join(format!("card_{stem}.ppm")), bytes).unwrap();
        println!("{name}: {} triangles, {parts} parts", mesh.triangle_count());
    }
}
