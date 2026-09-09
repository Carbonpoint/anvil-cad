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
}

impl Default for RevolveFeature {
    fn default() -> Self {
        RevolveFeature { sketch: 0, axis: "Y".into(), angle_deg: "360".into() }
    }
}

#[typetag::serde(name = "revolve")]
impl Feature for RevolveFeature {
    fn type_id(&self) -> &'static str {
        "revolve"
    }
    fn name(&self) -> String {
        format!("Revolve ({} deg)", self.angle_deg)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("sketch", "Sketch", vec!["sketch"], self.sketch),
            ParamSpec::choice("axis", "Axis", vec!["X", "Y"], &self.axis),
            ParamSpec::angle("angle", "Angle", &self.angle_deg),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("sketch", ParamValue::FeatureRef(i)) => self.sketch = i,
            ("axis", ParamValue::Choice(a)) => self.axis = a,
            ("angle", ParamValue::Expr(s)) => self.angle_deg = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let angle = ctx.eval(&self.angle_deg)?.to_radians();
        let (plane, profiles) = ctx.profiles_of(self.sketch)?;
        let dir2 = if self.axis == "X" { DVec2::X } else { DVec2::Y };
        let axis = Axis::new(plane.origin, plane.to_world(dir2) - plane.origin);
        let mut bodies = Vec::new();
        for p in profiles {
            bodies.push(ctx.kernel.revolve(plane, &p.points, &axis, angle)?);
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "revolve",
        label: "Revolve",
        tab: "Home",
        group: "Feature",
        tooltip: "Revolve a sketch profile about an axis",
        order: 20,
        create: || Box::new(RevolveFeature::default()),
    }
}
