//! Anvil must be able to read back the neutral files it writes, and to
//! survive the ones a real CAD tool writes.
//!
//! This file is the contract for task 3. It is checksummed by
//! `nova/run_task.sh`; a change to it fails the job.

use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::Document;

/// A 40 x 20 x 12 plate: volume 9600 mm3.
fn plate() -> Document {
    let mut doc = Document::new("plate");
    doc.add_feature(Box::new(SketchFeature::rectangle("XY", 40.0, 20.0)));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "12".into(), ..Default::default() }));
    doc
}

fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("anvil_import_{}_{name}", std::process::id()));
    p
}

#[test]
fn a_step_file_anvil_wrote_reads_back_as_one_body() {
    let doc = plate();
    let path = tmp("plate.step");
    anvil_io::export::step(&doc, &path).expect("write STEP");
    let got = anvil_io::read_step(&path).expect("read STEP");
    assert_eq!(got.solids.len(), 1, "expected one body, notes: {:?}", got.notes);
    let v = got.solids[0].volume().abs();
    assert!((v - 9600.0).abs() < 9600.0 * 0.01, "volume {v}, expected 9600");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_3mf_file_anvil_wrote_reads_back_with_its_triangles() {
    let doc = plate();
    let path = tmp("plate.3mf");
    anvil_io::write_3mf(&doc, &path).expect("write 3MF");
    let got = anvil_io::read_3mf(&path).expect("read 3MF");
    assert_eq!(got.meshes.len(), 1, "expected one mesh, notes: {:?}", got.notes);
    // A box tessellates to twelve triangles.
    assert_eq!(got.meshes[0].triangle_count(), 12, "notes: {:?}", got.notes);
    let _ = std::fs::remove_file(&path);
}

/// A face on a surface the reader does not support is counted and named,
/// not a reason to fail the whole file.
#[test]
fn an_unsupported_surface_is_reported_not_fatal() {
    let text = "\
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('test'),'2;1');
FILE_NAME('t','2026-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));
ENDSEC;
DATA;
#10=CARTESIAN_POINT('',(0.,0.,0.));
#11=DIRECTION('',(0.,0.,1.));
#12=DIRECTION('',(1.,0.,0.));
#13=AXIS2_PLACEMENT_3D('',#10,#11,#12);
#20=CYLINDRICAL_SURFACE('',#13,5.);
#21=ADVANCED_FACE('',(),#20,.T.);
#22=CLOSED_SHELL('',(#21));
#23=MANIFOLD_SOLID_BREP('cyl',#22);
ENDSEC;
END-ISO-10303-21;
";
    let path = tmp("cyl.step");
    std::fs::write(&path, text).unwrap();
    let got = anvil_io::read_step(&path).expect("a cylinder file must not be an error");
    assert!(got.solids.is_empty(), "no planar face, so no body");
    let notes = got.notes.join(" ");
    assert!(notes.contains("CYLINDRICAL_SURFACE"), "the note should name the surface, got {notes:?}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn rubbish_gives_an_error_and_never_a_panic() {
    let path = tmp("rubbish.step");
    std::fs::write(&path, b"this is not a STEP file\n\x00\x01\x02").unwrap();
    assert!(anvil_io::read_step(&path).is_err());
    std::fs::write(&path, b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((").unwrap();
    assert!(anvil_io::read_step(&path).is_err(), "a file that stops mid entity is an error");
    let _ = std::fs::remove_file(&path);

    let path = tmp("rubbish.3mf");
    std::fs::write(&path, b"PK\x03\x04 not really a zip").unwrap();
    assert!(anvil_io::read_3mf(&path).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_file_is_an_error() {
    assert!(anvil_io::read_step(std::path::Path::new("/nonexistent/x.step")).is_err());
    assert!(anvil_io::read_3mf(std::path::Path::new("/nonexistent/x.3mf")).is_err());
}
