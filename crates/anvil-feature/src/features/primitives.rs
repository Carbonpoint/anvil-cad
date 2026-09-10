//! Primitive solids: Box, Cylinder, Sphere, Torus.

use crate::features::datum_plane;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_math::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

macro_rules! simple_setters {
    ($self:ident, $name:ident, $value:ident; $($field:ident),*) => {
        match ($name, $value) {
            $( (stringify!($field), ParamValue::Expr(s)) => $self.$field = s, )*
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoxFeature {
    pub x: String,
    pub y: String,
    pub z: String,
    pub width: String,
    pub depth: String,
    pub height: String,
}

impl Default for BoxFeature {
    fn default() -> Self {
        BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "20".into(),
            depth: "20".into(),
            height: "10".into(),
        }
    }
}

#[typetag::serde(name = "box")]
impl Feature for BoxFeature {
    fn kind(&self) -> &'static str {
        "box"
    }
    fn name(&self) -> String {
        format!("Box ({} x {} x {})", self.width, self.depth, self.height)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Corner X", &self.x),
            ParamSpec::length("y", "Corner Y", &self.y),
            ParamSpec::length("z", "Corner Z", &self.z),
            ParamSpec::length("width", "Width (X)", &self.width),
            ParamSpec::length("depth", "Depth (Y)", &self.depth),
            ParamSpec::length("height", "Height (Z)", &self.height),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        simple_setters!(self, name, value; x, y, z, width, depth, height);
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let corner = DVec3::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?, ctx.eval(&self.z)?);
        let size = DVec3::new(ctx.eval(&self.width)?, ctx.eval(&self.depth)?, ctx.eval(&self.height)?);
        Ok(FeatureOutput { bodies: vec![ctx.kernel.box_solid(corner, size)?], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CylinderFeature {
    pub plane: String,
    pub cx: String,
    pub cy: String,
    pub radius: String,
    pub height: String,
}

impl Default for CylinderFeature {
    fn default() -> Self {
        CylinderFeature { plane: "XY".into(), cx: "0".into(), cy: "0".into(), radius: "10".into(), height: "20".into() }
    }
}

#[typetag::serde(name = "cylinder")]
impl Feature for CylinderFeature {
    fn kind(&self) -> &'static str {
        "cylinder"
    }
    fn name(&self) -> String {
        format!("Cylinder (r={}, h={})", self.radius, self.height)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::choice("plane", "Base plane", vec!["XY", "XZ", "YZ"], &self.plane),
            ParamSpec::length("cx", "Centre X", &self.cx),
            ParamSpec::length("cy", "Centre Y", &self.cy),
            ParamSpec::length("radius", "Radius", &self.radius),
            ParamSpec::length("height", "Height", &self.height),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        if let ("plane", ParamValue::Choice(p)) = (name, &value) {
            self.plane = p.clone();
            return Ok(());
        }
        simple_setters!(self, name, value; cx, cy, radius, height);
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let plane = datum_plane(&self.plane);
        let c = DVec2::new(ctx.eval(&self.cx)?, ctx.eval(&self.cy)?);
        let body = ctx.kernel.cylinder(&plane, c, ctx.eval(&self.radius)?, ctx.eval(&self.height)?)?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SphereFeature {
    pub x: String,
    pub y: String,
    pub z: String,
    pub radius: String,
}

impl Default for SphereFeature {
    fn default() -> Self {
        SphereFeature { x: "0".into(), y: "0".into(), z: "0".into(), radius: "10".into() }
    }
}

#[typetag::serde(name = "sphere")]
impl Feature for SphereFeature {
    fn kind(&self) -> &'static str {
        "sphere"
    }
    fn name(&self) -> String {
        format!("Sphere (r={})", self.radius)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Centre X", &self.x),
            ParamSpec::length("y", "Centre Y", &self.y),
            ParamSpec::length("z", "Centre Z", &self.z),
            ParamSpec::length("radius", "Radius", &self.radius),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        simple_setters!(self, name, value; x, y, z, radius);
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let c = DVec3::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?, ctx.eval(&self.z)?);
        Ok(FeatureOutput { bodies: vec![ctx.kernel.sphere(c, ctx.eval(&self.radius)?)?], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorusFeature {
    pub x: String,
    pub y: String,
    pub z: String,
    pub major: String,
    pub minor: String,
}

impl Default for TorusFeature {
    fn default() -> Self {
        TorusFeature { x: "0".into(), y: "0".into(), z: "0".into(), major: "20".into(), minor: "4".into() }
    }
}

#[typetag::serde(name = "torus")]
impl Feature for TorusFeature {
    fn kind(&self) -> &'static str {
        "torus"
    }
    fn name(&self) -> String {
        format!("Torus (R={}, r={})", self.major, self.minor)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Centre X", &self.x),
            ParamSpec::length("y", "Centre Y", &self.y),
            ParamSpec::length("z", "Centre Z", &self.z),
            ParamSpec::length("major", "Major radius", &self.major),
            ParamSpec::length("minor", "Minor radius", &self.minor),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        simple_setters!(self, name, value; x, y, z, major, minor);
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let c = DVec3::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?, ctx.eval(&self.z)?);
        let body = ctx.kernel.torus(c, ctx.eval(&self.major)?, ctx.eval(&self.minor)?)?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "box", label: "Box", tab: "Solid", group: "Create", tooltip: "Box by corner and size", order: 50, create: || Box::new(BoxFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "cylinder", label: "Cylinder", tab: "Solid", group: "Create", tooltip: "Cylinder on a datum plane", order: 51, create: || Box::new(CylinderFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "sphere", label: "Sphere", tab: "Solid", group: "Create", tooltip: "Sphere by centre and radius", order: 52, create: || Box::new(SphereFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "torus", label: "Torus", tab: "Solid", group: "Create", tooltip: "Torus about the Z axis", order: 53, create: || Box::new(TorusFeature::default()) } }
