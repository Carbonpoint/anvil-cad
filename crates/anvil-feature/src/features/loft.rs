//! Loft: skin between the profiles of two or more sketches.

use crate::features::parse_index_list;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_feature_param_text as text;
use serde::{Deserialize, Serialize};

mod anvil_feature_param_text {
    use crate::param::{ParamKind, ParamSpec, ParamValue};
    pub fn spec(name: &'static str, label: &'static str, v: &str) -> ParamSpec {
        ParamSpec { name, label, kind: ParamKind::Text, value: ParamValue::Expr(v.to_string()) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoftFeature {
    /// Comma-separated sketch feature indices, in order.
    pub sections: String,
}

impl Default for LoftFeature {
    fn default() -> Self {
        LoftFeature { sections: "0, 1".into() }
    }
}

#[typetag::serde(name = "loft")]
impl Feature for LoftFeature {
    fn kind(&self) -> &'static str {
        "loft"
    }
    fn name(&self) -> String {
        format!("Loft ({})", self.sections)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![text::spec("sections", "Sketches (comma list)", &self.sections)]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("sections", ParamValue::Expr(s)) => self.sections = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let idx = parse_index_list(&self.sections);
        if idx.len() < 2 {
            return Err(RegenError::Other("loft needs at least two sketches".into()));
        }
        let mut sections = Vec::new();
        for i in idx {
            let (plane, profiles) = ctx.profiles_of(i)?;
            sections.push((*plane, profiles[0].points.clone()));
        }
        Ok(FeatureOutput { bodies: vec![ctx.kernel.loft(&sections)?], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "loft", label: "Loft", tab: "Solid", group: "Create", tooltip: "Skin between two or more sketch profiles", order: 31, create: || Box::new(LoftFeature::default()) } }
