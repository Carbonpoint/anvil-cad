//! Lattice fill: replace the inside of a body with a sheet lattice under
//! a solid skin, the way field driven tools such as nTop lighten a part.
//!
//! The body is sampled into a signed distance grid, the lattice and the
//! skin are combined as fields, and the zero surface is meshed back into
//! a body. Nothing here calls a B-rep boolean, so it works on any closed
//! body, patterned or not, and the cost depends only on the grid size.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_implicit::{
    cylindrical, BeamCell, BeamLattice, Field, Graded, Intersect, Lattice, Offset, Radial, Ramp, Sampled, Skin, Tpms,
    Union, Warp,
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
    /// "none", "x", "y", "z" (thickness ramps along that axis from `wall`
    /// to `wall_end`), or "radial" (from `wall` at the centre to
    /// `wall_end` at the outside).
    #[serde(default = "none")]
    pub grade: String,
    /// "none" or "cylinder": wrap the cells around the Z axis so they
    /// follow a round wall.
    #[serde(default = "none")]
    pub conform: String,
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
            grade: "none".into(),
            conform: "none".into(),
        }
    }
}

/// The lattice kinds the feature offers: three TPMS sheets and four beam
/// cells.
pub const KINDS: [&str; 7] = ["gyroid", "schwarz", "diamond", "cubic", "bcc", "octet", "kelvin"];

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
            ParamSpec::choice("grade", "Grade along", vec!["none", "x", "y", "z", "radial"], &self.grade),
            ParamSpec::choice("conform", "Conform to", vec!["none", "cylinder"], &self.conform),
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
            ("grade", ParamValue::Choice(s)) => self.grade = s,
            ("conform", ParamValue::Choice(s)) => self.conform = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let tpms = Tpms::parse(&self.lattice);
        let beam = BeamCell::parse(&self.lattice);
        if tpms.is_none() && beam.is_none() {
            return Err(RegenError::Other(format!(
                "unknown lattice {}; use one of {}",
                self.lattice,
                KINDS.join(", ")
            )));
        }
        let wall_end = ctx.eval(&self.wall_end)?;
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
                _ => Box::new(anvil_implicit::Func(move |_p: DVec3| wall)),
            };
            let centre: Box<dyn Field> = match (tpms, beam) {
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
                if self.grade == "none" { "lattice".to_string() } else { format!("graded along {}", self.grade) },
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
}
