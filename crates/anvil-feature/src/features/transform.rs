//! Body transforms: Move/Copy, Scale, Mirror, Rectangular pattern, Circular pattern.

use crate::features::{datum_axis, datum_plane};
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_kernel::Solid;
use anvil_math::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

fn consumed(copy: bool, body: usize) -> Vec<usize> {
    if copy {
        Vec::new()
    } else {
        vec![body]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveFeature {
    pub body: usize,
    pub dx: String,
    pub dy: String,
    pub dz: String,
    pub rx: String,
    pub ry: String,
    pub rz: String,
    pub copy: bool,
}

impl Default for MoveFeature {
    fn default() -> Self {
        MoveFeature {
            body: 1,
            dx: "0".into(),
            dy: "0".into(),
            dz: "0".into(),
            rx: "0".into(),
            ry: "0".into(),
            rz: "0".into(),
            copy: false,
        }
    }
}

#[typetag::serde(name = "move")]
impl Feature for MoveFeature {
    fn kind(&self) -> &'static str {
        "move"
    }
    fn name(&self) -> String {
        format!("{} body {}", if self.copy { "Copy" } else { "Move" }, self.body)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::length("dx", "Move X", &self.dx),
            ParamSpec::length("dy", "Move Y", &self.dy),
            ParamSpec::length("dz", "Move Z", &self.dz),
            ParamSpec::angle("rx", "Rotate X", &self.rx),
            ParamSpec::angle("ry", "Rotate Y", &self.ry),
            ParamSpec::angle("rz", "Rotate Z", &self.rz),
            ParamSpec::boolean("copy", "Keep original", self.copy),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("dx", ParamValue::Expr(s)) => self.dx = s,
            ("dy", ParamValue::Expr(s)) => self.dy = s,
            ("dz", ParamValue::Expr(s)) => self.dz = s,
            ("rx", ParamValue::Expr(s)) => self.rx = s,
            ("ry", ParamValue::Expr(s)) => self.ry = s,
            ("rz", ParamValue::Expr(s)) => self.rz = s,
            ("copy", ParamValue::Bool(b)) => self.copy = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let t = DVec3::new(ctx.eval(&self.dx)?, ctx.eval(&self.dy)?, ctx.eval(&self.dz)?);
        // glam's ZYX takes the Z angle first and applies X first, so the
        // fields go in reversed: Rotate X really turns about X.
        let q = DQuat::from_euler(
            anvil_math::glam_euler(),
            ctx.eval(&self.rz)?.to_radians(),
            ctx.eval(&self.ry)?.to_radians(),
            ctx.eval(&self.rx)?.to_radians(),
        );
        let bodies = ctx.bodies_of(self.body)?.iter().map(|b| b.transformed(|p| q * p + t, false)).collect();
        Ok(FeatureOutput { bodies, consumes: consumed(self.copy, self.body), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScaleFeature {
    pub body: usize,
    pub factor: String,
    pub copy: bool,
}

impl Default for ScaleFeature {
    fn default() -> Self {
        ScaleFeature { body: 1, factor: "2".into(), copy: false }
    }
}

#[typetag::serde(name = "scale")]
impl Feature for ScaleFeature {
    fn kind(&self) -> &'static str {
        "scale"
    }
    fn name(&self) -> String {
        format!("Scale body {} by {}", self.body, self.factor)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::length("factor", "Factor", &self.factor),
            ParamSpec::boolean("copy", "Keep original", self.copy),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("factor", ParamValue::Expr(s)) => self.factor = s,
            ("copy", ParamValue::Bool(b)) => self.copy = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let f = ctx.eval(&self.factor)?;
        if f <= 0.0 {
            return Err(RegenError::Other("scale factor must be positive".into()));
        }
        let bodies = ctx
            .bodies_of(self.body)?
            .iter()
            .map(|b| {
                let c = b.bounds().center();
                b.transformed(|p| c + (p - c) * f, false)
            })
            .collect();
        Ok(FeatureOutput { bodies, consumes: consumed(self.copy, self.body), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorFeature {
    pub body: usize,
    pub plane: String,
    pub keep: bool,
}

impl Default for MirrorFeature {
    fn default() -> Self {
        MirrorFeature { body: 1, plane: "YZ".into(), keep: true }
    }
}

#[typetag::serde(name = "mirror")]
impl Feature for MirrorFeature {
    fn kind(&self) -> &'static str {
        "mirror"
    }
    fn name(&self) -> String {
        format!("Mirror body {} across {}", self.body, self.plane)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("plane", "Mirror plane", vec!["XY", "XZ", "YZ"], &self.plane),
            ParamSpec::boolean("keep", "Keep original", self.keep),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("plane", ParamValue::Choice(p)) => self.plane = p,
            ("keep", ParamValue::Bool(b)) => self.keep = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let plane = datum_plane(&self.plane);
        let bodies: Vec<Solid> = ctx.bodies_of(self.body)?.iter().map(|b| ctx.kernel.mirror(b, &plane)).collect();
        Ok(FeatureOutput { bodies, consumes: consumed(self.keep, self.body), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RectPatternFeature {
    pub body: usize,
    pub count_x: String,
    pub spacing_x: String,
    pub count_y: String,
    pub spacing_y: String,
    pub dir_x: String,
    pub dir_y: String,
}

impl Default for RectPatternFeature {
    fn default() -> Self {
        RectPatternFeature {
            body: 1,
            count_x: "3".into(),
            spacing_x: "30".into(),
            count_y: "1".into(),
            spacing_y: "30".into(),
            dir_x: "X".into(),
            dir_y: "Y".into(),
        }
    }
}

#[typetag::serde(name = "rect_pattern")]
impl Feature for RectPatternFeature {
    fn kind(&self) -> &'static str {
        "rect_pattern"
    }
    fn name(&self) -> String {
        format!("Rectangular pattern ({} x {})", self.count_x, self.count_y)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("dir_x", "Direction 1", vec!["X", "Y", "Z"], &self.dir_x),
            ParamSpec::length("count_x", "Count 1", &self.count_x),
            ParamSpec::length("spacing_x", "Spacing 1", &self.spacing_x),
            ParamSpec::choice("dir_y", "Direction 2", vec!["X", "Y", "Z"], &self.dir_y),
            ParamSpec::length("count_y", "Count 2", &self.count_y),
            ParamSpec::length("spacing_y", "Spacing 2", &self.spacing_y),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("dir_x", ParamValue::Choice(p)) => self.dir_x = p,
            ("dir_y", ParamValue::Choice(p)) => self.dir_y = p,
            ("count_x", ParamValue::Expr(s)) => self.count_x = s,
            ("spacing_x", ParamValue::Expr(s)) => self.spacing_x = s,
            ("count_y", ParamValue::Expr(s)) => self.count_y = s,
            ("spacing_y", ParamValue::Expr(s)) => self.spacing_y = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let nx = ctx.eval(&self.count_x)?.round().max(1.0) as usize;
        let ny = ctx.eval(&self.count_y)?.round().max(1.0) as usize;
        let sx = datum_axis(&self.dir_x) * ctx.eval(&self.spacing_x)?;
        let sy = datum_axis(&self.dir_y) * ctx.eval(&self.spacing_y)?;
        let src = ctx.bodies_of(self.body)?;
        let mut bodies = Vec::with_capacity(nx * ny * src.len());
        for i in 0..nx {
            for j in 0..ny {
                if i == 0 && j == 0 {
                    continue;
                }
                let t = sx * i as f64 + sy * j as f64;
                bodies.extend(src.iter().map(|b| b.transformed(|p| p + t, false)));
            }
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircPatternFeature {
    pub body: usize,
    pub count: String,
    pub total_angle: String,
    /// "X", "Y", "Z", or "Feature" for an axis feature.
    pub axis: String,
    #[serde(default)]
    pub axis_feature: usize,
}

impl Default for CircPatternFeature {
    fn default() -> Self {
        CircPatternFeature { body: 1, count: "6".into(), total_angle: "360".into(), axis: "Z".into(), axis_feature: 0 }
    }
}

#[typetag::serde(name = "circ_pattern")]
impl Feature for CircPatternFeature {
    fn kind(&self) -> &'static str {
        "circ_pattern"
    }
    fn name(&self) -> String {
        format!("Circular pattern ({} about {})", self.count, self.axis)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("axis", "Axis", vec!["X", "Y", "Z", "Feature"], &self.axis),
        ]
        .into_iter()
        .chain((self.axis == "Feature").then(|| {
            ParamSpec::feature_ref("axis_feature", "Axis feature", crate::AXIS_TYPES.to_vec(), self.axis_feature)
        }))
        .chain([
            ParamSpec::length("count", "Count", &self.count),
            ParamSpec::angle("total_angle", "Total angle", &self.total_angle),
        ])
        .collect()
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("axis", ParamValue::Choice(p)) => self.axis = p,
            ("axis_feature", ParamValue::FeatureRef(i)) => self.axis_feature = i,
            ("count", ParamValue::Expr(s)) => self.count = s,
            ("total_angle", ParamValue::Expr(s)) => self.total_angle = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let n = ctx.eval(&self.count)?.round().max(1.0) as usize;
        let total = ctx.eval(&self.total_angle)?.to_radians();
        let full = (total.abs() - std::f64::consts::TAU).abs() < 1e-9;
        let step = if full {
            total / n as f64
        } else if n > 1 {
            total / (n - 1) as f64
        } else {
            0.0
        };
        let axis = if self.axis == "Feature" {
            ctx.axis_of(self.axis_feature)?
        } else {
            anvil_math::Axis::new(DVec3::ZERO, datum_axis(&self.axis))
        };
        let src = ctx.bodies_of(self.body)?;
        let mut bodies = Vec::new();
        for i in 1..n {
            let angle = step * i as f64;
            bodies.extend(src.iter().map(|b| b.transformed(|p| axis.rotate(p, angle), false)));
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "move", label: "Move/Copy", tab: "Solid", group: "Modify", tooltip: "Translate and rotate a body", order: 40, create: || Box::new(MoveFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "scale", label: "Scale", tab: "Solid", group: "Modify", tooltip: "Scale a body about its centre", order: 41, create: || Box::new(ScaleFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "mirror", label: "Mirror", tab: "Solid", group: "Create", tooltip: "Mirror a body across a datum plane", order: 60, create: || Box::new(MirrorFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "rect_pattern", label: "Rect Pattern", tab: "Solid", group: "Create", tooltip: "Rectangular pattern of a body", order: 61, create: || Box::new(RectPatternFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "circ_pattern", label: "Circ Pattern", tab: "Solid", group: "Create", tooltip: "Circular pattern of a body about a datum axis or an axis feature", order: 62, create: || Box::new(CircPatternFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::primitives::BoxFeature;
    use crate::Document;

    #[test]
    fn rotate_x_turns_about_x() {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "10".into(),
            depth: "2".into(),
            height: "4".into(),
        }));
        doc.add_feature(Box::new(MoveFeature { body: 0, rx: "90".into(), ..Default::default() }));
        let bb = doc.features[1].output.as_ref().unwrap().bodies[0].bounds();
        // Rotating +90 about X takes +Y to +Z and +Z to -Y: the box now
        // spans 4 along Y (negative) and 2 along Z.
        assert!((bb.max.x - 10.0).abs() < 1e-9, "{bb:?}");
        assert!((bb.min.y + 4.0).abs() < 1e-9 && bb.max.y.abs() < 1e-9, "{bb:?}");
        assert!((bb.max.z - 2.0).abs() < 1e-9, "{bb:?}");
    }
}
