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
        // The largest region of the first sketch, then in each later
        // sketch the region nearest the one before it.
        let mut sections: Vec<(anvil_math::Plane, Vec<anvil_math::DVec2>)> = Vec::new();
        let mut last: Option<anvil_math::DVec3> = None;
        for i in idx {
            let (plane, profiles) = ctx.profiles_of(i)?;
            let regions = crate::region_select::select(profiles, "")?;
            let centre = |pts: &[anvil_math::DVec2]| {
                plane.to_world(pts.iter().copied().sum::<anvil_math::DVec2>() / pts.len().max(1) as f64)
            };
            let area = |pts: &[anvil_math::DVec2]| {
                let n = pts.len();
                (0..n).map(|k| pts[k].perp_dot(pts[(k + 1) % n])).sum::<f64>().abs() * 0.5
            };
            let pick = match last {
                None => regions.iter().max_by(|a, b| area(&a.0).total_cmp(&area(&b.0))),
                Some(c) => regions
                    .iter()
                    .min_by(|a, b| (centre(&a.0) - c).length_squared().total_cmp(&(centre(&b.0) - c).length_squared())),
            };
            let outer = pick.ok_or(RegenError::BadReference(ctx.index, i, "closed profile"))?.0.clone();
            last = Some(centre(&outer));
            sections.push((*plane, outer));
        }
        Ok(FeatureOutput { bodies: vec![ctx.kernel.loft(&sections)?], ..Default::default() })
    }
    fn remap_refs(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<&'static str> {
        let mut broken = Vec::new();
        let mapped: Vec<String> = parse_index_list(&self.sections)
            .into_iter()
            .filter_map(|i| match map(i) {
                Some(n) => Some(n.to_string()),
                None => {
                    broken.push("sections");
                    None
                }
            })
            .collect();
        self.sections = mapped.join(", ");
        broken
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "loft", label: "Loft", tab: "Solid", group: "Create", tooltip: "Skin between two or more sketch profiles", order: 31, create: || Box::new(LoftFeature::default()) } }
