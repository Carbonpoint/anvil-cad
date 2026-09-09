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
    doc.add_feature(Box::new(SketchFeature::default()));
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
