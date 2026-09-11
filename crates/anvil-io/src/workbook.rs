//! The Anvil workbook: six CSWA-style practice parts built through the
//! feature API. Each function returns a document the GUI can open and the
//! tests can check. The steps a user would follow in the GUI are listed in
//! `docs/WORKBOOK.md`.

use anvil_feature::features::emboss::TextFeature;
use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::hole::HoleFeature;
use anvil_feature::features::loft::LoftFeature;
use anvil_feature::features::primitives::BoxFeature;
use anvil_feature::features::revolve::RevolveFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::features::solid_extra::PipeFeature;
use anvil_feature::{Document, Material};
use anvil_math::{DVec2, DVec3, Plane};

fn steel(doc: &mut Document, idx: usize) {
    doc.material.insert(idx, Material { name: "Steel".into(), density: 7.85 });
}

/// A sketch feature on a plane offset along Z from XY.
fn sketch_at_z(z: f64) -> SketchFeature {
    SketchFeature::on_plane(Plane { origin: DVec3::new(0.0, 0.0, z), ..Plane::XY })
}

/// Exercise 1: Mounting plate: 100 x 60 x 8, R10 corners, four holes, central slot.
pub fn ex01_mounting_plate() -> Document {
    let mut doc = Document::new("Workbook 01 Mounting plate");
    doc.set_expression("L", "100").ok();
    doc.set_expression("W", "60").ok();
    doc.set_expression("T", "8").ok();
    doc.set_expression("hole_d", "8").ok();
    let mut sk = SketchFeature::on_datum("XY");
    sk.sketch.add_rounded_rectangle(0.0, 0.0, 100.0, 60.0, [10.0; 4]);
    doc.add_feature(Box::new(sk)); // 0
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "T".into(), ..Default::default() })); // 1
    doc.add_feature(Box::new(HoleFeature {
        body: 1,
        x: "10".into(),
        y: "10".into(),
        z: "T".into(),
        diameter: "hole_d".into(),
        depth: "T + 1".into(),
        ..Default::default()
    })); // 2
    doc.add_feature(Box::new(HoleFeature {
        body: 2,
        x: "90".into(),
        y: "10".into(),
        z: "T".into(),
        diameter: "hole_d".into(),
        depth: "T + 1".into(),
        ..Default::default()
    })); // 3
    doc.add_feature(Box::new(HoleFeature {
        body: 3,
        x: "90".into(),
        y: "50".into(),
        z: "T".into(),
        diameter: "hole_d".into(),
        depth: "T + 1".into(),
        ..Default::default()
    })); // 4
    doc.add_feature(Box::new(HoleFeature {
        body: 4,
        x: "10".into(),
        y: "50".into(),
        z: "T".into(),
        diameter: "hole_d".into(),
        depth: "T + 1".into(),
        ..Default::default()
    })); // 5
    let mut slot = sketch_at_z(8.0);
    slot.sketch.add_slot(DVec2::new(41.0, 30.0), DVec2::new(59.0, 30.0), 12.0);
    doc.add_feature(Box::new(slot)); // 6
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 6,
        distance: "-(T + 1)".into(),
        operation: "cut".into(),
        target: 5,
        ..Default::default()
    })); // 7
    steel(&mut doc, 7);
    doc
}

/// Exercise 2: L-bracket: 60 base, 40 upright, 50 wide, 6 thick, R6 inner fillet
/// drawn in the profile, two base holes and one upright slot.
pub fn ex02_l_bracket() -> Document {
    let mut doc = Document::new("Workbook 02 L-bracket");
    let mut sk = SketchFeature::on_datum("XZ");
    // L profile in the XZ plane (sketch x = world X, sketch y = world Z).
    let p = [(0.0, 0.0), (60.0, 0.0), (60.0, 6.0), (6.0, 6.0), (6.0, 40.0), (0.0, 40.0)];
    let ids: Vec<_> = p.iter().map(|(x, y)| sk.sketch.add_point(*x, *y)).collect();
    let mut lines = Vec::new();
    for i in 0..6 {
        lines.push(sk.sketch.add_line(ids[i], ids[(i + 1) % 6]));
    }
    // Inner corner is between line 2 (60,6)->(6,6) and line 3 (6,6)->(6,40).
    sk.sketch.fillet_lines(lines[2], lines[3], 6.0).expect("fillet");
    doc.add_feature(Box::new(sk)); // 0
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "-50".into(), ..Default::default() })); // 1, along -Y? XZ normal is -Y, so -50 goes +Y
    doc.add_feature(Box::new(HoleFeature {
        body: 1,
        x: "45".into(),
        y: "12".into(),
        z: "6".into(),
        diameter: "7".into(),
        depth: "10".into(),
        ..Default::default()
    })); // 2
    doc.add_feature(Box::new(HoleFeature {
        body: 2,
        x: "45".into(),
        y: "38".into(),
        z: "6".into(),
        diameter: "7".into(),
        depth: "10".into(),
        ..Default::default()
    })); // 3
         // Slot in the upright: sketch on the YZ-parallel face at x = 6 (upright outer face is x = 0; sketch on x = 6 face and cut inward).
    let mut slot =
        SketchFeature::on_plane(Plane { origin: DVec3::new(6.0, 0.0, 0.0), x_axis: DVec3::Y, y_axis: DVec3::Z });
    slot.sketch.add_slot(DVec2::new(18.0, 25.0), DVec2::new(32.0, 25.0), 7.0);
    doc.add_feature(Box::new(slot)); // 4
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 4,
        distance: "-8".into(),
        operation: "cut".into(),
        target: 3,
        ..Default::default()
    })); // 5
    steel(&mut doc, 5);
    doc
}

