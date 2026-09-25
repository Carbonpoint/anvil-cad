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
    let cases: [(&str, &str, &str, &str, &str); 6] = [
        ("gyroid", "gyroid", "none", "0", "none"),
        ("octet", "octet", "none", "0", "none"),
        ("kelvin", "kelvin", "none", "0", "none"),
        ("graded", "cubic", "z", "3", "none"),
        ("conformal", "cubic", "none", "0", "cylinder"),
        ("ribs", "honeycomb", "distance", "3", "none"),
    ];
    for (name, kind, grade, wall_end, conform) in cases {
        let mut doc = Document::new(name);
        let t0 = std::time::Instant::now();
        if grade == "distance" {
            // A plate with a boss. Wall runs from 0.8 at the boss to wall_end
            // far from it, so the ribs thicken away from the boss.
            doc.add_feature(Box::new(BoxFeature {
                x: "-30".into(),
                y: "-20".into(),
                z: "0".into(),
                width: "60".into(),
                depth: "40".into(),
                height: "8".into(),
            }));
            doc.add_feature(Box::new(CylinderFeature {
                plane: "XY".into(),
                cx: "18".into(),
                cy: "0".into(),
                radius: "5".into(),
                height: "8".into(),
            }));
            doc.add_feature(Box::new(LatticeFillFeature {
                body: 0,
                lattice: "honeycomb".into(),
                cell: "8".into(),
                wall: "0.8".into(),
                wall_end: wall_end.into(),
                grade: "distance".into(),
                ref_body: 1,
                map_hi: "28".into(),
                skin: "0.8".into(),
                resolution: "0.4".into(),
                ..Default::default()
            }));
            // Cut the top skin away so the ribs show.
            doc.add_feature(Box::new(SplitBodyFeature {
                body: 2,
                plane: "XY".into(),
                offset: "6".into(),
                keep: "below".into(),
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
            write(&out, &format!("lattice_{name}.ppm"), anvil_ui::render_view(&doc, 900, 700, 0.6, 0.9));
            continue;
        }
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
