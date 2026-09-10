//! TEMPLATE FEATURE. Copy this file to add a new feature.
//!
//! Steps:
//! 1. Copy to `features/<your_feature>.rs` and add `pub mod <your_feature>;`
//!    to `features/mod.rs`.
//! 2. Rename `TemplateFeature`, fill in the fields your feature needs.
//! 3. Describe editable fields in `params()` and accept edits in `set_param()`.
//! 4. Build geometry in `regenerate()`.
//! 5. Update the `inventory::submit!` block: id, label, tab, group, tooltip.
//!    Tabs today: Solid (groups Create, Modify, Construct, Inspect), CAM.
//!    Bodies from another feature: `ctx.bodies_of(idx)`. Planes: `ctx.plane_of(idx)`.
//!    If your feature replaces its input body, list it in `consumes`.
//!
//! That is all. The ribbon, part navigator, property panel, save/load, and
//! undo pick the feature up automatically.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateFeature {
    /// Example numeric parameter stored as an expression.
    pub size: String,
    /// Example flag.
    pub enabled: bool,
}

impl Default for TemplateFeature {
    fn default() -> Self {
        TemplateFeature { size: "10".into(), enabled: true }
    }
}

#[typetag::serde(name = "template")]
impl Feature for TemplateFeature {
    fn kind(&self) -> &'static str {
        "template"
    }
    fn name(&self) -> String {
        "Template".into()
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![ParamSpec::length("size", "Size", &self.size), ParamSpec::boolean("enabled", "Enabled", self.enabled)]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("size", ParamValue::Expr(s)) => self.size = s,
            ("enabled", ParamValue::Bool(b)) => self.enabled = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let _size = ctx.eval(&self.size)?;
        // Build geometry with ctx.kernel here.
        Ok(FeatureOutput::default())
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// Remove the `#[cfg(any())]` line below to show the template in the ribbon.
#[cfg(any())]
inventory::submit! {
    FeatureDescriptor {
        id: "template",
        label: "Template",
        tab: "Solid",
        group: "Create",
        tooltip: "Example feature. Copy me.",
        order: 900,
        create: || Box::new(TemplateFeature::default()),
    }
}

#[allow(dead_code)]
fn _keep_imports_used() -> Option<&'static FeatureDescriptor> {
    None
}
