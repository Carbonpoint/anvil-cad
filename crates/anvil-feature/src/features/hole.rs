//! Hole: a round hole cut into a body, placed on a plane by (x, y).

use crate::features::datum_plane;
use crate::{
    Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES, PLANE_TYPES,
};
use anvil_kernel::BooleanOp;
use anvil_math::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoleFeature {
    pub body: usize,
    pub plane: String,
    pub plane_feature: Option<usize>,
    #[serde(default)]
    pub face_plane: Option<anvil_math::Plane>,
    pub x: String,
    pub y: String,
    pub z: String,
    pub diameter: String,
    /// Depth into the body from the plane, along the negative normal.
    /// A large value makes a through hole.
    pub depth: String,
    /// Optional counterbore: diameter and depth. Zero disables it.
    pub cbore_diameter: String,
    pub cbore_depth: String,
}

impl Default for HoleFeature {
    fn default() -> Self {
        HoleFeature {
            body: 1,
            plane: "XY".into(),
            plane_feature: None,
            face_plane: None,
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            diameter: "6".into(),
            depth: "100".into(),
            cbore_diameter: "0".into(),
            cbore_depth: "0".into(),
        }
    }
}

#[typetag::serde(name = "hole")]
impl Feature for HoleFeature {
    fn kind(&self) -> &'static str {
        "hole"
    }
    fn name(&self) -> String {
        format!("Hole d={} at ({}, {})", self.diameter, self.x, self.y)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature", "Face"], &self.plane),
            ParamSpec::length("x", "X on plane", &self.x),
            ParamSpec::length("y", "Y on plane", &self.y),
            ParamSpec::length("z", "Plane offset (the hole drills against the plane normal)", &self.z),
            ParamSpec::length("diameter", "Diameter", &self.diameter),
            ParamSpec::length("depth", "Depth (large = through)", &self.depth),
            ParamSpec::length("cbore_diameter", "Counterbore diameter (0 = none)", &self.cbore_diameter),
            ParamSpec::length("cbore_depth", "Counterbore depth", &self.cbore_depth),
        ];
        if self.plane == "Feature" {
            v.push(ParamSpec::feature_ref(
                "plane_feature",
                "Plane feature",
                PLANE_TYPES.to_vec(),
                self.plane_feature.unwrap_or(0),
            ));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("plane", ParamValue::Choice(p)) => self.plane = p,
            ("plane_feature", ParamValue::FeatureRef(i)) => self.plane_feature = Some(i),
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("z", ParamValue::Expr(s)) => self.z = s,
            ("diameter", ParamValue::Expr(s)) => self.diameter = s,
            ("depth", ParamValue::Expr(s)) => self.depth = s,
            ("cbore_diameter", ParamValue::Expr(s)) => self.cbore_diameter = s,
            ("cbore_depth", ParamValue::Expr(s)) => self.cbore_depth = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut plane = match (self.plane.as_str(), self.plane_feature) {
            ("Face", _) => self
                .face_plane
                .ok_or_else(|| RegenError::Other("no face stored; select a face and add the hole again".into()))?,
            ("Feature", Some(i)) => ctx.plane_of(i)?,
            ("Feature", None) => return Err(RegenError::Other("choose a plane feature".into())),
            (d, _) => datum_plane(d),
        };
        plane.origin += plane.normal() * ctx.eval(&self.z)?;
        let c = DVec2::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?);
        let d = ctx.eval(&self.diameter)?;
        let depth = ctx.eval(&self.depth)?;
        let cb_d = ctx.eval(&self.cbore_diameter)?;
        let cb_depth = ctx.eval(&self.cbore_depth)?;
        // Start slightly above the plane so the top face is cut cleanly.
        let mut start = plane;
        start.origin += plane.normal() * 0.01;
        let drill = ctx.kernel.cylinder(&start, c, d / 2.0, -(depth + 0.01))?;
        let mut bodies = Vec::new();
        for b in ctx.bodies_of(self.body)? {
            let before = b.volume();
            let mut r = ctx.kernel.boolean(b, &drill, BooleanOp::Subtract)?;
            if (before - r.volume()).abs() <= 1e-9 * before.abs().max(1.0) {
                return Err(RegenError::Other(
                    "the hole does not touch the body; check the plane, the offset sign, and x/y".into(),
                ));
            }
            if cb_d > 0.0 && cb_depth > 0.0 {
                let cb = ctx.kernel.cylinder(&start, c, cb_d / 2.0, -(cb_depth + 0.01))?;
                r = ctx.kernel.boolean(&r, &cb, BooleanOp::Subtract)?;
            }
            bodies.push(r);
        }
        Ok(FeatureOutput { bodies, consumes: vec![self.body], ..Default::default() })
    }
    fn place_on_face(&mut self, plane: anvil_math::Plane, body: usize) -> bool {
        self.plane = "Face".into();
        self.face_plane = Some(plane);
        self.body = body;
        self.x = "0".into();
        self.y = "0".into();
        self.z = "0".into();
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "hole", label: "Hole", tab: "Solid", group: "Create", tooltip: "Round hole into a body, with optional counterbore", order: 40, create: || Box::new(HoleFeature::default()) } }
