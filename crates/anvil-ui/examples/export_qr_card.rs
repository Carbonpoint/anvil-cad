//! Make a 3D-printable business card with a name and a QR code.
//!
//! `cargo run --release -p anvil-ui --example export_qr_card -- <out_dir> "Your Name" "https://www.linkedin.com/in/your-handle"`
//!
//! Writes business_card.stl (one colour), business_card.3mf (card, name,
//! and code as separate parts), business_card.anvil, and a preview.
fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| "out".into()));
    let name = args.next().unwrap_or_else(|| "Your Name".into());
    let url = args.next().unwrap_or_else(|| "https://www.linkedin.com/in/your-handle".into());
    std::fs::create_dir_all(&dir).unwrap();
    let doc = anvil_io::business_card(&name, &url);
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            eprintln!("feature {i} ({}): {e}", f.feature.name());
            std::process::exit(1);
        }
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
    let mesh = anvil_io::document_mesh(&doc);
    anvil_io::write_stl(&mesh, &dir.join("business_card.stl")).unwrap();
    let parts = anvil_io::write_3mf(&doc, &dir.join("business_card.3mf")).unwrap();
    anvil_io::save_document(&doc, &dir.join("business_card.anvil")).unwrap();
    let img = anvil_ui::render_top(&doc, 1400, 900);
    let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(dir.join("business_card.ppm"), bytes).unwrap();
    println!("wrote {} ({} triangles, {parts} parts)", dir.display(), mesh.triangle_count());
}
