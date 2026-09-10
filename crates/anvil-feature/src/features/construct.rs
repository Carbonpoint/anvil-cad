//! Construction geometry: Offset plane, Plane at angle.

use crate::features::{datum_axis, datum_plane};
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, PLANE_TYPES};
use anvil_math::{DQuat, Plane};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OffsetPlaneFeature {
    pub base: String,
    pub base_feature: Option<usize>,
    pub offset: String,
}

impl Default for OffsetPlaneFeature {
    fn default() -> Self {
        OffsetPlaneFeature { base: "XY".into(), base_feature: None, offset: "10".into() }
    }
}

fn resolve_base(base: &str, base_feature: Option<usize>, ctx: &RegenContext) -> Result<Plane, RegenError> {
    match (base, base_feature) {
        ("Feature", Some(i)) => ctx.plane_of(i),
        ("Feature", None) => Err(RegenError::Other("choose a base plane feature".into())),
        (d, _) => Ok(datum_plane(d)),
    }
}

#[typetag::serde(name = "offset_plane")]
impl Feature for OffsetPlaneFeature {
    fn kind(&self) -> &'static str {
        "offset_plane"
    }
    fn name(&self) -> String {
        format!("Offset plane ({} + {})", self.base, self.offset)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::choice("base", "Base", vec!["XY", "XZ", "YZ", "Feature"], &self.base),
            ParamSpec::length("offset", "Offset", &self.offset),
        ];
        if self.base == "Feature" {
            v.push(ParamSpec::feature_ref(
                "base_feature",
                "Base plane feature",
                PLANE_TYPES.to_vec(),
                self.base_feature.unwrap_or(0),
            ));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("base", ParamValue::Choice(p)) => self.base = p,
            ("offset", ParamValue::Expr(s)) => self.offset = s,
            ("base_feature", ParamValue::FeatureRef(i)) => self.base_feature = Some(i),
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut p = resolve_base(&self.base, self.base_feature, ctx)?;
        p.origin += p.normal() * ctx.eval(&self.offset)?;
        Ok(FeatureOutput { plane: Some(p), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnglePlaneFeature {
    pub base: String,
    pub axis: String,
    pub angle: String,
}

impl Default for AnglePlaneFeature {
    fn default() -> Self {
        AnglePlaneFeature { base: "XY".into(), axis: "X".into(), angle: "45".into() }
    }
}

#[typetag::serde(name = "angle_plane")]
impl Feature for AnglePlaneFeature {
    fn kind(&self) -> &'static str {
        "angle_plane"
    }
    fn name(&self) -> String {
        format!("Plane at angle ({} about {} by {})", self.base, self.axis, self.angle)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::choice("base", "Base", vec!["XY", "XZ", "YZ"], &self.base),
            ParamSpec::choice("axis", "Axis", vec!["X", "Y", "Z"], &self.axis),
            ParamSpec::angle("angle", "Angle", &self.angle),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("base", ParamValue::Choice(p)) => self.base = p,
            ("axis", ParamValue::Choice(p)) => self.axis = p,
            ("angle", ParamValue::Expr(s)) => self.angle = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let base = datum_plane(&self.base);
        let q = DQuat::from_axis_angle(datum_axis(&self.axis), ctx.eval(&self.angle)?.to_radians());
        let p = Plane { origin: base.origin, x_axis: q * base.x_axis, y_axis: q * base.y_axis };
        Ok(FeatureOutput { plane: Some(p), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "offset_plane", label: "Offset Plane", tab: "Solid", group: "Construct", tooltip: "Plane parallel to a base plane at a distance", order: 0, create: || Box::new(OffsetPlaneFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "angle_plane", label: "Plane at Angle", tab: "Solid", group: "Construct", tooltip: "Plane rotated about a datum axis", order: 1, create: || Box::new(AnglePlaneFeature::default()) } }
