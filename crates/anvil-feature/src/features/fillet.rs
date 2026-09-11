//! Fillet and Chamfer on picked edges.
//!
//! Edges are stored as world-space end points captured when the user
//! picked them. They are matched against the body at regeneration time, so
//! edits that move an edge break the fillet with a clear error. Persistent
//! edge naming is on the roadmap.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_kernel::blend::BlendKind;
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

fn regen(
    ctx: &RegenContext,
    body: usize,
    edges: &[[DVec3; 2]],
    size: f64,
    kind: BlendKind,
) -> Result<FeatureOutput, RegenError> {
    if edges.is_empty() {
        return Err(RegenError::Other(
            "no edges: click edges in the viewport, then press Use selected edges in Properties".into(),
        ));
    }
    let mut bodies = Vec::new();
    for b in ctx.bodies_of(body)? {
        bodies.push(ctx.kernel.blend_edges(b, edges, size, kind)?);
    }
    Ok(FeatureOutput { bodies, consumes: vec![body], ..Default::default() })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilletFeature {
    pub body_feature: usize,
    pub radius: String,
    #[serde(default)]
    pub edges: Vec<[DVec3; 2]>,
}

impl Default for FilletFeature {
    fn default() -> Self {
        FilletFeature { body_feature: 1, radius: "2".into(), edges: Vec::new() }
    }
}

#[typetag::serde(name = "fillet")]
impl Feature for FilletFeature {
    fn kind(&self) -> &'static str {
        "fillet"
    }
    fn name(&self) -> String {
        format!("Fillet r={} ({} edges)", self.radius, self.edges.len())
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body_feature),
            ParamSpec::length("radius", "Radius", &self.radius),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body_feature = i,
            ("radius", ParamValue::Expr(s)) => self.radius = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let r = ctx.eval(&self.radius)?;
        regen(ctx, self.body_feature, &self.edges, r, BlendKind::Fillet)
    }
    fn set_edges(&mut self, edges: Vec<[DVec3; 2]>, body: usize) -> bool {
        self.edges = edges;
        self.body_feature = body;
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChamferFeature {
    pub body: usize,
    pub distance: String,
    #[serde(default)]
    pub edges: Vec<[DVec3; 2]>,
}

impl Default for ChamferFeature {
    fn default() -> Self {
        ChamferFeature { body: 1, distance: "1".into(), edges: Vec::new() }
    }
}

#[typetag::serde(name = "chamfer")]
impl Feature for ChamferFeature {
    fn kind(&self) -> &'static str {
        "chamfer"
    }
    fn name(&self) -> String {
        format!("Chamfer {} ({} edges)", self.distance, self.edges.len())
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::length("distance", "Distance", &self.distance),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("distance", ParamValue::Expr(s)) => self.distance = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let d = ctx.eval(&self.distance)?;
        regen(ctx, self.body, &self.edges, d, BlendKind::Chamfer)
    }
    fn set_edges(&mut self, edges: Vec<[DVec3; 2]>, body: usize) -> bool {
        self.edges = edges;
        self.body = body;
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "fillet",
        label: "Fillet",
        tab: "Solid",
        group: "Modify",
        tooltip: "Round the selected straight edges (F). Click edges first",
        order: 30,
        create: || Box::new(FilletFeature::default()),
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "chamfer",
        label: "Chamfer",
        tab: "Solid",
        group: "Modify",
        tooltip: "Bevel the selected straight edges. Click edges first",
        order: 31,
        create: || Box::new(ChamferFeature::default()),
    }
}
