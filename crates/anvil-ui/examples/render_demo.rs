//! Offscreen render of the demo part to a PPM file. Used to check the
//! software renderer without a window: `cargo run -p anvil-ui --example render_demo -- out.ppm`
use anvil_feature::features::{extrude::ExtrudeFeature, revolve::RevolveFeature, sketch::SketchFeature};
use anvil_feature::Document;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "render.ppm".into());
    let mut doc = Document::new("Demo");
    doc.add_feature(Box::new(SketchFeature::rectangle("XY", 60.0, 30.0)));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "12".into(), symmetric: false }));
    let mut ring = SketchFeature::on_datum("XZ");
    ring.sketch.add_rectangle(41.0, 22.0, 49.0, 28.0);
    doc.add_feature(Box::new(ring));
    doc.add_feature(Box::new(RevolveFeature { sketch: 2, axis: "Y".into(), angle_deg: "270".into() }));
    let (w, h) = (900usize, 600usize);
    let img = anvil_ui::render_offscreen(&doc, w, h);
    let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(&out, bytes).unwrap();
    println!("wrote {out}");
}
