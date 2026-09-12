//! The Skyline SAR card: a sample that builds a name-and-logo business
//! card. The logo is one sketch, so it exports as a single part and can be
//! printed in a second colour.
//!
//! Logo mark: a scan ring, a mountain skyline inside it, and a quadcopter
//! above the peaks. Every element is a closed loop with no overlaps, so the
//! extrusion is one clean body per loop and the mesh stays manifold.
//! Stroke widths are kept above 1.2 mm so a 0.4 mm nozzle prints them.

use anvil_feature::features::emboss::TextFeature;
use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::Document;
use anvil_math::DVec2;

/// Card size and style.
pub const CARD_W: f64 = 85.6;
pub const CARD_H: f64 = 53.98;
pub const CARD_T: f64 = 0.8;
pub const EMBOSS: f64 = 0.6;
pub const CORNER_R: f64 = 4.0;

/// Logo radius and centre on the card.
pub const LOGO_R: f64 = 15.0;
pub const LOGO_C: DVec2 = DVec2::new(20.0, 27.0);

fn circle_pts(c: DVec2, r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            c + DVec2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

/// A thin lens shape, used for a spinning propeller.
fn ellipse_pts(c: DVec2, rx: f64, ry: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            c + DVec2::new(rx * t.cos(), ry * t.sin())
        })
        .collect()
}

/// Loops of the logo mark, centred on the origin.
///
/// From the outside in: a scan ring, two mountain peaks, and a quadcopter
/// seen from the front with two spinning propellers. Nothing overlaps, so
/// each loop becomes its own clean body.
pub fn logo_loops() -> Vec<Vec<DVec2>> {
    let ring_outer = circle_pts(DVec2::ZERO, LOGO_R, 72);
    let ring_inner = circle_pts(DVec2::ZERO, LOGO_R - 2.2, 72);
    let mountains = vec![
        DVec2::new(-9.8, -6.6),
        DVec2::new(-4.6, 1.2),
        DVec2::new(-1.4, -1.6),
        DVec2::new(2.4, 3.0),
        DVec2::new(9.8, -6.6),
    ];
    // Drone as one loop: camera body, mast, motor bar, and two motor stubs
    // that reach up to the propellers. The stubs stop the bar and props
    // reading as a face.
    let drone = vec![
        DVec2::new(-5.8, 7.4),
        DVec2::new(-1.1, 7.4),
        DVec2::new(-1.1, 6.6),
        DVec2::new(-2.7, 6.6),
        DVec2::new(-2.7, 4.9),
        DVec2::new(-1.8, 4.2),
        DVec2::new(1.8, 4.2),
        DVec2::new(2.7, 4.9),
        DVec2::new(2.7, 6.6),
        DVec2::new(1.1, 6.6),
        DVec2::new(1.1, 7.4),
        DVec2::new(5.8, 7.4),
        DVec2::new(5.8, 8.5),
        DVec2::new(5.2, 8.5),
        DVec2::new(5.2, 9.4),
        DVec2::new(4.0, 9.4),
        DVec2::new(4.0, 8.5),
        DVec2::new(-4.0, 8.5),
        DVec2::new(-4.0, 9.4),
        DVec2::new(-5.2, 9.4),
        DVec2::new(-5.2, 8.5),
        DVec2::new(-5.8, 8.5),
    ];
    // Propellers: thin blades spinning above each motor, kept inside the ring.
    let prop_l = ellipse_pts(DVec2::new(-4.6, 9.9), 2.5, 0.55, 44);
    let prop_r = ellipse_pts(DVec2::new(4.6, 9.9), 2.5, 0.55, 44);
    vec![ring_outer, ring_inner, mountains, drone, prop_l, prop_r]
}

/// Cofounder cards: one per first name.
pub const COFOUNDERS: [&str; 6] = ["Alexander", "Aayan", "Israa", "Jayson", "Lucas", "Joshua"];

/// The Skyline SAR business card.
pub fn skyline_sar_card() -> Document {
    skyline_sar_card_for("")
}

