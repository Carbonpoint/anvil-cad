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
