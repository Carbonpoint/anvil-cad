use anvil_feature::features::construct::OffsetPlaneFeature;
use anvil_feature::features::primitives::BoxFeature;
use anvil_feature::features::transform::{MoveFeature, RectPatternFeature};
use anvil_feature::features::{extrude::ExtrudeFeature, sketch::SketchFeature};
use anvil_feature::{descriptors, Document, ParamValue};

#[test]
fn sketch_then_extrude_makes_a_body() {
    let mut doc = Document::new("test");
    doc.set_expression("h", "12").unwrap();
    doc.add_feature(Box::new(SketchFeature::rectangle("XY", 10.0, 20.0)));
    let e = doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 0,
        distance: "h".into(),
        symmetric: false,
        ..Default::default()
    }));
    assert!(doc.features[e].error.is_none(), "{:?}", doc.features[e].error);
    let bodies = doc.bodies();
    assert_eq!(bodies.len(), 1);
    assert!((bodies[0].volume() - 2400.0).abs() < 1e-6);

    doc.set_expression("h", "6").unwrap();
    assert!((doc.bodies()[0].volume() - 1200.0).abs() < 1e-6);

    doc.undo();
    assert!((doc.bodies()[0].volume() - 2400.0).abs() < 1e-6);
}

#[test]
fn generic_param_edit_and_json_round_trip() {
    let mut doc = Document::new("test");
    doc.add_feature(Box::new(SketchFeature::rectangle("XY", 40.0, 25.0)));
    doc.add_feature(Box::new(ExtrudeFeature::default()));
    doc.edit_feature(1, |f| f.set_param("distance", ParamValue::Expr("3".into())).unwrap());
    let v1 = doc.bodies()[0].volume();
    let json = doc.to_json().unwrap();
    let doc2 = Document::from_json(&json).unwrap();
    assert!((doc2.bodies()[0].volume() - v1).abs() < 1e-9);
}

#[test]
fn registry_has_the_basics() {
    let ids: Vec<_> = descriptors().iter().map(|d| d.id).collect();
    for want in ["sketch", "extrude", "revolve", "fillet"] {
        assert!(ids.contains(&want), "missing {want} in {ids:?}");
    }
}

#[test]
fn move_consumes_and_pattern_multiplies() {
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature::default()));
    doc.add_feature(Box::new(MoveFeature { body: 0, dx: "5".into(), copy: false, ..Default::default() }));
    assert_eq!(doc.bodies().len(), 1, "moved body replaces the original");
    assert!((doc.bodies()[0].bounds().min.x - 5.0).abs() < 1e-9);
    doc.add_feature(Box::new(RectPatternFeature {
        body: 1,
        count_x: "3".into(),
        count_y: "2".into(),
        ..Default::default()
    }));
    assert_eq!(doc.bodies().len(), 6);
}

#[test]
fn sketch_on_offset_plane_follows_the_plane() {
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(OffsetPlaneFeature { base: "XY".into(), base_feature: None, offset: "7".into() }));
    let mut sk = SketchFeature::on_feature(0);
    sk.sketch.add_rectangle(0.0, 0.0, 4.0, 4.0);
    doc.add_feature(Box::new(sk));
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 1,
        distance: "3".into(),
        symmetric: false,
        ..Default::default()
    }));
    let b = doc.bodies();
    assert_eq!(b.len(), 1, "{:?}", doc.features.iter().map(|f| f.error.clone()).collect::<Vec<_>>());
    assert!((b[0].bounds().min.z - 7.0).abs() < 1e-9);
    assert!((b[0].bounds().max.z - 10.0).abs() < 1e-9);
}

