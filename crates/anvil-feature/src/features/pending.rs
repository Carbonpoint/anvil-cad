//! Combine: join, cut, or intersect two bodies through the CSG boolean.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_kernel::BooleanOp;
use serde::{Deserialize, Serialize};

// Hole is implemented in `hole.rs` now that booleans exist.

/// Combine: join, cut, or intersect a target body with a tool body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombineFeature {
    pub body: usize,
    pub tool: usize,
    pub op: String,
}

impl Default for CombineFeature {
    fn default() -> Self {
        CombineFeature { body: 1, tool: 2, op: "join".into() }
    }
}

#[typetag::serde(name = "combine")]
impl Feature for CombineFeature {
    fn kind(&self) -> &'static str {
        "combine"
    }
    fn name(&self) -> String {
        format!("Combine {} body {} with {}", self.op, self.body, self.tool)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Target body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::feature_ref("tool", "Tool body", BODY_TYPES.to_vec(), self.tool),
            ParamSpec::choice("op", "Operation", vec!["join", "cut", "intersect"], &self.op),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("tool", ParamValue::FeatureRef(i)) => self.tool = i,
            ("op", ParamValue::Choice(o)) => self.op = o,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let op = match self.op.as_str() {
            "cut" => BooleanOp::Subtract,
            "intersect" => BooleanOp::Intersect,
            _ => BooleanOp::Union,
        };
        let tools = ctx.bodies_of(self.tool)?.to_vec();
        let mut bodies = Vec::new();
        for a in ctx.bodies_of(self.body)? {
            let mut acc = a.clone();
            for t in &tools {
                acc = ctx.kernel.boolean(&acc, t, op)?;
            }
            bodies.push(acc);
        }
        Ok(FeatureOutput { bodies, consumes: vec![self.body, self.tool], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "combine", label: "Combine", tab: "Solid", group: "Modify", tooltip: "Join, cut, or intersect two bodies", order: 33, create: || Box::new(CombineFeature::default()) } }
