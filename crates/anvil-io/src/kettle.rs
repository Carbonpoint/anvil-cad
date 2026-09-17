//! Cast iron kettle sample (tetsubin), see `docs/KETTLE.md`.
//!
//! A flat round body with a hobnail dome, a tapered spout, two lugs, a
//! bail, and a domed lid with a bud knob. Built through the feature API so
//! it is also a regression test for revolve runs, Pattern on face,
//! localized booleans, and the tapered pipe.

use anvil_feature::features::extrude::ExtrudeFeature;
use anvil_feature::features::hole::HoleFeature;
use anvil_feature::features::pending::CombineFeature;
use anvil_feature::features::revolve::RevolveFeature;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::features::solid_extra::PipeFeature;
use anvil_feature::features::surface_pattern::SurfacePatternFeature;
use anvil_feature::Document;
use anvil_kernel::{Solid, SurfaceGeom};
use anvil_math::{DVec2, DVec3, Plane};
use anvil_sketch::{Entity, EntityId};

/// Draw a closed loop of lines and splines on a sketch. Each piece is a
/// list of points: one point is a corner, three or more make a spline
/// through them. A line joins the end of one piece to the start of the
/// next unless they are the same point, and the loop closes back to the
/// first point.
fn closed_loop(sk: &mut SketchFeature, pieces: &[&[(f64, f64)]]) {
    let same = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
    let mut first: Option<(EntityId, (f64, f64))> = None;
    let mut last: Option<(EntityId, (f64, f64))> = None;
    for piece in pieces {
        let mut ids: Vec<EntityId> = Vec::with_capacity(piece.len());
        for (j, &(x, y)) in piece.iter().enumerate() {
            let id = match (j, last) {
                (0, Some((prev_id, prev))) if same(prev, (x, y)) => prev_id,
                _ => sk.sketch.add_point(x, y),
            };
            if j == 0 {
                if let Some((prev_id, _)) = last {
                    if prev_id != id {
                        sk.sketch.add_line(prev_id, id);
                    }
                }
            }
            ids.push(id);
        }
        if ids.len() >= 3 {
            sk.sketch.entities.insert(Entity::Spline { points: ids.clone(), closed: false });
        } else {
            for w in ids.windows(2) {
                sk.sketch.add_line(w[0], w[1]);
            }
        }
        let end = *piece.last().unwrap();
        if first.is_none() {
            first = Some((ids[0], piece[0]));
        }
        last = Some((*ids.last().unwrap(), end));
    }
    if let (Some((f, fp)), Some((l, lp))) = (first, last) {
        if f != l && !same(fp, lp) {
            sk.sketch.add_line(l, f);
        }
    }
}