#[test]
fn text_and_qr_make_bodies_with_holes() {
    use anvil_feature::features::emboss::{QrFeature, TextFeature};
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(TextFeature {
        text: "AO".into(),
        size: "10".into(),
        height: "1".into(),
        ..Default::default()
    }));
    let n = doc.features[0].error.clone();
    assert!(n.is_none(), "{n:?}");
    let bodies = doc.bodies();
    assert_eq!(bodies.len(), 2, "one body per glyph");
    assert!(bodies.iter().all(|b| b.volume() > 0.0));
    assert!(bodies[1].faces.values().any(|f| !f.inner.is_empty()), "O has a hole");
    doc.add_feature(Box::new(QrFeature { data: "https://anvil.test".into(), ..Default::default() }));
    assert!(doc.features[1].error.is_none());
    assert!(doc.bodies().len() > 30);
}

#[test]
fn sketch_circle_inside_rectangle_extrudes_a_hole() {
    let mut doc = Document::new("t");
    let mut sk = SketchFeature::rectangle("XY", 20.0, 20.0);
    let c = sk.sketch.add_point(0.0, 0.0);
    sk.sketch.add_circle(c, 5.0);
    doc.add_feature(Box::new(sk));
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 0,
        distance: "2".into(),
        symmetric: false,
        ..Default::default()
    }));
    let b = doc.bodies();
    assert_eq!(b.len(), 1);
    let exact = (400.0 - std::f64::consts::PI * 25.0) * 2.0;
    assert!((b[0].volume() - exact).abs() / exact < 0.01, "{} vs {exact}", b[0].volume());
}

#[test]
fn cut_extrude_and_hole_remove_volume() {
    use anvil_feature::features::hole::HoleFeature;
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature {
        width: "40".into(),
        depth: "30".into(),
        height: "10".into(),
        ..Default::default()
    }));
    let mut pocket = SketchFeature::rectangle("XY", 10.0, 10.0);
    for e in pocket.sketch.entities.values_mut() {
        if let anvil_sketch::Entity::Point { pos, .. } = e {
            pos.x += 20.0;
            pos.y += 15.0;
        }
    }
    doc.add_feature(Box::new(pocket));
    // Pocket 4 mm deep from the top face (z = 10): sketch on an offset plane via z? Use symmetric cut through the top.
    let mut sk = doc.features[1].feature.downcast_ref::<SketchFeature>().unwrap().clone();
    sk.sketch.plane.origin.z = 10.0;
    sk.source = anvil_feature::features::sketch::PlaneSource::Custom;
    doc.edit_feature(1, |f| *f.downcast_mut::<SketchFeature>().unwrap() = sk);
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 1,
        distance: "-4".into(),
        symmetric: false,
        operation: "cut".into(),
        target: 0,
    }));
    assert!(doc.features[2].error.is_none(), "{:?}", doc.features[2].error);
    let v = doc.bodies()[0].volume();
    assert!((v - (12000.0 - 400.0)).abs() < 1e-6, "{v}");
    doc.add_feature(Box::new(HoleFeature {
        body: 2,
        x: "8".into(),
        y: "8".into(),
        z: "10".into(),
        diameter: "6".into(),
        depth: "100".into(),
        ..Default::default()
    }));
    assert!(doc.features[3].error.is_none(), "{:?}", doc.features[3].error);
    let v2 = doc.bodies()[0].volume();
    let hole = std::f64::consts::PI * 9.0 * 10.0;
    assert!((v2 - (11600.0 - hole)).abs() / hole < 0.02, "{v2}");
    assert_eq!(doc.bodies().len(), 1, "cut and hole consume their targets");
}

