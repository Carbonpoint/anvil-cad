//! Fillet: round selected edges of a body.
//!
//! The native kernel does not implement fillets yet, so this feature reports
//! `Unsupported` on regenerate. The parameter surface and ribbon entry are
//! in place so the UI flow can be developed against it.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilletFeature {
    pub body_feature: usize,
    pub radius: String,
}

impl Default for FilletFeature {
    fn default() -> Self {
        FilletFeature { body_feature: 1, radius: "2".into() }
    }
}

#[typetag::serde(name = "fillet")]
impl Feature for FilletFeature {
    fn type_id(&self) -> &'static str {
        "fillet"
    }
    fn name(&self) -> String {
        format!("Fillet (r={})", self.radius)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", vec!["extrude", "revolve"], self.body_feature),
            ParamSpec::length("radius", "Radius", &self.radius),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body_feature = i,
            ("radius", ParamValue::Expr(s)) => self.radius = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let r = ctx.eval(&self.radius)?;
        let body = ctx
            .upstream
            .get(self.body_feature)
            .and_then(|o| o.as_ref())
            .and_then(|o| o.bodies.first())
            .ok_or(RegenError::BadReference(ctx.index, self.body_feature, "body"))?;
        let edges: Vec<_> = body.edges.keys().collect();
        let out = ctx.kernel.fillet(body, &edges, r)?;
        Ok(FeatureOutput { bodies: vec![out], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "fillet",
        label: "Fillet",
        tab: "Home",
        group: "Feature",
        tooltip: "Round the edges of a body (kernel support pending)",
        order: 30,
        create: || Box::new(FilletFeature::default()),
    }
}
