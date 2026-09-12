//! Render one large letter and report its mesh health, to separate
//! modelling faults from rendering faults.
//! `cargo run -p anvil-ui --example render_text_check -- <out_dir>`
use anvil_feature::features::emboss::TextFeature;
use anvil_feature::Document;

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut doc = Document::new("R");
    doc.add_feature(Box::new(TextFeature {
        text: "RAOB8".into(),
        font_path: "builtin:Archivo Black".into(),
        size: "20".into(),
        height: "2".into(),
        center: true,
        ..Default::default()
    }));
    for (i, b) in doc.bodies().iter().enumerate() {
        println!("body {i}: volume {:.3} faces {} open/over {:?}", b.volume(), b.faces.len(), b.open_edge_report());
    }
    let img = anvil_ui::render_top(&doc, 1200, 500);
    let mut bytes = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(dir.join("letters.ppm"), bytes).unwrap();
}