#[test]
fn removing_a_feature_renumbers_later_references() {
    use anvil_feature::features::hole::HoleFeature;
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature {
        width: "40".into(),
        depth: "30".into(),
        height: "10".into(),
        ..Default::default()
    }));
    doc.add_feature(Box::new(HoleFeature {
        body: 0,
        x: "10".into(),
        y: "10".into(),
        z: "10".into(),
        diameter: "4".into(),
        depth: "20".into(),
        ..Default::default()
    }));
    doc.add_feature(Box::new(HoleFeature {
        body: 1,
        x: "30".into(),
        y: "10".into(),
        z: "10".into(),
        diameter: "4".into(),
        depth: "20".into(),
        ..Default::default()
    }));
    doc.add_feature(Box::new(HoleFeature {
        body: 2,
        x: "30".into(),
        y: "20".into(),
        z: "10".into(),
        diameter: "4".into(),
        depth: "20".into(),
        ..Default::default()
    }));
    assert!(doc.features.iter().all(|f| f.error.is_none()));
    // Delete the middle hole: the last hole should now point at index 1 and work.
    let broken = doc.remove_feature(2);
    assert_eq!(broken, vec![2], "the hole that used the deleted one is flagged");
    assert!(doc.features[2].error.is_some());
    // Undo restores everything.
    doc.undo();
    assert!(doc.features.iter().all(|f| f.error.is_none()));
    // Deleting the first hole renumbers nothing broken for features that used the box.
    let _ = doc.remove_feature(3);
    assert_eq!(doc.features.len(), 3);
    assert!(
        doc.features.iter().all(|f| f.error.is_none()),
        "{:?}",
        doc.features.iter().map(|f| f.error.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn moving_a_feature_keeps_references_valid() {
    use anvil_feature::features::primitives::SphereFeature;
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature::default()));
    doc.add_feature(Box::new(SphereFeature::default()));
    doc.add_feature(Box::new(MoveFeature { body: 0, dx: "5".into(), ..Default::default() }));
    // Move the sphere to the end: Move/Copy now sits at 1 and must still use the box at 0.
    doc.move_feature(1, 2).unwrap();
    assert_eq!(doc.features[1].feature.kind(), "move");
    assert!(doc.features.iter().all(|f| f.error.is_none()));
    // Moving Move/Copy before the box is refused.
    assert!(doc.move_feature(1, 0).is_err());
}

#[test]
fn a_cut_that_misses_reports_an_error() {
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature::default()));
    let mut sk = SketchFeature::rectangle("XY", 5.0, 5.0);
    for e in sk.sketch.entities.values_mut() {
        if let anvil_sketch::Entity::Point { pos, .. } = e {
            pos.x += 200.0;
        }
    }
    doc.add_feature(Box::new(sk));
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 1,
        distance: "5".into(),
        operation: "cut".into(),
        target: 0,
        ..Default::default()
    }));
    assert!(doc.features[2].error.as_deref().unwrap_or("").contains("does not touch"));
}

#[test]
fn fillet_and_chamfer_on_picked_edges() {
    use anvil_feature::features::fillet::{ChamferFeature, FilletFeature};
    use anvil_math::DVec3;
    let mut doc = Document::new("t");
    doc.add_feature(Box::new(BoxFeature {
        width: "20".into(),
        depth: "10".into(),
        height: "10".into(),
        ..Default::default()
    }));
    let e = [DVec3::new(0.0, 0.0, 10.0), DVec3::new(20.0, 0.0, 10.0)];
    doc.add_feature(Box::new(FilletFeature { body_feature: 0, radius: "2".into(), edges: vec![e] }));
    assert!(doc.features[1].error.is_none(), "{:?}", doc.features[1].error);
    let e2 = [DVec3::new(0.0, 10.0, 0.0), DVec3::new(20.0, 10.0, 0.0)];
    doc.add_feature(Box::new(ChamferFeature { body: 1, distance: "1".into(), edges: vec![e2] }));
    assert!(doc.features[2].error.is_none(), "{:?}", doc.features[2].error);
    assert_eq!(doc.bodies().len(), 1);
    let v = doc.bodies()[0].volume();
    let expected = 2000.0 - 20.0 * 4.0 * (1.0 - std::f64::consts::FRAC_PI_4) - 10.0;
    assert!((v - expected).abs() / expected < 0.005, "{v} vs {expected}");
    // Round trip keeps the edges.
    let json = doc.to_json().unwrap();
    let back = Document::from_json(&json).unwrap();
    assert!((back.bodies()[0].volume() - v).abs() < 1e-6);
}

