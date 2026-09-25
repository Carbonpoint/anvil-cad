//! A plane parameter must be able to name a datum, a plane Feature, or
//! a flat face of a Body, and must find that face again after the part
//! changes.
//!
//! This file is the contract for task 2. It is checksummed by
//! `nova/run_task.sh`; a change to it fails the job.

use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::features::solid_extra::MidplaneFeature;
use anvil_feature::{Document, PlaneRef};
use anvil_math::{DVec3, Plane};

/// A plane with its origin at `z` and its normal along Z.
fn z_plane(z: f64) -> Plane {
    Plane { origin: DVec3::new(0.0, 0.0, z), x_axis: DVec3::X, y_axis: DVec3::Y }
}

/// Sketch a 40 x 20 rectangle on XY and extrude it `d` high.
/// Feature 0 is the sketch, feature 1 the extrude.
fn plate(d: &str) -> Document {
    let mut doc = Document::new("plate");
    doc.add_feature(Box::new(SketchFeature::rectangle("XY", 40.0, 20.0)));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: d.into(), ..Default::default() }));
    doc
}

/// The plane a Feature produced, after a regenerate.
fn plane_of(doc: &Document, idx: usize) -> Plane {
    doc.features[idx]
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("feature {idx} has no output: {:?}", doc.features[idx].error))
        .plane
        .unwrap_or_else(|| panic!("feature {idx} made no plane: {:?}", doc.features[idx].error))
}

#[test]
fn a_midplane_between_a_face_and_a_datum_sits_halfway() {
    let mut doc = plate("12");
    let top = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) };
    let idx = doc.add_feature(Box::new(MidplaneFeature { first: top, second: PlaneRef::Datum("XY".into()) }));
    assert!(doc.features[idx].error.is_none(), "{:?}", doc.features[idx].error);
    let p = plane_of(&doc, idx);
    assert!((p.origin.z - 6.0).abs() < 1e-9, "origin {:?}", p.origin);
    assert!(p.normal().cross(DVec3::Z).length() < 1e-9, "normal {:?}", p.normal());
}

#[test]
fn the_reference_follows_the_face_when_the_part_changes() {
    let mut doc = plate("12");
    let top = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) };
    let idx = doc.add_feature(Box::new(MidplaneFeature { first: top, second: PlaneRef::Datum("XY".into()) }));
    assert!((plane_of(&doc, idx).origin.z - 6.0).abs() < 1e-9);
    // The plate gets taller. The picked face is the same face, higher up.
    doc.edit_feature(1, |f| {
        f.set_param("distance", anvil_feature::ParamValue::Expr("20".into())).unwrap();
    });
    assert!(doc.features[idx].error.is_none(), "{:?}", doc.features[idx].error);
    let p = plane_of(&doc, idx);
    assert!((p.origin.z - 10.0).abs() < 1e-6, "the reference did not follow the face: origin {:?}", p.origin);
}

#[test]
fn a_face_that_is_gone_keeps_the_last_plane_and_says_so() {
    let mut doc = plate("12");
    // No face of this Body lies at z = 40.
    let nowhere = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(40.0) };
    let idx = doc.add_feature(Box::new(MidplaneFeature { first: nowhere, second: PlaneRef::Datum("XY".into()) }));
    assert!(doc.features[idx].error.is_none(), "a moved face must not be an error: {:?}", doc.features[idx].error);
    let p = plane_of(&doc, idx);
    assert!((p.origin.z - 20.0).abs() < 1e-9, "the stored plane should still be used: {:?}", p.origin);
    let note = doc.features[idx].output.as_ref().and_then(|o| o.note.clone()).unwrap_or_default();
    assert!(note.to_lowercase().contains("face"), "expected a note about the face, got {note:?}");
}

#[test]
fn a_reference_to_a_feature_with_no_output_is_an_error() {
    let mut doc = plate("12");
    let top = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) };
    let idx = doc.add_feature(Box::new(MidplaneFeature { first: top, second: PlaneRef::Datum("XY".into()) }));
    doc.set_suppressed(1, true);
    assert!(doc.features[idx].error.is_some(), "a suppressed source must make the midplane fail");
}

#[test]
fn a_plane_feature_can_still_be_named_by_number() {
    let mut doc = plate("12");
    let idx = doc
        .add_feature(Box::new(MidplaneFeature { first: PlaneRef::Feature(0), second: PlaneRef::Datum("XY".into()) }));
    assert!(doc.features[idx].error.is_none(), "{:?}", doc.features[idx].error);
}

#[test]
fn the_document_round_trips_through_json() {
    let mut doc = plate("12");
    let top = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) };
    let idx = doc.add_feature(Box::new(MidplaneFeature { first: top, second: PlaneRef::Datum("XY".into()) }));
    let json = doc.to_json().unwrap();
    let back = Document::from_json(&json).unwrap();
    let a = plane_of(&doc, idx);
    let b = plane_of(&back, idx);
    assert!((a.origin - b.origin).length() < 1e-9, "{:?} vs {:?}", a.origin, b.origin);
}

#[test]
fn a_document_written_before_this_change_still_loads() {
    let json = include_str!("fixtures/old_midplane.json");
    let doc = Document::from_json(json).expect("an old document must still load");
    let idx = doc.features.len() - 1;
    assert_eq!(doc.features[idx].feature.kind(), "midplane");
    assert!(doc.features[idx].error.is_none(), "{:?}", doc.features[idx].error);
    // Old file: first "XY", second "0", the sketch on XY. Both planes
    // are z = 0, so the midplane is z = 0 as well.
    let p = plane_of(&doc, idx);
    assert!(p.origin.length() < 1e-9, "origin {:?}", p.origin);
}

#[test]
fn a_label_reads_as_a_person_would_say_it() {
    let doc = plate("12");
    assert_eq!(PlaneRef::Datum("XY".into()).label(&doc), "XY");
    let f = PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) };
    let l = f.label(&doc);
    assert!(l.to_lowercase().contains("face"), "a face label should say face, got {l:?}");
    assert_eq!(PlaneRef::Face { feature: 1, body: 0, plane: z_plane(12.0) }.depends_on(), Some(1));
    assert_eq!(PlaneRef::Datum("XY".into()).depends_on(), None);
}
