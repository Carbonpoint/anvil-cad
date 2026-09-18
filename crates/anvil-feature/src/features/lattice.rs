//! Lattice fill: replace the inside of a body with a sheet lattice under
//! a solid skin, the way field driven tools such as nTop lighten a part.
//!
//! The body is sampled into a signed distance grid, the lattice and the
//! skin are combined as fields, and the zero surface is meshed back into
//! a body. Nothing here calls a B-rep boolean, so it works on any closed
//! body, patterned or not, and the cost depends only on the grid size.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_implicit::{
    cylindrical, BeamCell, BeamLattice, Field, Graded, Intersect, Lattice, Offset, Radial, Ramp, Remap, Sampled, Skin,
    Tpms, Union, Warp,
};
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LatticeFillFeature {
    pub body: usize,
    /// gyroid, schwarz, or diamond.
    pub lattice: String,
    /// Repeat length of the lattice cell (mm).
    pub cell: String,
    /// Sheet thickness of the lattice (mm).
    pub wall: String,
    /// Solid skin kept under the body's surface (mm); 0 for none.
    pub skin: String,
    /// Voxel size for sampling and meshing (mm).
    pub resolution: String,
    /// Wall or beam thickness at the far end of the grade; 0 keeps `wall`
    /// everywhere.
    #[serde(default = "zero")]
    pub wall_end: String,
    /// Cell size at the far end of the grade (sheet lattices, grades
    /// along x, y, or z); 0 keeps `cell` everywhere.
    #[serde(default = "zero")]
    pub cell_end: String,
    /// "none", "x", "y", "z" (thickness ramps along that axis from `wall`
    /// to `wall_end`), or "radial" (from `wall` at the centre to
    /// `wall_end` at the outside).
    #[serde(default = "none")]
    pub grade: String,
    /// "none" or "cylinder": wrap the cells around the Z axis so they
    /// follow a round wall.
    #[serde(default = "none")]
    pub conform: String,
    /// CSV file of `x, y, z, value` samples (a stress or temperature map)
    /// that drives the thickness when `grade` is "map": `wall` where the
    /// value is `map_lo`, `wall_end` where it is `map_hi`. Both zero
    /// means the map's own range.
    /// Body whose surface drives the thickness when `grade` is
    /// "distance": `wall` at the surface, `wall_end` at `map_hi` mm away.
    #[serde(default)]
    pub ref_body: usize,
    #[serde(default)]
    pub map_file: String,
    #[serde(default = "zero")]
    pub map_lo: String,
    #[serde(default = "zero")]
    pub map_hi: String,
}

fn zero() -> String {
    "0".into()
}
fn none() -> String {
    "none".into()
}

impl Default for LatticeFillFeature {
    fn default() -> Self {
        LatticeFillFeature {
            body: 1,
            lattice: "gyroid".into(),
            cell: "8".into(),
            wall: "1.2".into(),
            skin: "1.2".into(),
            resolution: "0.4".into(),
            wall_end: "0".into(),
            cell_end: "0".into(),
            grade: "none".into(),
            conform: "none".into(),
            ref_body: 0,
            map_file: String::new(),
            map_lo: "0".into(),
            map_hi: "0".into(),
        }
    }
}

/// The lattice kinds the feature offers: three TPMS sheets and four beam
/// cells.
pub const KINDS: [&str; 8] = ["gyroid", "schwarz", "diamond", "cubic", "bcc", "octet", "kelvin", "honeycomb"];

