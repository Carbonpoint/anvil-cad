//! Offscreen renders of the Aircraft feature, plain and optimised:
//! `cargo run -p anvil-ui --example render_aircraft -- out_dir`
use anvil_feature::features::aircraft::AircraftFeature;
use anvil_feature::Document;

fn write(out: &std::path::Path, name: &str, img: egui::ColorImage) {
    let [w, h] = img.size;
    let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(out.join(name), bytes).unwrap();
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    for (name, optimize) in [("plain", false), ("optimised", true)] {
        let t0 = std::time::Instant::now();
        let mut doc = Document::new(name);
        doc.add_feature(Box::new(AircraftFeature { optimize, ..Default::default() }));
        let f = &doc.features[0];
        if let Some(e) = &f.error {
            eprintln!("{name}: {e}");
        }
        if let Some(n) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            eprintln!("{name}: {n}");
        }
        eprintln!("{name}: built in {:.1} s", t0.elapsed().as_secs_f64());
        write(&out, &format!("aircraft_{name}.ppm"), anvil_ui::render_view(&doc, 1200, 800, -0.7, 0.45));
        write(&out, &format!("aircraft_{name}_top.ppm"), anvil_ui::render_view(&doc, 1200, 800, 0.0, 1.45));
        anvil_io::export::write(&doc, anvil_io::export::Format::ThreeMf, &out.join(format!("aircraft_{name}.3mf")))
            .unwrap();
    }
}
