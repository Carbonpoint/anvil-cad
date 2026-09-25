//! Fillet and Chamfer on picked edges.
//!
//! Edges are stored as world-space end points, with the normals of the two
//! faces that meet there. On each Regenerate an edge is found again: the
//! stored segment if it still lies on an edge of the body, else the edge
//! with the same direction and the same two face normals nearest the
//! stored one. The found edge is written back, so a saved file holds where
//! each edge is now. A box that grows taller keeps its fillets.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES};
use anvil_kernel::blend::BlendKind;
use anvil_kernel::Solid;
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

/// The normals of the two faces that meet along `e`, in a fixed order.
fn face_pair(s: &Solid, e: [DVec3; 2]) -> Option<[DVec3; 2]> {
    match anvil_kernel::blend::edge_face_normals(s, e[0], e[1]).as_slice() {
        [a, b] => Some(if (a.x, a.y, a.z) <= (b.x, b.y, b.z) { [*a, *b] } else { [*b, *a] }),
        _ => None,
    }
}

/// Find a stored edge in the body. See the module notes.
fn find_edge(s: &Solid, e: [DVec3; 2], normals: Option<[DVec3; 2]>) -> Option<([DVec3; 2], [DVec3; 2])> {
    if let Some(n) = face_pair(s, e) {
        return Some((e, n));
    }
    let n = normals?;
    let dir = (e[1] - e[0]).normalize_or_zero();
    let mid = (e[0] + e[1]) * 0.5;
    let same = |a: [DVec3; 2]| (a[0] - n[0]).length() < 1e-6 && (a[1] - n[1]).length() < 1e-6;
    s.feature_edges(0.35)
        .into_iter()
        .filter(|c| (c[1] - c[0]).normalize_or_zero().cross(dir).length() < 1e-6)
        .filter_map(|c| face_pair(s, c).filter(|p| same(*p)).map(|p| (c, p)))
        .min_by(|a, b| {
            let d = |c: [DVec3; 2]| ((c[0] + c[1]) * 0.5 - mid).length_squared();
            d(a.0).total_cmp(&d(b.0))
        })
        .map(|(c, p)| {
            // Keep the stored direction, so the edge list reads the same.
            if (c[1] - c[0]).dot(dir) < 0.0 {
                ([c[1], c[0]], p)
            } else {
                (c, p)
            }
        })
}

fn regen(
    ctx: &RegenContext,
    body: usize,
    edges: &[[DVec3; 2]],
    normals: &[[DVec3; 2]],
    size: f64,
    kind: BlendKind,
) -> Result<FeatureOutput, RegenError> {
    if edges.is_empty() {
        return Err(RegenError::Other(
            "no edges: click edges in the viewport, then press Use selected edges in Properties".into(),
        ));
    }
    let mut bodies = Vec::new();
    let mut found: Vec<([DVec3; 2], [DVec3; 2])> = Vec::new();
    for b in ctx.bodies_of(body)? {
        let mut here = Vec::new();
        for (k, e) in edges.iter().enumerate() {
            match find_edge(b, *e, normals.get(k).copied()) {
                Some(f) => here.push(f),
                None => {
                    return Err(RegenError::Other(format!(
                        "edge {} of the {} is gone: pick the edges again",
                        k + 1,
                        match kind {
                            BlendKind::Fillet => "fillet",
                            BlendKind::Chamfer => "chamfer",
                        }
                    )))
                }
            }
        }
        let now: Vec<[DVec3; 2]> = here.iter().map(|(e, _)| *e).collect();
        bodies.push(ctx.kernel.blend_edges(b, &now, size, kind)?);
        found = here;
    }
    ctx.edge_hits.borrow_mut().extend(found);
    Ok(FeatureOutput { bodies, consumes: vec![body], ..Default::default() })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilletFeature {
    pub body_feature: usize,
    pub radius: String,
    #[serde(default)]
    pub edges: Vec<[DVec3; 2]>,
    /// The two face normals at each edge, found on the first Regenerate.
    #[serde(default)]
    pub edge_normals: Vec<[DVec3; 2]>,
}

impl Default for FilletFeature {
    fn default() -> Self {
        FilletFeature { body_feature: 1, radius: "2".into(), edges: Vec::new(), edge_normals: Vec::new() }
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
        regen(ctx, self.body_feature, &self.edges, &self.edge_normals, r, BlendKind::Fillet)
    }
    fn set_edges(&mut self, edges: Vec<[DVec3; 2]>, body: usize) -> bool {
        self.edges = edges;
        self.edge_normals.clear();
        self.body_feature = body;
        true
    }
    fn update_edges(&mut self, found: &[([DVec3; 2], [DVec3; 2])]) {
        if found.len() == self.edges.len() {
            self.edges = found.iter().map(|(e, _)| *e).collect();
            self.edge_normals = found.iter().map(|(_, n)| *n).collect();
        }
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
    /// The two face normals at each edge, found on the first Regenerate.
    #[serde(default)]
    pub edge_normals: Vec<[DVec3; 2]>,
}

impl Default for ChamferFeature {
    fn default() -> Self {
        ChamferFeature { body: 1, distance: "1".into(), edges: Vec::new(), edge_normals: Vec::new() }
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
        regen(ctx, self.body, &self.edges, &self.edge_normals, d, BlendKind::Chamfer)
    }
    fn set_edges(&mut self, edges: Vec<[DVec3; 2]>, body: usize) -> bool {
        self.edges = edges;
        self.edge_normals.clear();
        self.body = body;
        true
    }
    fn update_edges(&mut self, found: &[([DVec3; 2], [DVec3; 2])]) {
        if found.len() == self.edges.len() {
            self.edges = found.iter().map(|(e, _)| *e).collect();
            self.edge_normals = found.iter().map(|(_, n)| *n).collect();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::primitives::BoxFeature;
    use crate::Document;

    #[test]
    fn a_fillet_follows_its_edge_when_the_box_grows() {
        let mut doc = Document::new("fillet");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "20".into(),
            depth: "10".into(),
            height: "10".into(),
        }));
        // The top front edge: along X, between the top and the front.
        let edge = [DVec3::new(0.0, 0.0, 10.0), DVec3::new(20.0, 0.0, 10.0)];
        let f = doc.add_feature(Box::new(FilletFeature {
            body_feature: 0,
            radius: "2".into(),
            edges: vec![edge],
            edge_normals: vec![],
        }));
        let cut = 4.0 - std::f64::consts::PI; // r^2 (1 - pi / 4) per unit length, r = 2
        let vol = |d: &Document| {
            d.features[f].output.as_ref().unwrap_or_else(|| panic!("{:?}", d.features[f].error)).bodies[0].volume()
        };
        assert!((vol(&doc) - (2000.0 - cut * 20.0)).abs() < 0.5, "{}", vol(&doc));
        doc.edit_feature(0, |b| {
            b.set_param("height", ParamValue::Expr("15".into())).unwrap();
        });
        assert!(doc.features[f].error.is_none(), "the fillet lost its edge: {:?}", doc.features[f].error);
        assert!((vol(&doc) - (3000.0 - cut * 20.0)).abs() < 0.5, "{}", vol(&doc));
        // The edge moved up, and the file keeps where it is now.
        let e = doc.features[f].feature.downcast_ref::<FilletFeature>().unwrap().edges[0];
        assert!((e[0].z - 15.0).abs() < 1e-9 && (e[1].z - 15.0).abs() < 1e-9, "{e:?}");
    }
}