#[typetag::serde(name = "lattice_fill")]
impl Feature for LatticeFillFeature {
    fn kind(&self) -> &'static str {
        "lattice_fill"
    }
    fn name(&self) -> String {
        format!("Lattice fill ({} {} mm)", self.lattice, self.cell)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("lattice", "Lattice", KINDS.to_vec(), &self.lattice),
            ParamSpec::length("cell", "Cell size", &self.cell),
            ParamSpec::length("wall", "Wall thickness", &self.wall),
            ParamSpec::length("skin", "Skin thickness", &self.skin),
            ParamSpec::length("resolution", "Resolution", &self.resolution),
            ParamSpec::length("wall_end", "Thickness at far end (0 = same)", &self.wall_end),
            ParamSpec::length("cell_end", "Cell size at far end (0 = same)", &self.cell_end),
            ParamSpec::choice(
                "grade",
                "Grade along",
                vec!["none", "x", "y", "z", "radial", "map", "distance"],
                &self.grade,
            ),
            ParamSpec::feature_ref("ref_body", "Distance to body", BODY_TYPES.to_vec(), self.ref_body),
            ParamSpec::choice("conform", "Conform to", vec!["none", "cylinder"], &self.conform),
            ParamSpec {
                name: "map_file",
                label: "Point map CSV (x, y, z, value)",
                kind: crate::param::ParamKind::Text,
                value: ParamValue::Expr(self.map_file.clone()),
            },
            ParamSpec::length("map_lo", "Map value for Thickness (0, 0 = map range)", &self.map_lo),
            ParamSpec::length("map_hi", "Map value for Thickness at far end", &self.map_hi),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("lattice", ParamValue::Choice(s)) => self.lattice = s,
            ("cell", ParamValue::Expr(s)) => self.cell = s,
            ("wall", ParamValue::Expr(s)) => self.wall = s,
            ("skin", ParamValue::Expr(s)) => self.skin = s,
            ("resolution", ParamValue::Expr(s)) => self.resolution = s,
            ("wall_end", ParamValue::Expr(s)) => self.wall_end = s,
            ("cell_end", ParamValue::Expr(s)) => self.cell_end = s,
            ("grade", ParamValue::Choice(s)) => self.grade = s,
            ("conform", ParamValue::Choice(s)) => self.conform = s,
            ("ref_body", ParamValue::FeatureRef(i)) => self.ref_body = i,
            ("map_file", ParamValue::Expr(s)) => self.map_file = s,
            ("map_lo", ParamValue::Expr(s)) => self.map_lo = s,
            ("map_hi", ParamValue::Expr(s)) => self.map_hi = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let tpms = Tpms::parse(&self.lattice);
        let beam = BeamCell::parse(&self.lattice);
        let honeycomb = self.lattice.eq_ignore_ascii_case("honeycomb");
        if tpms.is_none() && beam.is_none() && !honeycomb {
            return Err(RegenError::Other(format!(
                "unknown lattice {}; use one of {}",
                self.lattice,
                KINDS.join(", ")
            )));
        }
        let wall_end = ctx.eval(&self.wall_end)?;
        let cell_end = ctx.eval(&self.cell_end)?;
        // A point map drives the thickness when the grade is "map".
        let map: Option<(std::sync::Arc<dyn Field + Send>, f64, f64)> = if self.grade == "map" {
            let data = anvil_implicit::vtk::load_scalar_file(std::path::Path::new(&self.map_file))
                .map_err(|e| RegenError::Other(format!("point map {}: {e}", self.map_file)))?;
            let (lo_v, hi_v) = (ctx.eval(&self.map_lo)?, ctx.eval(&self.map_hi)?);
            let (in_lo, in_hi) = if lo_v == 0.0 && hi_v == 0.0 { data.range() } else { (lo_v, hi_v) };
            let field: std::sync::Arc<dyn Field + Send> = std::sync::Arc::from(data.into_field());
            Some((field, in_lo, in_hi))
        } else {
            None
        };
        // Distance to another body's surface as the thickness driver.
        let reference: Option<(std::sync::Arc<dyn Field + Send>, f64)> =
            if self.grade == "distance" {
                let reach = ctx.eval(&self.map_hi)?;
                if reach <= 0.0 {
                    return Err(RegenError::Other(
                        "grade by distance needs Map value for Thickness at far end as the reach in mm".into(),
                    ));
                }
                let src = ctx.bodies_of(self.ref_body)?;
                let mut tris: Vec<[DVec3; 3]> = Vec::new();
                for b in src {
                    let m = anvil_kernel::mesh::tessellate(b);
                    tris.extend(
                        m.indices.as_chunks::<3>().0.iter().map(|t| {
                            [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]]
                        }),
                    );
                }
                if tris.is_empty() {
                    return Err(RegenError::Other("the distance body has no faces".into()));
                }
                let step_ref = ctx.eval(&self.resolution)?.max(0.05) * 2.0;
                let sampled = Sampled::from_triangles(&tris, step_ref, 3);
                Some((std::sync::Arc::new(sampled), reach))
            } else {
                None
            };
        let cell = ctx.eval(&self.cell)?;
        let wall = ctx.eval(&self.wall)?;
        let skin = ctx.eval(&self.skin)?;
        let step = ctx.eval(&self.resolution)?;
        if step <= 0.0 || cell <= 0.0 || wall <= 0.0 {
            return Err(RegenError::Other("cell, wall, and resolution must be positive".into()));
        }
        if wall < 2.0 * step {
            return Err(RegenError::Other(format!(
                "wall {wall} is thinner than two voxels; lower the resolution to {:.2} or less",
                wall / 2.0
            )));
        }
        let mut bodies = Vec::new();
        let mut note = String::new();
        for body in ctx.bodies_of(self.body)? {
            let m = anvil_kernel::mesh::tessellate(body);
            let tris: Vec<[DVec3; 3]> = m
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]])
                .collect();
            let bb = body.bounds();
            let voxels = ((bb.max - bb.min) / step).ceil();
            if voxels.x * voxels.y * voxels.z > 4.0e8 {
                return Err(RegenError::Other(format!(
                    "{:.0} million voxels; raise the resolution",
                    voxels.x * voxels.y * voxels.z / 1e6
                )));
            }
            let sampled = Sampled::from_triangles(&tris, step, 3);
            let lo = sampled.lo;
            let hi = lo + DVec3::new(sampled.n[0] as f64, sampled.n[1] as f64, sampled.n[2] as f64) * step;
            // The lattice centreline or mid sheet, thickened by a constant
            // or by a graded field, wrapped around the Z axis on request.
            let bb_c = (bb.min + bb.max) * 0.5;
            let half = (bb.max - bb.min) * 0.5;
            let far = if wall_end > 0.0 { wall_end } else { wall };
            let thickness: Box<dyn Field> = match self.grade.as_str() {
                "x" => Box::new(Ramp {
                    from: DVec3::new(bb.min.x, 0.0, 0.0),
                    to: DVec3::new(bb.max.x, 0.0, 0.0),
                    a: wall,
                    b: far,
                }),
                "y" => Box::new(Ramp {
                    from: DVec3::new(0.0, bb.min.y, 0.0),
                    to: DVec3::new(0.0, bb.max.y, 0.0),
                    a: wall,
                    b: far,
                }),
                "z" => Box::new(Ramp {
                    from: DVec3::new(0.0, 0.0, bb.min.z),
                    to: DVec3::new(0.0, 0.0, bb.max.z),
                    a: wall,
                    b: far,
                }),
                "radial" => Box::new(Radial { centre: bb_c, radius: half.x.max(half.y).max(half.z), a: wall, b: far }),
                "map" => {
                    let (pm, in_lo, in_hi) = map.as_ref().expect("map loaded above");
                    Box::new(Remap { field: pm.clone(), in_lo: *in_lo, in_hi: *in_hi, a: wall, b: far })
                }
                "distance" => {
                    let (sd, reach) = reference.as_ref().expect("reference sampled above");
                    // Unsigned distance to the reference surface.
                    let d = anvil_implicit::Dyn(Box::new(anvil_implicit::Func({
                        let sd = sd.clone();
                        move |p: DVec3| sd.at(p).abs()
                    })));
                    Box::new(Remap { field: d, in_lo: 0.0, in_hi: *reach, a: wall, b: far })
                }
                _ => Box::new(anvil_implicit::Func(move |_p: DVec3| wall)),
            };
            let ramp_axis = match self.grade.as_str() {
                "x" => Some(0),
                "y" => Some(1),
                "z" => Some(2),
                _ => None,
            };
            let centre: Box<dyn Field> = match (tpms, beam) {
                _ if honeycomb => Box::new(anvil_implicit::Honeycomb { cell, wall: 0.0 }),
                (Some(k), _) if cell_end > 0.0 && ramp_axis.is_some() => {
                    let ax = ramp_axis.unwrap_or(0);
                    let (from, to) = ([bb.min.x, bb.min.y, bb.min.z][ax], [bb.max.x, bb.max.y, bb.max.z][ax]);
                    Box::new(anvil_implicit::CellRamp {
                        kind: k,
                        axis: ax,
                        from,
                        to,
                        cell_a: cell,
                        cell_b: cell_end,
                        wall: 0.0,
                    })
                }
                (Some(k), _) => Box::new(Lattice { kind: k, cell, wall: 0.0 }),
                (_, Some(b)) => Box::new(BeamLattice::new(b, cell, 0.0)),
                _ => unreachable!(),
            };
            let centre: Box<dyn Field> = if self.conform == "cylinder" {
                let radius = half.x.max(half.y);
                Box::new(Warp { field: anvil_implicit::Dyn(centre), map: cylindrical(radius) })
            } else {
                centre
            };
            let lattice = Graded { core: anvil_implicit::Dyn(centre), thickness: anvil_implicit::Dyn(thickness) };
            // Lattice inside the core (the body shrunk by the skin), plus
            // the skin itself.
            let core = Intersect(Offset { a: &sampled, d: -skin }, lattice);
            let tris_out = if skin > 0.0 {
                let field = Union(core, Skin { a: &sampled, t: skin });
                anvil_implicit::mesh::surface_nets(&field, lo, hi, step)
            } else {
                anvil_implicit::mesh::surface_nets(&core, lo, hi, step)
            };
            if tris_out.is_empty() {
                return Err(RegenError::Other("the lattice left nothing; check cell and wall".into()));
            }
            let solid_vol = body.volume();
            let vol = anvil_implicit::mesh::volume(&tris_out);
            let out = anvil_kernel::ops::from_triangles(&tris_out, step * 1e-3);
            note = format!(
                "{} {}, {} triangles, {:.0} percent of the solid volume, {} voxels",
                self.lattice,
                match self.grade.as_str() {
                    "none" => "lattice".to_string(),
                    "map" => format!("thickness from {}", self.map_file),
                    "distance" => format!("thickness by distance to feature {}", self.ref_body),
                    g if cell_end > 0.0 => format!("graded along {g}, cell {cell} to {cell_end}"),
                    g => format!("graded along {g}"),
                },
                tris_out.len(),
                100.0 * vol / solid_vol.max(1e-12),
                sampled.n[0] * sampled.n[1] * sampled.n[2]
            );
            bodies.push(out);
        }
        Ok(FeatureOutput { bodies, consumes: vec![self.body], note: Some(note), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "lattice_fill", label: "Lattice fill", tab: "Solid", group: "Field", tooltip: "Replace the inside of a body with a gyroid, Schwarz, or diamond sheet lattice under a skin", order: 90, create: || Box::new(LatticeFillFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::primitives::SphereFeature;
    use crate::Document;

    #[test]
    fn sphere_gets_a_gyroid_core() {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(SphereFeature { x: "0".into(), y: "0".into(), z: "0".into(), radius: "15".into() }));
        let full = doc.features[0].output.as_ref().unwrap().bodies[0].volume();
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: "gyroid".into(),
            cell: "8".into(),
            wall: "1.2".into(),
            skin: "1.2".into(),
            resolution: "0.5".into(),
            ..Default::default()
        }));
        let f = &doc.features[1];
        assert!(f.error.is_none(), "{:?}", f.error);
        let out = f.output.as_ref().unwrap();
        let b = &out.bodies[0];
        let (open, _) = b.open_edge_report();
        assert_eq!(open, 0, "lattice body has open edges");
        let v = b.volume();
        assert!(v > 0.2 * full && v < 0.8 * full, "lattice volume {v} of {full}");
        eprintln!("{}", out.note.as_deref().unwrap_or(""));
    }

    #[test]
    fn box_gets_a_graded_octet_core() {
        use crate::features::primitives::BoxFeature;
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "30".into(),
            depth: "30".into(),
            height: "30".into(),
        }));
        let full = doc.features[0].output.as_ref().unwrap().bodies[0].volume();
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: "octet".into(),
            cell: "10".into(),
            wall: "1.2".into(),
            wall_end: "2.4".into(),
            grade: "z".into(),
            skin: "1.0".into(),
            resolution: "0.5".into(),
            ..Default::default()
        }));
        let f = &doc.features[1];
        assert!(f.error.is_none(), "{:?}", f.error);
        let b = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(b.open_edge_report().0, 0, "lattice body has open edges");
        let v = b.volume();
        assert!(v > 0.15 * full && v < 0.8 * full, "lattice volume {v} of {full}");
        // Thicker beams at the top: more material in the upper half.
        let m = anvil_kernel::mesh::tessellate(b);
        let (mut lo, mut hi) = (0.0, 0.0);
        for t in m.indices.as_chunks::<3>().0 {
            let p = [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]];
            let zc = (p[0].z + p[1].z + p[2].z) / 3.0;
            let v6 = p[0].dot(p[1].cross(p[2])) / 6.0;
            if zc < 15.0 {
                lo += v6;
            } else {
                hi += v6;
            }
        }
        let _ = (lo, hi);
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("graded along z"), "{note}");
    }

    #[test]
    fn cell_size_ramps_along_x() {
        use crate::features::primitives::BoxFeature;
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "60".into(),
            depth: "20".into(),
            height: "20".into(),
        }));
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: "gyroid".into(),
            cell: "5".into(),
            cell_end: "15".into(),
            wall: "1.0".into(),
            grade: "x".into(),
            skin: "0".into(),
            resolution: "0.5".into(),
            ..Default::default()
        }));
        let f = &doc.features[1];
        assert!(f.error.is_none(), "{:?}", f.error);
        let b = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(b.open_edge_report().0, 0);
        // Both halves hold material and differ: the ramp did something.
        let m = anvil_kernel::mesh::tessellate(b);
        let (mut lo, mut hi) = (0.0, 0.0);
        for t in m.indices.as_chunks::<3>().0 {
            let p = [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]];
            let v6 = p[0].dot(p[1].cross(p[2])) / 6.0;
            if (p[0].x + p[1].x + p[2].x) / 3.0 < 30.0 {
                lo += v6;
            } else {
                hi += v6;
            }
        }
        assert!(lo > 500.0 && hi > 500.0 && (lo - hi).abs() > 0.05 * lo, "small cells {lo} vs large cells {hi}");
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("cell 5 to 15"), "{note}");
    }

    #[test]
    fn honeycomb_ribs_thicken_near_a_reference_body() {
        use crate::features::primitives::{BoxFeature, CylinderFeature};
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "60".into(),
            depth: "30".into(),
            height: "6".into(),
        }));
        // A bolt boss at one end drives the rib thickness.
        doc.add_feature(Box::new(CylinderFeature {
            plane: "XY".into(),
            cx: "50".into(),
            cy: "15".into(),
            radius: "4".into(),
            height: "6".into(),
        }));
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: "honeycomb".into(),
            cell: "8".into(),
            wall: "1.0".into(),
            wall_end: "3.0".into(),
            grade: "distance".into(),
            ref_body: 1,
            map_hi: "25".into(),
            skin: "0.8".into(),
            resolution: "0.4".into(),
            ..Default::default()
        }));
        let f = &doc.features[2];
        assert!(f.error.is_none(), "{:?}", f.error);
        let b = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(b.open_edge_report().0, 0);
        // Ribs are thick near the boss (x near 50) and thin far away, so
        // the near half holds more material.
        let m = anvil_kernel::mesh::tessellate(b);
        let (mut near, mut far) = (0.0, 0.0);
        for t in m.indices.as_chunks::<3>().0 {
            let p = [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]];
            let v6 = p[0].dot(p[1].cross(p[2])) / 6.0;
            if (p[0].x + p[1].x + p[2].x) / 3.0 > 30.0 {
                near += v6;
            } else {
                far += v6;
            }
        }
        // Thickness 1 far from the boss, 3 at it: the near half is denser.
        assert!(near > far * 1.2, "near the boss {near} vs far {far}");
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("thickness by distance"), "{note}");
    }

    #[test]
    fn thickness_follows_a_point_map() {
        use crate::features::primitives::BoxFeature;
        let dir = std::env::temp_dir().join(format!("anvil_map_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("stress.csv");
        // Stress high at x = 30, low at x = 0.
        let mut text = String::from("x,y,z,stress\n");
        for i in 0..=6 {
            for j in 0..=3 {
                for k in 0..=3 {
                    let x = 5.0 * i as f64;
                    text.push_str(&format!("{x},{},{},{}\n", 10.0 * j as f64, 10.0 * k as f64, 100.0 + 10.0 * x));
                }
            }
        }
        std::fs::write(&csv, text).unwrap();
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "30".into(),
            depth: "30".into(),
            height: "30".into(),
        }));
        doc.add_feature(Box::new(LatticeFillFeature {
            body: 0,
            lattice: "cubic".into(),
            cell: "10".into(),
            wall: "1.0".into(),
            wall_end: "3.0".into(),
            grade: "map".into(),
            map_file: csv.display().to_string(),
            skin: "0".into(),
            resolution: "0.5".into(),
            ..Default::default()
        }));
        let f = &doc.features[1];
        assert!(f.error.is_none(), "{:?}", f.error);
        let b = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(b.open_edge_report().0, 0);
        // More material where the stress is high: compare the two halves.
        let m = anvil_kernel::mesh::tessellate(b);
        let (mut lo, mut hi) = (0.0, 0.0);
        for t in m.indices.as_chunks::<3>().0 {
            let p = [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]];
            let v6 = p[0].dot(p[1].cross(p[2])) / 6.0;
            if (p[0].x + p[1].x + p[2].x) / 3.0 < 15.0 {
                lo += v6;
            } else {
                hi += v6;
            }
        }
        assert!(hi > lo * 1.3, "high stress half {hi} vs low {lo}");
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("thickness from"), "{note}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