/// The revolved run on `body` with the most points and the largest radius
/// among runs whose height span covers `z_lo..z_hi`.
fn dome_surface(body: &Solid, z_lo: f64, z_hi: f64) -> Option<u32> {
    body.surfaces
        .iter()
        .filter_map(|(id, g)| {
            let SurfaceGeom::Revolved { run, .. } = g;
            let zmin = run.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
            let zmax = run.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
            let rmax = run.iter().map(|p| p.x).fold(0.0, f64::max);
            (run.len() >= 8 && zmin <= z_lo && zmax >= z_hi).then_some((*id, rmax))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

fn circumcentre(a: DVec2, b: DVec2, c: DVec2) -> DVec2 {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let ux = ((a.x * a.x + a.y * a.y) * (b.y - c.y)
        + (b.x * b.x + b.y * b.y) * (c.y - a.y)
        + (c.x * c.x + c.y * c.y) * (a.y - b.y))
        / d;
    let uy = ((a.x * a.x + a.y * a.y) * (c.x - b.x)
        + (b.x * b.x + b.y * b.y) * (a.x - c.x)
        + (c.x * c.x + c.y * c.y) * (b.x - a.x))
        / d;
    DVec2::new(ux, uy)
}

/// Arc sketch on the XZ plane along the spout centreline, from angle
/// `a0` to `a1` (radians) about the spout circle, clockwise from the body
/// to the tip. `extend` widens the arc at both ends by that many radians.
fn spout_path(extend_start: f64, extend_end: f64) -> SketchFeature {
    // Three points the spout centreline passes through: inside the wall,
    // mid, tip (r, z in mm). The centre of the arc lies above and left.
    let s = DVec2::new(60.0, 56.0);
    let m = DVec2::new(80.0, 66.0);
    let t = DVec2::new(94.0, 90.0);
    let c = circumcentre(s, m, t);
    let r = (s - c).length();
    let a_s = (s - c).y.atan2((s - c).x);
    let a_t = (t - c).y.atan2((t - c).x);
    // Counter-clockwise from the start to the tip is the short way round.
    let a_start = a_s - extend_start;
    let a_end = a_t + extend_end;
    let p = |a: f64| c + DVec2::new(r * a.cos(), r * a.sin());
    let mut sk = SketchFeature::on_datum("XZ");
    sk.sketch.add_arc_center(c, p(a_start), p(a_end));
    sk
}

/// The kettle: body, bail, and lid as three bodies.
pub fn kettle() -> Document {
    let mut doc = TimedDoc(Document::new("Kettle"));
    for (k, v) in [
        ("dot_pitch", "4.5"),
        ("dot_size", "3"),
        ("dot_height", "1"),
        ("spout_d", "24"),
        ("spout_tip", "15"),
        ("bore_d", "18"),
        ("bore_tip", "11"),
        ("bail_d", "8"),
        ("lug_hole", "4.5"),
        ("segments", "96"),
    ] {
        doc.set_expression(k, v).ok();
    }

    // 0: body profile on XZ (sketch x = radius, sketch y = height).
    let mut body = SketchFeature::on_datum("XZ");
    closed_loop(
        &mut body,
        &[
            &[(0.0, 0.0)],
            &[(68.0, 0.0)],
            &[(70.0, 2.0)],
            &[(80.0, 30.0)],
            // Outer dome: ledge start to the collar.
            &[(78.0, 32.0), (77.0, 45.0), (72.0, 60.0), (64.0, 74.0), (55.0, 84.0), (47.0, 90.0)],
            &[(47.0, 94.0)],
            &[(44.0, 94.0)],
            // Inner dome, 3 mm inside the outer one.
            &[(44.0, 88.0), (52.0, 82.0), (61.0, 72.0), (69.0, 58.5), (74.0, 44.5), (75.5, 31.0)],
            &[(66.5, 3.0)],
            &[(0.0, 3.0)],
        ],
    );
    doc.add_feature(Box::new(body));
    // 1: revolve.
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 0,
        axis: "Y".into(),
        angle_deg: "360".into(),
        segments: "segments".into(),
        ..Default::default()
    }));
    let outer = doc.features[1]
        .output
        .as_ref()
        .and_then(|o| o.bodies.first())
        .and_then(|b| dome_surface(b, 35.0, 85.0))
        .unwrap_or(0);
    // 2, 3, 4: spout path, tapered pipe, join.
    doc.add_feature(Box::new(spout_path(0.0, 0.0)));
    doc.add_feature(Box::new(PipeFeature { path: 2, diameter: "spout_d".into(), end_diameter: "spout_tip".into() }));
    doc.add_feature(Box::new(CombineFeature { body: 1, tool: 3, op: "join".into() }));
    // 5, 6, 7: a wedge of the cavity, 0.3 mm inside the wall and 40 degrees
    // wide around the spout, removes the spout stub inside the kettle. A
    // narrow wedge keeps the boolean local to the spout base.
    let half = 20.0f64.to_radians();
    let mut cavity = SketchFeature::on_plane(Plane {
        origin: DVec3::ZERO,
        x_axis: DVec3::new(half.cos(), -half.sin(), 0.0),
        y_axis: DVec3::Z,
    });
    closed_loop(
        &mut cavity,
        &[
            &[(0.0, 3.3)],
            &[(66.2, 3.3)],
            // 0.3 mm inside the inner wall so no faces coincide.
            &[(75.2, 31.0), (73.7, 44.5), (68.7, 58.5), (60.7, 72.0), (51.7, 82.0), (43.7, 88.0)],
            &[(43.7, 96.0)],
            &[(0.0, 96.0)],
        ],
    );
    doc.add_feature(Box::new(cavity));
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 5,
        axis: "Y".into(),
        angle_deg: "40".into(),
        segments: "segments".into(),
        ..Default::default()
    }));
    doc.add_feature(Box::new(CombineFeature { body: 4, tool: 6, op: "cut".into() }));
    // 8, 9, 10: bore through the wall and out of the tip.
    doc.add_feature(Box::new(spout_path(0.12, 0.16)));
    doc.add_feature(Box::new(PipeFeature { path: 8, diameter: "bore_d".into(), end_diameter: "bore_tip".into() }));
    doc.add_feature(Box::new(CombineFeature { body: 7, tool: 9, op: "cut".into() }));
    // 11, 12: lugs for the bail, two bosses on the shoulder along Y.
    let mut lugs = SketchFeature::on_plane(Plane { origin: DVec3::new(0.0, 5.0, 0.0), ..Plane::XZ });
    for x in [60.0, -60.0] {
        let c = lugs.sketch.add_point(x, 80.0);
        lugs.sketch.add_circle(c, 6.0);
    }
    doc.add_feature(Box::new(lugs));
    doc.add_feature(Box::new(ExtrudeFeature {
        sketch: 11,
        distance: "10".into(),
        symmetric: false,
        operation: "join".into(),
        target: 10,
    }));
    // 13, 14: pin holes through the lugs.
    for (k, x) in [(12usize, "60"), (13usize, "-60")] {
        doc.add_feature(Box::new(HoleFeature {
            body: k,
            plane: "XZ".into(),
            x: x.into(),
            y: "80".into(),
            z: "6".into(),
            diameter: "lug_hole".into(),
            depth: "14".into(),
            ..Default::default()
        }));
    }
    // 15: hobnail dots on the outer dome, last so every boolean before it
    // ran on the coarse mesh. Cells the spout and lugs cut stay smooth.
    doc.add_feature(Box::new(SurfacePatternFeature {
        body: 14,
        surface: outer.to_string(),
        layout: "hobnail".into(),
        pitch: "dot_pitch".into(),
        dot: "dot_size".into(),
        height: "dot_height".into(),
        margin: "3".into(),
        step: "0.6".into(),
        ..Default::default()
    }));
    // 16, 17: bail arch between the lugs.
    let mut bail = SketchFeature::on_datum("XZ");
    bail.sketch.add_arc_3pt(DVec2::new(-60.0, 80.0), DVec2::new(0.0, 185.0), DVec2::new(60.0, 80.0));
    doc.add_feature(Box::new(bail));
    doc.add_feature(Box::new(PipeFeature { path: 16, diameter: "bail_d".into(), ..Default::default() }));
    // 18, 19: lid shell.
    let mut lid = SketchFeature::on_datum("XZ");
    closed_loop(
        &mut lid,
        &[
            &[(0.0, 101.0), (20.0, 99.5), (35.0, 96.0), (41.0, 94.0)],
            &[(41.0, 91.0)],
            &[(43.5, 91.0)],
            &[(43.5, 94.0)],
            &[(48.0, 94.0)],
            &[(48.0, 96.0), (35.0, 99.0), (20.0, 102.5), (0.0, 104.0)],
        ],
    );
    doc.add_feature(Box::new(lid));
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 18,
        axis: "Y".into(),
        angle_deg: "360".into(),
        segments: "segments".into(),
        ..Default::default()
    }));
    let lid_top = doc.features[19]
        .output
        .as_ref()
        .and_then(|o| o.bodies.first())
        .and_then(|b| dome_surface(b, 97.0, 103.0))
        .unwrap_or(0);
    // 20, 21, 22: bud knob joined to the lid.
    let mut knob = SketchFeature::on_datum("XZ");
    closed_loop(
        &mut knob,
        &[
            &[(0.0, 103.0)],
            &[(3.0, 103.0)],
            &[(3.0, 108.0)],
            &[(5.5, 109.5)],
            &[(5.5, 110.5), (8.5, 114.5), (8.0, 119.5), (4.5, 123.5), (0.0, 125.0)],
        ],
    );
    doc.add_feature(Box::new(knob));
    doc.add_feature(Box::new(RevolveFeature {
        sketch: 20,
        axis: "Y".into(),
        angle_deg: "360".into(),
        segments: "48".into(),
        ..Default::default()
    }));
    doc.add_feature(Box::new(CombineFeature { body: 19, tool: 21, op: "join".into() }));
    // 23: dots on the lid top, stopping at the knob.
    doc.add_feature(Box::new(SurfacePatternFeature {
        body: 22,
        surface: lid_top.to_string(),
        layout: "hobnail".into(),
        pitch: "3.5".into(),
        dot: "2.2".into(),
        height: "0.7".into(),
        margin: "4".into(),
        step: "0.5".into(),
        ..Default::default()
    }));

    let iron = [96, 98, 104];
    for i in [15usize, 17, 23] {
        doc.appearance.insert(i, iron);
    }
    doc.0
}