/// Exercise 3: Stepped shaft: revolved profile with chamfers, a keyway, and a
/// cross hole.
pub fn ex03_stepped_shaft() -> Document {
    let mut doc = Document::new("Workbook 03 Stepped shaft");
    let mut sk = SketchFeature::on_datum("XY");
    // Half profile above the X axis (sketch y = radius). Revolve about X.
    let pts = [
        (0.0, 0.0),
        (0.0, 9.0),
        (1.0, 10.0),
        (30.0, 10.0),
        (30.0, 15.0),
        (70.0, 15.0),
        (70.0, 8.0),
        (89.0, 8.0),
        (90.0, 7.0),
        (90.0, 0.0),
    ];
    let ids: Vec<_> = pts.iter().map(|(x, y)| sk.sketch.add_point(*x, *y)).collect();
    for i in 0..ids.len() {
        sk.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
    }
    doc.add_feature(Box::new(sk)); // 0
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 0,
        axis: "X".into(),
        angle_deg: "360".into(),
        ..Default::default()
    })); // 1
         // Keyway on the large step: 6 wide, 3 deep, 25 long, cut from the top (z = 15).
    let mut key = sketch_at_z(15.0);
    key.sketch.add_rectangle(38.0, -3.0, 63.0, 3.0);
    doc.add_feature(Box::new(key)); // 2
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 2,
        distance: "-3".into(),
        operation: "cut".into(),
        target: 1,
        ..Default::default()
    })); // 3
         // Cross hole through the small end, along Y, at x = 80. Plane XZ has normal -Y; place it at y = +10 and drill 20 deep in -normal direction (+Y)... use the plane at y = -10 with depth 20.
         // The XZ datum normal is -Y. Offset 10 puts the plane at y = -10; the
         // hole drills against the normal, toward +Y, through the shaft.
    doc.add_feature(Box::new(HoleFeature {
        body: 3,
        plane: "XZ".into(),
        x: "80".into(),
        y: "0".into(),
        z: "10".into(),
        diameter: "4".into(),
        depth: "20".into(),
        ..Default::default()
    })); // 4
    steel(&mut doc, 4);
    doc
}

/// Exercise 4: Hex nut M10: hexagon 17 across flats, 8 thick, chamfered by
/// intersecting with a revolved cone, with a 10 mm through hole.
pub fn ex04_hex_nut() -> Document {
    let mut doc = Document::new("Workbook 04 Hex nut");
    let mut hex = SketchFeature::on_datum("XY");
    hex.sketch.add_polygon_inscribed(DVec2::ZERO, 8.5, 6, 0.0);
    doc.add_feature(Box::new(hex)); // 0
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "8".into(), ..Default::default() })); // 1
                                                                                                         // Chamfer body: revolve a profile about Z that is a cylinder of the corner radius with 30 degree chamfers top and bottom.
    let mut ch = SketchFeature::on_datum("XZ");
    let r = 8.5 / (std::f64::consts::PI / 6.0).cos(); // corner radius
    let c = 1.2; // chamfer height
    let pts = [(0.0, 0.0), (r - c * 1.732, 0.0), (r, c), (r, 8.0 - c), (r - c * 1.732, 8.0), (0.0, 8.0)];
    let ids: Vec<_> = pts.iter().map(|(x, y)| ch.sketch.add_point(*x, *y)).collect();
    for i in 0..ids.len() {
        ch.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
    }
    doc.add_feature(Box::new(ch)); // 2
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 2,
        axis: "Y".into(),
        angle_deg: "360".into(),
        operation: "intersect".into(),
        target: 1,
    })); // 3
    doc.add_feature(Box::new(HoleFeature {
        body: 3,
        x: "0".into(),
        y: "0".into(),
        z: "8".into(),
        diameter: "10".into(),
        depth: "9".into(),
        ..Default::default()
    })); // 4
    steel(&mut doc, 4);
    doc
}

