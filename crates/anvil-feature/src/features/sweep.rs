//! Sweep: carry a closed profile along an open path from another sketch.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepFeature {
    pub profile: usize,
    pub path: usize,
    /// Regions of the profile sketch to sweep; empty means all (see
    /// `region_select`).
    #[serde(default)]
    pub regions: String,
}

impl Default for SweepFeature {
    fn default() -> Self {
        SweepFeature { profile: 0, path: 1, regions: String::new() }
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
            ParamSpec::regions("regions", "profile", &self.regions),
            ParamSpec::feature_ref("path", "Path sketch", vec!["sketch"], self.path),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("profile", ParamValue::FeatureRef(i)) => self.profile = i,
            ("path", ParamValue::FeatureRef(i)) => self.path = i,
            ("regions", ParamValue::Expr(s)) => self.regions = s,
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
        for (outer, holes) in crate::region_select::select(&profiles, &self.regions)? {
            let mut body = ctx.kernel.sweep(&p, &outer, path, false)?;
            // A hole in the region runs the length of the sweep.
            for h in &holes {
                let tool = ctx.kernel.sweep(&p, h, path, false)?;
                body = ctx.kernel.boolean(&body, &tool, anvil_kernel::BooleanOp::Subtract)?;
            }
            bodies.push(body);
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "sweep", label: "Sweep", tab: "Solid", group: "Create", tooltip: "Sweep a profile sketch along a path sketch", order: 30, create: || Box::new(SweepFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::sketch::SketchFeature;
    use crate::Document;

    #[test]
    fn a_ring_profile_sweeps_to_one_tube() {
        let mut doc = Document::new("tube");
        let mut ring = SketchFeature::on_datum("XY");
        let c = ring.sketch.add_point(0.0, 0.0);
        ring.sketch.add_circle(c, 5.0);
        let c2 = ring.sketch.add_point(0.0, 0.0);
        ring.sketch.add_circle(c2, 3.0);
        doc.add_feature(Box::new(ring));
        let mut path = SketchFeature::on_datum("XZ");
        let a = path.sketch.add_point(0.0, 0.0);
        let b = path.sketch.add_point(0.0, 20.0);
        path.sketch.add_line(a, b);
        doc.add_feature(Box::new(path));
        let i = doc.add_feature(Box::new(SweepFeature { profile: 0, path: 1, regions: String::new() }));
        let out = doc.features[i].output.as_ref().unwrap_or_else(|| panic!("{:?}", doc.features[i].error));
        assert_eq!(out.bodies.len(), 1, "one tube, not a rod and a core");
        let v = out.bodies[0].volume();
        let want = std::f64::consts::PI * (25.0 - 9.0) * 20.0;
        assert!((v - want).abs() < 0.02 * want, "{v} vs {want}");
    }
}
