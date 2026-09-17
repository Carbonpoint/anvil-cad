//! Revolve: sweep a sketch profile about an axis in the sketch plane.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_math::{Axis, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevolveFeature {
    pub sketch: usize,
    /// "X" or "Y": the sketch axis to revolve about, through the sketch origin.
    pub axis: String,
    pub angle_deg: String,
    #[serde(default = "crate::features::extrude::default_op")]
    pub operation: String,
    #[serde(default)]
    pub target: usize,
    /// Facets per full turn. More facets give a rounder body; a surface
    /// pattern refines the facets further on its own.
    #[serde(default = "default_segments")]
    pub segments: String,
}

fn default_segments() -> String {
    "64".into()
}

impl Default for RevolveFeature {
    fn default() -> Self {
        RevolveFeature {
            sketch: 0,
            axis: "Y".into(),
            angle_deg: "360".into(),
            operation: "new".into(),
            target: 0,
            segments: default_segments(),
        }
    }
}

#[typetag::serde(name = "revolve")]
impl Feature for RevolveFeature {
    fn kind(&self) -> &'static str {
        "revolve"
    }
    fn name(&self) -> String {
        format!("Revolve ({} deg)", self.angle_deg)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::feature_ref("sketch", "Sketch", vec!["sketch"], self.sketch),
            ParamSpec::choice("axis", "Axis", vec!["X", "Y"], &self.axis),
            ParamSpec::angle("angle", "Angle", &self.angle_deg),
            ParamSpec::choice("operation", "Operation", vec!["new", "join", "cut", "intersect"], &self.operation),
            ParamSpec::length("segments", "Segments per turn", &self.segments),
        ];
        if self.operation != "new" {
            v.push(ParamSpec::feature_ref("target", "Target body", crate::BODY_TYPES.to_vec(), self.target));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("sketch", ParamValue::FeatureRef(i)) => self.sketch = i,
            ("axis", ParamValue::Choice(a)) => self.axis = a,
            ("angle", ParamValue::Expr(s)) => self.angle_deg = s,
            ("operation", ParamValue::Choice(o)) => self.operation = o,
            ("target", ParamValue::FeatureRef(i)) => self.target = i,
            ("segments", ParamValue::Expr(s)) => self.segments = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let angle = ctx.eval(&self.angle_deg)?.to_radians();
        let (plane, profiles) = ctx.profiles_of(self.sketch)?;
        let dir2 = if self.axis == "X" { DVec2::X } else { DVec2::Y };
        let axis = Axis::new(plane.origin, plane.to_world(dir2) - plane.origin);
        let segments = ctx.eval(&self.segments)?.round().clamp(6.0, 4096.0) as usize;
        let mut bodies = Vec::new();
        for p in profiles {
            bodies.push(ctx.kernel.revolve_n(plane, &p.points, &axis, angle, segments)?);
        }
        crate::features::extrude::apply_operation(ctx, &self.operation, self.target, bodies)
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "revolve",
        label: "Revolve",
        tab: "Solid",
        group: "Create",
        tooltip: "Revolve a sketch profile about an axis",
        order: 20,
        create: || Box::new(RevolveFeature::default()),
    }
}