/// Exercise 5: Pipe elbow with flanges: a 12 mm pipe along a line-arc-line path,
/// joined to two square flanges with bolt holes.
pub fn ex05_pipe_elbow() -> Document {
    let mut doc = Document::new("Workbook 05 Pipe elbow");
    let mut path = SketchFeature::on_datum("XY");
    let a = path.sketch.add_point(0.0, 0.0);
    let b = path.sketch.add_point(40.0, 0.0);
    path.sketch.add_line(a, b);
    let c = path.sketch.add_point(40.0, 20.0);
    let e = path.sketch.add_point(60.0, 20.0);
    path.sketch.add_arc(c, b, e);
    let f = path.sketch.add_point(60.0, 60.0);
    path.sketch.add_line(e, f);
    doc.add_feature(Box::new(path)); // 0
    doc.add_feature(Box::new(PipeFeature { path: 0, diameter: "12".into() })); // 1
    doc.add_feature(Box::new(BoxFeature {
        x: "-2".into(),
        y: "-12".into(),
        z: "-12".into(),
        width: "4".into(),
        depth: "24".into(),
        height: "24".into(),
    })); // 2
    doc.add_feature(Box::new(BoxFeature {
        x: "48".into(),
        y: "58".into(),
        z: "-12".into(),
        width: "24".into(),
        depth: "4".into(),
        height: "24".into(),
    })); // 3
         // Join pipe and flanges with Combine: pipe + flange A, then + flange B.
    use anvil_feature::features::pending::CombineFeature;
    doc.add_feature(Box::new(CombineFeature { body: 1, tool: 2, op: "join".into() })); // 4
    doc.add_feature(Box::new(CombineFeature { body: 4, tool: 3, op: "join".into() })); // 5
                                                                                       // Bolt holes in flange A (plane YZ at x = 2, drilling in -X).
    doc.add_feature(Box::new(HoleFeature {
        body: 5,
        plane: "YZ".into(),
        x: "-8".into(),
        y: "-8".into(),
        z: "2".into(),
        diameter: "3".into(),
        depth: "5".into(),
        ..Default::default()
    })); // 6
    doc.add_feature(Box::new(HoleFeature {
        body: 6,
        plane: "YZ".into(),
        x: "8".into(),
        y: "8".into(),
        z: "2".into(),
        diameter: "3".into(),
        depth: "5".into(),
        ..Default::default()
    })); // 7
    steel(&mut doc, 7);
    doc
}

/// Exercise 6: Square-to-round adapter: loft from a 40 mm square to a 24 mm circle
/// over 30 mm, with a through bore and an engraved label.
pub fn ex06_adapter() -> Document {
    let mut doc = Document::new("Workbook 06 Adapter");
    let mut sq = SketchFeature::on_datum("XY");
    sq.sketch.add_rectangle_center(0.0, 0.0, 40.0, 40.0);
    doc.add_feature(Box::new(sq)); // 0
    let mut ci = sketch_at_z(30.0);
    let c = ci.sketch.add_point(0.0, 0.0);
    ci.sketch.add_circle(c, 12.0);
    doc.add_feature(Box::new(ci)); // 1
    doc.add_feature(Box::new(LoftFeature { sections: "0, 1".into() })); // 2
    doc.add_feature(Box::new(HoleFeature {
        body: 2,
        x: "0".into(),
        y: "0".into(),
        z: "30".into(),
        diameter: "12".into(),
        depth: "31".into(),
        ..Default::default()
    })); // 3
         // Engrave: text sketched on the bottom face (z = 0), extruded 0.5 mm up into the body, cut.
    doc.add_feature(Box::new(TextFeature {
        text: "ANVIL".into(),
        plane: "XY".into(),
        x: "0".into(),
        y: "-19".into(),
        z: "0".into(),
        size: "5".into(),
        height: "0.5".into(),
        center: true,
        operation: "cut".into(),
        target: 3,
        ..Default::default()
    })); // 4
    steel(&mut doc, 4);
    doc
}