/// The card with a cofounder's first name under the company name. An empty
/// name leaves the card without that line.
pub fn skyline_sar_card_for(name: &str) -> Document {
    let title = if name.is_empty() { "Skyline SAR card".to_string() } else { format!("Skyline SAR card, {name}") };
    let mut doc = Document::new(&title);
    doc.set_expression("card_t", "0.8").ok();
    doc.set_expression("emboss", "0.6").ok();

    // 0: card outline.
    let mut outline = SketchFeature::on_datum("XY");
    outline.sketch.add_rounded_rectangle(0.0, 0.0, CARD_W, CARD_H, [CORNER_R; 4]);
    doc.add_feature(Box::new(outline));
    // 1: card body.
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "card_t".into(), ..Default::default() }));

    // 2: logo sketch on the top face, one sketch so it exports as one part.
    let mut logo = SketchFeature::on_plane(anvil_math::Plane {
        origin: anvil_math::DVec3::new(0.0, 0.0, CARD_T),
        ..anvil_math::Plane::XY
    });
    for lp in logo_loops() {
        let ids: Vec<_> = lp.iter().map(|p| logo.sketch.add_point(p.x + LOGO_C.x, p.y + LOGO_C.y)).collect();
        for i in 0..ids.len() {
            logo.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
        }
    }
    doc.add_feature(Box::new(logo));
    // 3: logo relief.
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 2, distance: "emboss".into(), ..Default::default() }));

    // 4 and 5: the name, in a heavy font that prints with a 0.4 mm nozzle.
    let line = |text: &str, y: f64| TextFeature {
        text: text.into(),
        font_path: "builtin:Archivo Black".into(),
        x: "60".into(),
        y: format!("{y}"),
        z: "card_t".into(),
        size: "7.5".into(),
        height: "emboss".into(),
        center: true,
        ..Default::default()
    };
    let (y_top, y_mid) = if name.is_empty() { (30.0, 15.8) } else { (33.5, 21.5) };
    doc.add_feature(Box::new(TextFeature { tracking: "0.6".into(), ..line("SKYLINE", y_top) }));
    // Wider letter spacing makes the short acronym match the line above.
    doc.add_feature(Box::new(TextFeature { size: "8.5".into(), tracking: "3.0".into(), ..line("SAR", y_mid) }));
    if !name.is_empty() {
        doc.add_feature(Box::new(TextFeature { size: "5.0".into(), tracking: "0.4".into(), ..line(name, 10.5) }));
    }

    doc.appearance.insert(1, [38, 44, 54]);
    for i in [3, 4, 5, 6] {
        doc.appearance.insert(i, [232, 238, 245]);
    }
    doc.material.insert(1, anvil_feature::Material { name: "PLA".into(), density: 1.24 });
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_builds_with_parts_and_printable_strokes() {
        let doc = skyline_sar_card();
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "feature {i} {}: {:?}", f.feature.name(), f.error);
        }
        // Card, logo (4 loops become 3 bodies: ring with hole, mountains,
        // drone), and one body per letter.
        let parts = crate::document_parts(&doc);
        assert_eq!(parts.len(), 4, "card, logo, and two text lines");
        let named = skyline_sar_card_for("Alexander");
        for (i, f) in named.features.iter().enumerate() {
            assert!(f.error.is_none(), "named feature {i}: {:?}", f.error);
        }
        assert_eq!(crate::document_parts(&named).len(), 5, "the name is its own part");
        assert_eq!(doc.features[3].output.as_ref().unwrap().bodies.len(), 5);
        for i in [4, 5] {
            let note = doc.features[i].output.as_ref().unwrap().note.clone().unwrap();
            assert!(note.contains("printable"), "{note}");
        }
        // Everything sits on the card and inside its outline.
        let b = doc.bodies().iter().fold(anvil_math::Aabb::empty(), |mut acc, s| {
            let bb = s.bounds();
            acc.include(bb.min);
            acc.include(bb.max);
            acc
        });
        assert!(b.min.x > -0.01 && b.max.x < CARD_W + 0.01, "{b:?}");
        assert!(b.min.y > -0.01 && b.max.y < CARD_H + 0.01, "{b:?}");
        assert!((b.max.z - (CARD_T + EMBOSS)).abs() < 1e-9);
    }

    #[test]
    fn logo_elements_do_not_touch_each_other() {
        let loops = logo_loops();
        // Ring inner radius against the drone and the peaks.
        let inner = LOGO_R - 2.2;
        for lp in &loops[2..] {
            for p in lp {
                assert!(p.length() < inner - 0.5, "{p:?} is too close to the ring");
            }
        }
        // Highest peak against the drone body.
        let peak_top = loops[2].iter().fold(f64::MIN, |m, p| m.max(p.y));
        let drone_bottom = loops[3].iter().fold(f64::MAX, |m, p| m.min(p.y));
        assert!(drone_bottom - peak_top > 1.0, "{drone_bottom} vs {peak_top}");
    }
}
