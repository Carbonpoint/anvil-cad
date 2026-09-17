//! Lattice fill: replace the inside of a body with a sheet lattice under
//! a solid skin, the way field driven tools such as nTop lighten a part.
//!
//! The body is sampled into a signed distance grid, the lattice and the
//! skin are combined as fields, and the zero surface is meshed back into
//! a body. Nothing here calls a B-rep boolean, so it works on any closed
//! body, patterned or not, and the cost depends only on the grid size.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_implicit::{Intersect, Lattice, Offset, Sampled, Skin, Tpms, Union};
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
        }
    }
}

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
            ParamSpec::choice("lattice", "Lattice", Tpms::NAMES.to_vec(), &self.lattice),
            ParamSpec::length("cell", "Cell size", &self.cell),
            ParamSpec::length("wall", "Wall thickness", &self.wall),
            ParamSpec::length("skin", "Skin thickness", &self.skin),
            ParamSpec::length("resolution", "Resolution", &self.resolution),
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
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let kind = Tpms::parse(&self.lattice).ok_or_else(|| {
            RegenError::Other(format!("unknown lattice {}; use gyroid, schwarz, or diamond", self.lattice))
        })?;
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
            let lattice = Lattice { kind, cell, wall };
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
                "{} triangles, {:.0} percent of the solid volume, {} voxels",
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
}