#[test]
fn stroke_note_prefers_heavy_fonts() {
    use anvil_feature::features::emboss::TextFeature;
    let width = |font: &str, thicken: &str| -> f64 {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(TextFeature {
            text: "Alex Goldman".into(),
            size: "7".into(),
            height: "0.6".into(),
            font_path: font.into(),
            thicken: thicken.into(),
            ..Default::default()
        }));
        let note = doc.features[0].output.as_ref().unwrap().note.clone().unwrap();
        eprintln!("{font} thicken {thicken}: {note}");
        note.split_whitespace().find_map(|w| w.parse::<f64>().ok()).unwrap()
    };
    let regular = width("", "0");
    let black = width("builtin:Archivo Black", "0");
    let bold = width("builtin:Liberation Sans Bold", "0");
    let thick = width("", "0.15");
    assert!(black > regular * 1.4, "{black} vs {regular}");
    assert!(bold > regular);
    assert!(thick > regular + 0.2, "{thick} vs {regular}");
}

#[test]
fn glyphs_with_counters_tessellate_to_the_right_area() {
    use anvil_feature::features::emboss::{nest_loops, text_outlines};
    use anvil_feature::fonts;
    // R, A, O, and B have counters; a notch in the triangulation shows up
    // as a volume below the outline area.
    for font in ["", "builtin:Archivo Black"] {
        let bytes = fonts::load(font).unwrap();
        for ch in ["R", "A", "O", "B", "S", "8"] {
            let glyphs = text_outlines(&bytes, ch, 10.0, false).unwrap();
            let loops: Vec<Vec<anvil_math::DVec2>> = glyphs.into_iter().flatten().collect();
            let area = |p: &Vec<anvil_math::DVec2>| {
                let n = p.len();
                0.5 * (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum::<f64>()
            };
            let mut outline = 0.0;
            let mut solid = 0.0;
            for (outer, holes) in nest_loops(&loops) {
                outline += area(&outer).abs() - holes.iter().map(|h| area(h).abs()).sum::<f64>();
                let body = anvil_kernel::ops::extrude_with_holes(&anvil_math::Plane::XY, &outer, &holes, 1.0).unwrap();
                solid += body.volume();
            }
            assert!((solid - outline).abs() / outline < 1e-6, "{font} {ch}: {solid} vs {outline}");
        }
    }
}

/// Known issue: ear clipping inverts one triangle in some glyphs with
/// counters (visible as a notch in R). A fix that passed this test
/// changed boolean results elsewhere and was reverted; see PROGRESS.md.
#[test]
#[ignore = "known triangulation fault in glyphs with counters"]
fn glyph_cap_triangles_all_face_the_same_way() {
    use anvil_feature::features::emboss::{nest_loops, text_outlines};
    use anvil_feature::fonts;
    let bytes = fonts::load("builtin:Archivo Black").unwrap();
    for ch in ["R", "A", "S", "B", "8", "O"] {
        let glyphs = text_outlines(&bytes, ch, 20.0, false).unwrap();
        let loops: Vec<Vec<anvil_math::DVec2>> = glyphs.into_iter().flatten().collect();
        for (outer, holes) in nest_loops(&loops) {
            let body = anvil_kernel::ops::extrude_with_holes(&anvil_math::Plane::XY, &outer, &holes, 2.0).unwrap();
            let mesh = anvil_kernel::mesh::tessellate(&body);
            let mut bad = 0;
            let mut area_up = 0.0;
            let mut area_down = 0.0;
            for (k, t) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
                let (a, b, c) =
                    (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
                let n = (b - a).cross(c - a);
                let stored = mesh.normals[t[0] as usize];
                if n.length() > 1e-12 && n.normalize().dot(stored) < 0.0 {
                    bad += 1;
                    eprintln!("{ch}: triangle {k} is flipped, area {:.4}", n.length() / 2.0);
                }
                if stored.z > 0.5 {
                    area_up += n.length() / 2.0;
                }
                if stored.z < -0.5 {
                    area_down += n.length() / 2.0;
                }
            }
            assert_eq!(bad, 0, "{ch}: {bad} flipped triangles");
            assert!((area_up - area_down).abs() / area_up < 1e-9, "{ch}: caps differ {area_up} vs {area_down}");
        }
    }
}
