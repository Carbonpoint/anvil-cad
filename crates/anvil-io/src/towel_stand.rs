//! Stand for an IKEA paper towel holder.
//!
//! A flat disc a little wider than the 125 mm IKEA base, with two screw
//! holes 70 mm apart on a diameter. Screws go up from underneath, so each
//! hole has a counterbore from the bottom that keeps the screw head clear of
//! water on the counter. The underside stands on short hexagonal feet and a
//! segmented rim, so water can drain and the counter can dry.
//!
//! Every part is built by extruding profiles with holes. No booleans are
//! used, because repeated booleans are the weak spot of the native kernel.
//!
//! Print orientation: as modelled, feet down. The slab bridges the gaps
//! between feet (at most about 5 mm), which prints cleanly.

use anvil_feature::features::emboss::TextFeature;
use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::Document;
use anvil_math::{DVec2, DVec3, Plane};

/// Disc diameter: 15 mm wider than the IKEA base.
pub const DIA: f64 = 140.0;
/// Slab thickness above the feet.
pub const THICK: f64 = 6.0;
/// Foot height: the drainage gap under the stand.
pub const FOOT_H: f64 = 1.5;
/// Screw hole: M3 clearance, also fits 2 mm and 2.5 mm screws.
pub const HOLE_D: f64 = 3.4;
/// Hole centres, measured centre to centre.
pub const HOLE_SEP: f64 = 70.0;
/// Counterbore for an M3 pan or button head.
pub const CB_D: f64 = 7.0;
/// Counterbore depth into the slab. With the feet, the head sits
/// FOOT_H + CB_DEPTH above the counter.
pub const CB_DEPTH: f64 = 3.0;
/// Segmented rim under the edge: width and number of drain gaps.
pub const RIM_W: f64 = 3.0;
pub const RIM_GAPS: usize = 24;
pub const RIM_GAP_W: f64 = 4.0;
/// Hexagonal feet: circumradius and grid pitch.
pub const FOOT_R: f64 = 2.0;
pub const FOOT_PITCH: f64 = 7.0;

/// Roll assumed for the percent rings: a standard 140 mm roll on a 41 mm
/// core. Percent means paper left, by area of the roll's cross-section.
pub const ROLL_D: f64 = 140.0;
pub const CORE_D: f64 = 41.0;
/// Percent levels shown, with raised rings and labels.
pub const LEVELS: [f64; 3] = [75.0, 50.0, 25.0];
/// Marker ring width and height above the top face.
pub const RING_W: f64 = 1.6;
pub const MARK_H: f64 = 0.6;

/// Roll radius when `percent` of the paper is left.
// Design rules checked at compile time.
const _: () = assert!(DIA > 125.0 && DIA <= 150.0, "stand should be a little wider than the IKEA base");
const _: () = assert!(FOOT_PITCH - 2.0 * FOOT_R <= 5.0, "gaps between feet must be short bridges");
const _: () = assert!(RIM_GAP_W <= 5.0, "rim drain gaps must be short bridges");
const _: () = assert!(CB_DEPTH < THICK, "counterbore must leave material under the screw head");

pub fn level_radius(percent: f64) -> f64 {
    let (r_full, r_core) = (ROLL_D / 2.0, CORE_D / 2.0);
    (r_core * r_core + percent / 100.0 * (r_full * r_full - r_core * r_core)).sqrt()
}

fn plane_at(z: f64) -> Plane {
    Plane { origin: DVec3::new(0.0, 0.0, z), ..Plane::XY }
}

fn add_loop(sk: &mut SketchFeature, pts: &[DVec2]) {
    let ids: Vec<_> = pts.iter().map(|p| sk.sketch.add_point(p.x, p.y)).collect();
    for i in 0..ids.len() {
        sk.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
    }
}