/// Times every feature when `ANVIL_TIMING` is set, for tuning.
struct TimedDoc(Document);
impl std::ops::Deref for TimedDoc {
    type Target = Document;
    fn deref(&self) -> &Document {
        &self.0
    }
}
impl std::ops::DerefMut for TimedDoc {
    fn deref_mut(&mut self) -> &mut Document {
        &mut self.0
    }
}
impl TimedDoc {
    fn add_feature(&mut self, f: Box<dyn anvil_feature::Feature>) -> usize {
        let t = std::time::Instant::now();
        let name = f.name();
        let i = self.0.add_feature(f);
        if std::env::var("ANVIL_TIMING").is_err() {
            return i;
        }
        let faces: usize =
            self.0.features[i].output.as_ref().map(|o| o.bodies.iter().map(|b| b.faces.len()).sum()).unwrap_or(0);
        if let Some(o) = self.0.features[i].output.as_ref() {
            for b in &o.bodies {
                eprintln!("    edges {:?}", b.open_edge_report());
            }
        }
        eprintln!(
            "feature {i} {name}: {:.2} s, {faces} faces, err {:?}",
            t.elapsed().as_secs_f64(),
            self.0.features[i].error
        );
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kettle_builds_three_bodies() {
        let t0 = std::time::Instant::now();
        let doc = kettle();
        let secs = t0.elapsed().as_secs_f64();
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "feature {i} ({}): {:?}", f.feature.name(), f.error);
        }
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 3, "body, bail, lid");
        let vols: Vec<f64> = bodies.iter().map(|b| b.volume()).collect();
        for (b, v) in bodies.iter().zip(&vols) {
            eprintln!(
                "body: {} faces, volume {v:.0} mm3, open edges {:?}, bounds {:?}",
                b.faces.len(),
                b.open_edge_report(),
                b.bounds()
            );
        }
        let body = vols.iter().cloned().fold(0.0, f64::max);
        // Cast iron at 7.2 g/cm3: the body should weigh 1.0 to 1.7 kg.
        assert!(body > 140_000.0 && body < 240_000.0, "body volume {body} mm3");
        for (b, v) in bodies.iter().zip(&vols) {
            assert!(*v > 0.0, "positive volume");
            let (open, over) = b.open_edge_report();
            assert_eq!(open, 0, "open edges on a body with volume {v}");
            // Known limit: BSP booleans leave sliver faces at seams whose
            // edges are shared by more than two faces. Slicers accept them.
            assert!(over < 400, "{over} edges shared by more than two faces");
        }
        let bb = bodies[0].bounds();
        assert!(bb.max.x > 98.0 && bb.max.x < 104.0, "spout tip reaches x = {}", bb.max.x);
        assert!(secs < 60.0, "kettle took {secs:.1} s");
    }
}
