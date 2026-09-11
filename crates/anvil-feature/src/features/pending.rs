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

pending_feature!(ChamferFeature, "chamfer", "Chamfer", "Bevel the edges of a body (kernel support pending)", 31, "Modify",
{ distance: length = "1" / "Distance" },
|f, ctx| {
    let d = ctx.eval(&f.distance)?;
    let body = &ctx.bodies_of(f.body)?[0];
    let edges: Vec<_> = body.edges.keys().collect();
    let out = ctx.kernel.chamfer(body, &edges, d)?;
    Ok(FeatureOutput { bodies: vec![out], consumes: vec![f.body], ..Default::default() })
});

pending_feature!(ShellFeature, "shell", "Shell", "Hollow a body with a wall thickness (kernel support pending)", 32, "Modify",
{ thickness: length = "2" / "Thickness" },
|f, ctx| {
    let t = ctx.eval(&f.thickness)?;
    let body = &ctx.bodies_of(f.body)?[0];
    let out = ctx.kernel.shell(body, t)?;
    Ok(FeatureOutput { bodies: vec![out], consumes: vec![f.body], ..Default::default() })
});

pending_feature!(CombineFeature, "combine", "Combine", "Join, cut, or intersect two bodies", 33, "Modify",
{ tool: length = "2" / "Tool body index", op: length = "0" / "0 join, 1 cut, 2 intersect" },
|f, ctx| {
    let tool = ctx.eval(&f.tool)? as usize;
    let op = match ctx.eval(&f.op)? as i64 { 1 => BooleanOp::Subtract, 2 => BooleanOp::Intersect, _ => BooleanOp::Union };
    let a = &ctx.bodies_of(f.body)?[0];
    let b = &ctx.bodies_of(tool)?[0];
    let out = ctx.kernel.boolean(a, b, op)?;
    Ok(FeatureOutput { bodies: vec![out], consumes: vec![f.body, tool], ..Default::default() })
});

// Hole is implemented in `hole.rs` now that booleans exist.