fn circle(c: DVec2, r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            c + DVec2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

/// Hole centres on the X axis.
pub fn hole_centres() -> [DVec2; 2] {
    [DVec2::new(-HOLE_SEP / 2.0, 0.0), DVec2::new(HOLE_SEP / 2.0, 0.0)]
}

/// Foot centres: a hex grid inside the rim, kept clear of the counterbores.
pub fn foot_centres() -> Vec<DVec2> {
    let r_max = DIA / 2.0 - RIM_W - FOOT_R - 2.0;
    let keep_out = CB_D / 2.0 + FOOT_R + 1.5;
    let row = FOOT_PITCH * 3f64.sqrt() / 2.0;
    let n = (DIA / FOOT_PITCH) as i64 + 2;
    let mut out = Vec::new();
    for j in -n..=n {
        for i in -n..=n {
            let x = i as f64 * FOOT_PITCH + if j % 2 != 0 { FOOT_PITCH / 2.0 } else { 0.0 };
            let p = DVec2::new(x, j as f64 * row);
            if p.length() > r_max {
                continue;
            }
            if hole_centres().iter().any(|h| (p - *h).length() < keep_out) {
                continue;
            }
            out.push(p);
        }
    }
    out
}

/// Rim segments: annular sectors under the outer edge with drain gaps.
pub fn rim_segments() -> Vec<Vec<DVec2>> {
    let r_out = DIA / 2.0;
    let r_in = r_out - RIM_W;
    let pitch = std::f64::consts::TAU / RIM_GAPS as f64;
    let gap = RIM_GAP_W / r_out;
    (0..RIM_GAPS)
        .map(|k| {
            let a0 = k as f64 * pitch + gap / 2.0;
            let a1 = (k + 1) as f64 * pitch - gap / 2.0;
            let steps = 8;
            let mut pts = Vec::new();
            for s in 0..=steps {
                let a = a0 + (a1 - a0) * s as f64 / steps as f64;
                pts.push(DVec2::new(r_out * a.cos(), r_out * a.sin()));
            }
            for s in (0..=steps).rev() {
                let a = a0 + (a1 - a0) * s as f64 / steps as f64;
                pts.push(DVec2::new(r_in * a.cos(), r_in * a.sin()));
            }
            pts
        })
        .collect()
}

/// The stand. Feature order: feet sketch, feet, rim sketch, rim,
/// lower slab sketch, lower slab, upper slab sketch, upper slab.
pub fn towel_stand() -> Document {
    let mut doc = Document::new("Paper towel stand");
    let r = DIA / 2.0;

    // 0, 1: hexagonal feet on the counter.
    let mut feet = SketchFeature::on_datum("XY");
    for c in foot_centres() {
        add_loop(&mut feet, &circle(c, FOOT_R, 6));
    }
    doc.add_feature(Box::new(feet));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: format!("{FOOT_H}"), ..Default::default() }));

    // 2, 3: segmented rim, same height as the feet.
    let mut rim = SketchFeature::on_datum("XY");
    for seg in rim_segments() {
        add_loop(&mut rim, &seg);
    }
    doc.add_feature(Box::new(rim));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 2, distance: format!("{FOOT_H}"), ..Default::default() }));

    // 4, 5: lower slab with the counterbores.
    let mut lower = SketchFeature::on_plane(plane_at(FOOT_H));
    add_loop(&mut lower, &circle(DVec2::ZERO, r, 180));
    for h in hole_centres() {
        add_loop(&mut lower, &circle(h, CB_D / 2.0, 40));
    }
    doc.add_feature(Box::new(lower));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 4, distance: format!("{CB_DEPTH}"), ..Default::default() }));

    // 6, 7: upper slab with the screw holes.
    let mut upper = SketchFeature::on_plane(plane_at(FOOT_H + CB_DEPTH));
    add_loop(&mut upper, &circle(DVec2::ZERO, r, 180));
    for h in hole_centres() {
        add_loop(&mut upper, &circle(h, HOLE_D / 2.0, 24));
    }
    doc.add_feature(Box::new(upper));
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 6,
        distance: format!("{}", THICK - CB_DEPTH),
        ..Default::default()
    }));

    // 8, 9: percent rings on the top face, one part for a second colour.
    // The wire base rests evenly on rings of equal height.
    let top = FOOT_H + THICK;
    let mut rings = SketchFeature::on_plane(plane_at(top));
    for level in LEVELS {
        let rr = level_radius(level);
        add_loop(&mut rings, &circle(DVec2::ZERO, rr + RING_W / 2.0, 144));
        add_loop(&mut rings, &circle(DVec2::ZERO, rr - RING_W / 2.0, 144));
    }
    doc.add_feature(Box::new(rings));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 8, distance: format!("{MARK_H}"), ..Default::default() }));

    // 10, 11, 12: labels just inside each ring, at the front of the stand.
    for level in LEVELS {
        let rr = level_radius(level);
        doc.add_feature(Box::new(TextFeature {
            // Plain numbers: the percent sign's small circles are too thin
            // to print at this size with a 0.4 mm nozzle.
            text: format!("{}", level as i64),
            font_path: "builtin:Archivo Black".into(),
            x: "0".into(),
            y: format!("{:.3}", -(rr - RING_W / 2.0 - 1.4)),
            z: format!("{top}"),
            size: "6".into(),
            height: format!("{MARK_H}"),
            thicken: "0.1".into(),
            center: true,
            ..Default::default()
        }));
    }

    for i in [1, 3, 5, 7] {
        doc.appearance.insert(i, [235, 235, 230]);
        doc.material.insert(i, anvil_feature::Material { name: "PLA".into(), density: 1.24 });
    }
    for i in [9, 10, 11, 12] {
        doc.appearance.insert(i, [40, 120, 200]);
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stand_builds_with_expected_geometry() {
        let doc = towel_stand();
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "feature {i} {}: {:?}", f.feature.name(), f.error);
        }
        // Holes: 70 mm apart, symmetric about the centre, on one diameter.
        let [a, b] = hole_centres();
        assert!(((b - a).length() - 70.0).abs() < 1e-12);
        assert!((a + b).length() < 1e-12);
        // Upper slab: full disc minus two screw holes.
        let upper = &doc.features[7].output.as_ref().unwrap().bodies[0];
        let disc = std::f64::consts::PI * (DIA / 2.0).powi(2);
        let hole = std::f64::consts::PI * (HOLE_D / 2.0).powi(2);
        let expect = (disc - 2.0 * hole) * (THICK - CB_DEPTH);
        assert!((upper.volume() - expect).abs() / expect < 0.002, "{} vs {expect}", upper.volume());
        assert_eq!(upper.open_edge_report(), (0, 0));
        // Height: feet plus slab, then the markers on top.
        assert!((upper.bounds().max.z - (FOOT_H + THICK)).abs() < 1e-9);
        let top = doc.bodies().iter().fold(f64::MIN, |m, s| m.max(s.bounds().max.z));
        assert!((top - (FOOT_H + THICK + MARK_H)).abs() < 1e-9);
        // Diameter: a little wider than the 125 mm IKEA base (checked at
        // compile time below).
    }

    #[test]
    fn percent_markers_are_placed_and_printable() {
        let doc = towel_stand();
        // Rings at the right radii for a standard roll, outermost first.
        let radii: Vec<f64> = LEVELS.iter().map(|&l| level_radius(l)).collect();
        assert!((level_radius(100.0) - ROLL_D / 2.0).abs() < 1e-9);
        assert!((level_radius(0.0) - CORE_D / 2.0).abs() < 1e-9);
        assert!(radii.windows(2).all(|w| w[0] > w[1] + 5.0), "{radii:?}");
        // Rings clear the screw holes and stay on the disc.
        for rr in &radii {
            assert!((rr - HOLE_SEP / 2.0).abs() > RING_W / 2.0 + HOLE_D / 2.0 + 0.5, "ring {rr} hits a hole");
            assert!(rr + RING_W / 2.0 < DIA / 2.0);
        }
        // Rings are one part with three ring bodies; labels print cleanly.
        assert_eq!(doc.features[9].output.as_ref().unwrap().bodies.len(), 3);
        for i in [10, 11, 12] {
            let f = &doc.features[i];
            assert!(f.error.is_none(), "{:?}", f.error);
            let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
            assert!(note.contains("printable"), "{}: {note}", f.feature.name());
        }
        // Each label sits between its ring and the next ring inward.
        for (k, i) in [10, 11, 12].into_iter().enumerate() {
            let b =
                doc.features[i].output.as_ref().unwrap().bodies.iter().fold(anvil_math::Aabb::empty(), |mut acc, s| {
                    let bb = s.bounds();
                    acc.include(bb.min);
                    acc.include(bb.max);
                    acc
                });
            let outer_edge = -(radii[k] - RING_W / 2.0);
            assert!(b.min.y > outer_edge, "label {k} overlaps its ring: {b:?}");
            if let Some(next) = radii.get(k + 1) {
                assert!(b.max.y < -(next + RING_W / 2.0), "label {k} overlaps the next ring: {b:?}");
            }
        }
    }

    #[test]
    fn feet_clear_the_counterbores_and_bridges_are_short() {
        let feet = foot_centres();
        assert!(feet.len() > 150, "{} feet", feet.len());
        for h in hole_centres() {
            for f in &feet {
                assert!((*f - h).length() - FOOT_R > CB_D / 2.0 + 1.0, "foot {f:?} too close to hole {h:?}");
            }
        }
        // Bridge lengths between feet and across rim gaps are checked at
        // compile time below.
    }
}
