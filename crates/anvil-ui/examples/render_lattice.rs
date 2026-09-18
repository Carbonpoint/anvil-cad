//! Offscreen renders of Lattice fill on a box, one image per lattice
//! kind, plus a graded and a conformal example:
//! `cargo run -p anvil-ui --example render_lattice -- out_dir`
use anvil_feature::features::lattice::LatticeFillFeature;
use anvil_feature::features::primitives::{BoxFeature, CylinderFeature};
use anvil_feature::features::solid_extra::SplitBodyFeature;
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
    let cases: [(&str, &str, &str, &str, &str); 5] = [
        ("gyroid", "gyroid", "none", "0", "none"),
        ("octet", "octet", "none", "0", "none"),
        ("kelvin", "kelvin", "none", "0", "none"),
        ("graded", "cubic", "z", "3", "none"),
        ("conformal", "cubic", "none", "0", "cylinder"),
    ];
    for (name, kind, grade, wall_end, conform) in cases {
        let mut doc = Document::new(name);
        let t0 = std::time::Instant::now();
        if conform == "cylinder" {
            doc.add_feature(Box::new(CylinderFeature {
                plane: "XY".into(),
                cx: "0".into(),
                cy: "0".into(),
                radius: "20".into(),
                height: "30".into(),
            }));
        } else {
            doc.add_feature(Box::new(BoxFeature {
                x: "-20".into(),
                y: "-20".into(),
                z: "0".into(),
                width: "40".into(),
                depth: "40".into(),
                height: "30".into(),
            }));
        }
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: kind.into(),
            cell: "10".into(),
            wall: "1.2".into(),
            wall_end: wall_end.into(),
            grade: grade.into(),
            conform: conform.into(),
            skin: "1.0".into(),
            resolution: "0.4".into(),
            ..Default::default()
        }));
        // Cut the front half away so the core shows.
        doc.add_feature(Box::new(SplitBodyFeature {
            body: 1,
            plane: "XZ".into(),
            offset: "0".into(),
            keep: "above".into(),
            ..Default::default()
        }));
        for (i, f) in doc.features.iter().enumerate() {
            if let Some(e) = &f.error {
                eprintln!("{name} feature {i}: {e}");
            }
            if let Some(n) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
                eprintln!("{name}: {n}");
            }
        }
        eprintln!("{name}: built in {:.1} s", t0.elapsed().as_secs_f64());
        write(&out, &format!("lattice_{name}.ppm"), anvil_ui::render_view(&doc, 900, 700, 2.3, 0.5));
    }
}
