//! Extrude: sweep a sketch profile along the sketch plane normal.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    pub sketch: usize,
    pub distance: String,
    pub symmetric: bool,
    /// "new", "join", "cut", or "intersect".
    #[serde(default = "default_op")]
    pub operation: String,
    /// Target body feature for join, cut, intersect.
    #[serde(default)]
    pub target: usize,
}

pub(crate) fn default_op() -> String {
    "new".into()
}

impl Default for ExtrudeFeature {
    fn default() -> Self {
        ExtrudeFeature { sketch: 0, distance: "10".into(), symmetric: false, operation: "new".into(), target: 0 }
    }
}

/// Apply a boolean operation string to bodies against a target feature.
pub(crate) fn apply_operation(
    ctx: &RegenContext,
    operation: &str,
    target: usize,
    tools: Vec<anvil_kernel::Solid>,
) -> Result<FeatureOutput, RegenError> {
    use anvil_kernel::BooleanOp;
    let op = match operation {
        "join" => BooleanOp::Union,
        "cut" => BooleanOp::Subtract,
        "intersect" => BooleanOp::Intersect,
        _ => return Ok(FeatureOutput { bodies: tools, ..Default::default() }),
    };
    let targets = ctx.bodies_of(target)?;
    let mut bodies = Vec::new();
    for t in targets {
        let mut acc = t.clone();
        for tool in &tools {
            acc = ctx.kernel.boolean(&acc, tool, op)?;
        }
        bodies.push(acc);
    }
    Ok(FeatureOutput { bodies, consumes: vec![target], ..Default::default() })
}

#[typetag::serde(name = "extrude")]
impl Feature for ExtrudeFeature {
    fn kind(&self) -> &'static str {
        "extrude"
    }
    fn name(&self) -> String {
        if self.operation == "new" {
            format!("Extrude ({})", self.distance)
        } else {
            format!("Extrude {} ({})", self.operation, self.distance)
        }
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::feature_ref("sketch", "Sketch", vec!["sketch"], self.sketch),
            ParamSpec::length("distance", "Distance", &self.distance),
            ParamSpec::boolean("symmetric", "Symmetric", self.symmetric),
            ParamSpec::choice("operation", "Operation", vec!["new", "join", "cut", "intersect"], &self.operation),
        ];
        if self.operation != "new" {
            v.push(ParamSpec::feature_ref("target", "Target body", crate::BODY_TYPES.to_vec(), self.target));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("sketch", ParamValue::FeatureRef(i)) => self.sketch = i,
            ("distance", ParamValue::Expr(s)) => self.distance = s,
            ("symmetric", ParamValue::Bool(b)) => self.symmetric = b,
            ("operation", ParamValue::Choice(o)) => self.operation = o,
            ("target", ParamValue::FeatureRef(i)) => self.target = i,
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
        let loops: Vec<Vec<anvil_math::DVec2>> = profiles.iter().map(|p| p.points.clone()).collect();
        let mut bodies = Vec::new();
        for (outer, holes) in crate::features::emboss::nest_loops(&loops) {
            bodies.push(ctx.kernel.extrude_with_holes(&plane, &outer, &holes, dist)?);
        }
        apply_operation(ctx, &self.operation, self.target, bodies)
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
