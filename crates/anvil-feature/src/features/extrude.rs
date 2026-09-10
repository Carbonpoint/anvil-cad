//! Extrude: sweep a sketch profile along the sketch plane normal.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    pub sketch: usize,
    pub distance: String,
    pub symmetric: bool,
}

impl Default for ExtrudeFeature {
    fn default() -> Self {
        ExtrudeFeature { sketch: 0, distance: "10".into(), symmetric: false }
    }
}

#[typetag::serde(name = "extrude")]
impl Feature for ExtrudeFeature {
    fn kind(&self) -> &'static str {
        "extrude"
    }
    fn name(&self) -> String {
        format!("Extrude ({})", self.distance)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("sketch", "Sketch", vec!["sketch"], self.sketch),
            ParamSpec::length("distance", "Distance", &self.distance),
            ParamSpec::boolean("symmetric", "Symmetric", self.symmetric),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("sketch", ParamValue::FeatureRef(i)) => self.sketch = i,
            ("distance", ParamValue::Expr(s)) => self.distance = s,
            ("symmetric", ParamValue::Bool(b)) => self.symmetric = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let d = ctx.eval(&self.distance)?;
        let (plane, profiles) = ctx.profiles_of(self.sketch)?;
        let mut plane = *plane;
        let mut dist = d;
        if self.symmetric {
            plane.origin -= plane.normal() * (d / 2.0);
            dist = d.abs();
        }
        let mut bodies = Vec::new();
        for p in profiles {
            bodies.push(ctx.kernel.extrude(&plane, &p.points, dist)?);
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "extrude",
        label: "Extrude",
        tab: "Solid",
        group: "Create",
        tooltip: "Extrude a sketch profile along its normal",
        order: 10,
        create: || Box::new(ExtrudeFeature::default()),
    }
}
