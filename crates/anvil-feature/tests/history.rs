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
    let e = doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "h".into(), symmetric: false }));
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
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 1, distance: "3".into(), symmetric: false }));
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
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "2".into(), symmetric: false }));
    let b = doc.bodies();
    assert_eq!(b.len(), 1);
    let exact = (400.0 - std::f64::consts::PI * 25.0) * 2.0;
    assert!((b[0].volume() - exact).abs() / exact < 0.01, "{} vs {exact}", b[0].volume());
}