/// Name and builder for one exercise.
pub type Exercise = (&'static str, fn() -> Document);

pub const EXERCISES: [Exercise; 6] = [
    ("01 Mounting plate", ex01_mounting_plate),
    ("02 L-bracket", ex02_l_bracket),
    ("03 Stepped shaft", ex03_stepped_shaft),
    ("04 Hex nut", ex04_hex_nut),
    ("05 Pipe elbow", ex05_pipe_elbow),
    ("06 Adapter", ex06_adapter),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn check(doc: &Document) -> f64 {
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "{}: feature {i} {}: {:?}", doc.name, f.feature.name(), f.error);
        }
        let bodies = doc.bodies();
        assert!(!bodies.is_empty(), "{}: no bodies", doc.name);
        bodies.iter().map(|b| b.volume()).sum()
    }

    #[test]
    fn ex01_volume() {
        let v = check(&ex01_mounting_plate());
        let plate = (100.0 * 60.0 - 4.0 * 100.0 * (1.0 - PI / 4.0)) * 8.0;
        let holes = 4.0 * PI * 16.0 * 8.0;
        let slot = (18.0 * 12.0 + PI * 36.0) * 8.0;
        let exact = plate - holes - slot;
        assert!((v - exact).abs() / exact < 0.01, "{v} vs {exact}");
    }

    #[test]
    fn ex02_volume() {
        let v = check(&ex02_l_bracket());
        let profile = 60.0 * 6.0 + 34.0 * 6.0 + 36.0 * (1.0 - PI / 4.0);
        let body = profile * 50.0;
        let holes = 2.0 * PI * 3.5 * 3.5 * 6.0;
        let slot = (14.0 * 7.0 + PI * 3.5 * 3.5) * 6.0;
        let exact = body - holes - slot;
        assert!((v - exact).abs() / exact < 0.02, "{v} vs {exact}");
    }

    #[test]
    fn ex03_cross_hole_is_cut() {
        let mut doc = ex03_stepped_shaft();
        let with = doc.bodies()[0].volume();
        doc.set_suppressed(4, true);
        // Suppressing the hole leaves the material feature index pointing at
        // nothing, so read the keyway body directly.
        let without = doc.features[3].output.as_ref().unwrap().bodies[0].volume();
        let expected = PI * 4.0 * 16.0;
        assert!(((without - with) - expected).abs() / expected < 0.1, "{} vs {expected}", without - with);
    }

    #[test]
    fn ex03_volume() {
        let v = check(&ex03_stepped_shaft());
        let cyl = |r: f64, l: f64| PI * r * r * l;
        let body = cyl(10.0, 30.0) + cyl(15.0, 40.0) + cyl(8.0, 20.0)
            - 2.0 * (cyl(10.0, 1.0) - PI * 1.0 * (100.0 + 90.0 + 81.0) / 3.0 * 1.0 / 1.0).max(0.0);
        let keyway = 25.0 * 6.0 * 3.0;
        let cross = cyl(2.0, 16.0);
        let exact = body - keyway - cross;
        assert!((v - exact).abs() / exact < 0.03, "{v} vs {exact}");
    }

    #[test]
    fn ex04_volume() {
        let v = check(&ex04_hex_nut());
        let hex_area = 2.0 * 3f64.sqrt() * 8.5 * 8.5;
        let rough = hex_area * 8.0 - PI * 25.0 * 8.0;
        assert!(v < rough && v > rough * 0.9, "{v} vs {rough}");
    }

    #[test]
    fn ex05_volume() {
        let v = check(&ex05_pipe_elbow());
        let path_len = 40.0 + PI / 2.0 * 20.0 + 40.0;
        let pipe = PI * 36.0 * path_len;
        let flanges = 2.0 * 4.0 * 24.0 * 24.0;
        // Each flange overlaps half its thickness of pipe (the pipe ends mid-flange).
        let overlap = 2.0 * PI * 36.0 * 2.0;
        let exact = pipe + flanges - overlap - 2.0 * PI * 1.5 * 1.5 * 4.0;
        assert!((v - exact).abs() / exact < 0.05, "{v} vs {exact}");
    }

    #[test]
    fn ex06_volume() {
        let v = check(&ex06_adapter());
        // Loft between a 40 square (1600) and a 24 circle (452): prismoid estimate.
        let a1: f64 = 1600.0;
        let a2: f64 = PI * 144.0;
        let am = (a1.sqrt() + a2.sqrt()).powi(2) / 4.0;
        let loft = 30.0 / 6.0 * (a1 + 4.0 * am + a2);
        let bore = PI * 36.0 * 30.0;
        let exact = loft - bore;
        assert!((v - exact).abs() / exact < 0.08, "{v} vs {exact}");
        assert_eq!(ex06_adapter().bodies().len(), 1, "engraving cuts into the body");
    }
}
