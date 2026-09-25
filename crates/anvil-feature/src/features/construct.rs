//! Construction geometry: Offset plane, Plane at angle.

use crate::features::datum_axis;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, PlaneRef, RegenContext, RegenError};
use anvil_math::{DQuat, Plane};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "OffsetPlaneSaved")]
pub struct OffsetPlaneFeature {
    pub base: PlaneRef,
    pub offset: String,
}

/// What a file holds. Files written before plane references had
/// `"base":"Feature"` and the Feature number in `base_feature`.
#[derive(Deserialize)]
struct OffsetPlaneSaved {
    base: PlaneRef,
    #[serde(default)]
    base_feature: Option<usize>,
    offset: String,
}

impl From<OffsetPlaneSaved> for OffsetPlaneFeature {
    fn from(s: OffsetPlaneSaved) -> Self {
        OffsetPlaneFeature { base: PlaneRef::from_saved(s.base, s.base_feature), offset: s.offset }
    }
}

impl Default for OffsetPlaneFeature {
    fn default() -> Self {
        OffsetPlaneFeature { base: PlaneRef::Datum("XY".into()), offset: "10".into() }
    }
}

/// A plane parameter that also reads typed text: XY, XZ, YZ or a
/// Feature number.
pub(crate) fn plane_value(name: &str, value: ParamValue) -> Result<PlaneRef, String> {
    match value {
        ParamValue::Plane(r) => Ok(r),
        ParamValue::Expr(s) | ParamValue::Text(s) | ParamValue::Choice(s) => Ok(PlaneRef::parse(&s)),
        _ => Err(format!("{name} takes a plane")),
    }
}

#[typetag::serde(name = "offset_plane")]
impl Feature for OffsetPlaneFeature {
    fn kind(&self) -> &'static str {
        "offset_plane"
    }
    fn name(&self) -> String {
        format!("Offset plane ({} + {})", self.base.short(), self.offset)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::plane("base", "Base plane", self.base.clone()),
            ParamSpec::length("offset", "Offset", &self.offset),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("base", v) => self.base = plane_value(name, v)?,
            ("offset", ParamValue::Expr(s)) => self.offset = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut p = ctx.plane_of_ref(&self.base)?;
        p.origin += p.normal() * ctx.eval(&self.offset)?;
        Ok(FeatureOutput { plane: Some(p), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnglePlaneFeature {
    pub base: PlaneRef,
    pub axis: String,
    pub angle: String,
}

impl Default for AnglePlaneFeature {
    fn default() -> Self {
        AnglePlaneFeature { base: PlaneRef::Datum("XY".into()), axis: "X".into(), angle: "45".into() }
    }
}

#[typetag::serde(name = "angle_plane")]
impl Feature for AnglePlaneFeature {
    fn kind(&self) -> &'static str {
        "angle_plane"
    }
    fn name(&self) -> String {
        format!("Plane at angle ({} about {} by {})", self.base.short(), self.axis, self.angle)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::plane("base", "Base plane", self.base.clone()),
            ParamSpec::choice("axis", "Axis", vec!["X", "Y", "Z"], &self.axis),
            ParamSpec::angle("angle", "Angle", &self.angle),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("base", v) => self.base = plane_value(name, v)?,
            ("axis", ParamValue::Choice(p)) => self.axis = p,
            ("angle", ParamValue::Expr(s)) => self.angle = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        // The axis is a world axis through the base plane's origin.
        let base = ctx.plane_of_ref(&self.base)?;
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
