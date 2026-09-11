//! Features whose kernel support is pending: Chamfer, Shell. Combine works
//! through the CSG boolean.
//!
//! They exist so the ribbon, property panel, and file format are complete.
//! Each returns a clear error until the kernel gains the operation
//! (see ADR 0001, milestone M3).

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_kernel::BooleanOp;
use serde::{Deserialize, Serialize};

macro_rules! pending_feature {
    ($ty:ident, $id:literal, $label:literal, $tooltip:literal, $order:literal, $group:literal, { $($field:ident : $fkind:ident = $default:literal / $flabel:literal),* }, $regen:expr) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $ty {
            pub body: usize,
            $(pub $field: String,)*
        }
        impl Default for $ty {
            fn default() -> Self {
                $ty { body: 1, $($field: $default.into(),)* }
            }
        }
        #[typetag::serde(name = $id)]
        impl Feature for $ty {
            fn kind(&self) -> &'static str { $id }
            fn name(&self) -> String { format!("{} body {}", $label, self.body) }
            fn params(&self) -> Vec<ParamSpec> {
                vec![
                    ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
                    $(ParamSpec::$fkind(stringify!($field), $flabel, &self.$field),)*
                ]
            }
            fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
                match (name, value) {
                    ("body", ParamValue::FeatureRef(i)) => self.body = i,
                    $((stringify!($field), ParamValue::Expr(s)) => self.$field = s,)*
                    (n, _) => return Err(format!("unknown parameter {n}")),
                }
                Ok(())
            }
            fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
                let f: fn(&$ty, &mut RegenContext) -> Result<FeatureOutput, RegenError> = $regen;
                f(self, ctx)
            }
            fn clone_box(&self) -> Box<dyn Feature> { Box::new(self.clone()) }
        }
        inventory::submit! { FeatureDescriptor { id: $id, label: $label, tab: "Solid", group: $group, tooltip: $tooltip, order: $order, create: || Box::new($ty::default()) } }
    };
}

pending_feature!(ShellFeature, "shell", "Shell", "Hollow a body with a wall thickness (kernel support pending)", 32, "Modify",
{ thickness: length = "2" / "Thickness" },
|f, ctx| {
    let t = ctx.eval(&f.thickness)?;
    let body = &ctx.bodies_of(f.body)?[0];
    let out = ctx.kernel.shell(body, t)?;
    Ok(FeatureOutput { bodies: vec![out], consumes: vec![f.body], ..Default::default() })
});

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
