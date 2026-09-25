//! STEP files as CAD tools write them: advanced faces with circles, a
//! seam edge, and a hole wall on a cylinder. The fixture is written by
//! hand in that style, with entity ids out of order.

use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn a_plate_with_a_round_hole_reads_as_one_closed_body() {
    let got = anvil_io::read_step(&fixture("plate_hole.step")).expect("read");
    assert!(got.notes.is_empty(), "nothing should be skipped: {:?}", got.notes);
    assert_eq!(got.solids.len(), 1);
    let s = &got.solids[0];
    // 40 x 20 x 10 less a hole of radius 5; the hole is a polygon of
    // chords, so it is a little smaller than a true circle.
    let want = 8000.0 - std::f64::consts::PI * 25.0 * 10.0;
    let v = s.volume();
    assert!((v - want).abs() < 0.005 * want, "volume {v}, want about {want}");
    assert!(s.open_edges().is_empty(), "the body has open edges: {:?}", &s.open_edges()[..s.open_edges().len().min(4)]);
    // Euler-Poincare with rings: V - E + F - R = 2 - 2G. The top and the
    // bottom face each have one inner loop, and the hole makes genus one.
    let rings: isize = s.faces.values().map(|f| f.inner.len() as isize).sum();
    assert_eq!(rings, 2);
    assert_eq!(s.euler_characteristic() - rings, 0, "genus one");
}

#[test]
fn a_plate_with_rounded_corners_reads_closed_with_quarter_arcs() {
    let got = anvil_io::read_step(&fixture("rounded_plate.step")).expect("read");
    assert!(got.notes.is_empty(), "nothing should be skipped: {:?}", got.notes);
    assert_eq!(got.solids.len(), 1);
    let s = &got.solids[0];
    // Each corner loses a square of 3 x 3 less a quarter circle.
    let want = (800.0 - 4.0 * (9.0 - std::f64::consts::PI * 9.0 / 4.0)) * 10.0;
    let v = s.volume();
    assert!((v - want).abs() < 0.002 * want, "volume {v}, want about {want}");
    assert!(s.open_edges().is_empty(), "open edges: {:?}", &s.open_edges()[..s.open_edges().len().min(4)]);
    // The rounds bulge outward: no point lies outside the plate's box.
    let b = s.bounds();
    assert!(b.max.x <= 20.0 + 1e-9 && b.min.y >= -10.0 - 1e-9, "{b:?}");
}
