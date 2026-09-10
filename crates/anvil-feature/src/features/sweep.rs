//! Sweep: carry a closed profile along an open path from another sketch.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepFeature {
    pub profile: usize,
    pub path: usize,
}

impl Default for SweepFeature {
    fn default() -> Self {
        SweepFeature { profile: 0, path: 1 }
    }
}

#[typetag::serde(name = "sweep")]
impl Feature for SweepFeature {
    fn kind(&self) -> &'static str {
        "sweep"
    }
    fn name(&self) -> String {
        format!("Sweep (profile {}, path {})", self.profile, self.path)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("profile", "Profile sketch", vec!["sketch"], self.profile),
            ParamSpec::feature_ref("path", "Path sketch", vec!["sketch"], self.path),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("profile", ParamValue::FeatureRef(i)) => self.profile = i,
            ("path", ParamValue::FeatureRef(i)) => self.path = i,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let (plane, profiles) = ctx.profiles_of(self.profile)?;
        let plane = *plane;
        let profiles = profiles.to_vec();
        let paths = ctx.paths_of(self.path)?;
        let path = &paths[0];
        // Move the profile plane so its origin sits on the path start.
        let mut p = plane;
        p.origin = path[0];
        let mut bodies = Vec::new();
        for prof in &profiles {
            // Express the profile relative to the original plane origin.
            bodies.push(ctx.kernel.sweep(&p, &prof.points, path, false)?);
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "sweep", label: "Sweep", tab: "Solid", group: "Create", tooltip: "Sweep a profile sketch along a path sketch", order: 30, create: || Box::new(SweepFeature::default()) } }
