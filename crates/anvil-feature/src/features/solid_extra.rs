//! Coil, Pipe, Insert Mesh, Split Body.

use crate::features::{datum_plane, parse_index_list};
use crate::{
    Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, BODY_TYPES, PLANE_TYPES,
};
use anvil_math::{DVec2, DVec3, Plane};
use serde::{Deserialize, Serialize};

fn circle(r: f64) -> Vec<DVec2> {
    (0..24)
        .map(|i| {
            let t = i as f64 / 24.0 * std::f64::consts::TAU;
            DVec2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoilFeature {
    pub x: String,
    pub y: String,
    pub z: String,
    pub radius: String,
    pub pitch: String,
    pub turns: String,
    pub wire: String,
}

impl Default for CoilFeature {
    fn default() -> Self {
        CoilFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            radius: "10".into(),
            pitch: "4".into(),
            turns: "5".into(),
            wire: "2".into(),
        }
    }
}

#[typetag::serde(name = "coil")]
impl Feature for CoilFeature {
    fn kind(&self) -> &'static str {
        "coil"
    }
    fn name(&self) -> String {
        format!("Coil (r={}, {} turns)", self.radius, self.turns)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Centre X", &self.x),
            ParamSpec::length("y", "Centre Y", &self.y),
            ParamSpec::length("z", "Base Z", &self.z),
            ParamSpec::length("radius", "Coil radius", &self.radius),
            ParamSpec::length("pitch", "Pitch", &self.pitch),
            ParamSpec::length("turns", "Turns", &self.turns),
            ParamSpec::length("wire", "Wire diameter", &self.wire),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        let ParamValue::Expr(s) = value else { return Err("expected an expression".into()) };
        match name {
            "x" => self.x = s,
            "y" => self.y = s,
            "z" => self.z = s,
            "radius" => self.radius = s,
            "pitch" => self.pitch = s,
            "turns" => self.turns = s,
            "wire" => self.wire = s,
            n => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let c = DVec3::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?, ctx.eval(&self.z)?);
        let path = anvil_kernel::ops::helix_path(
            c,
            ctx.eval(&self.radius)?,
            ctx.eval(&self.pitch)?,
            ctx.eval(&self.turns)?,
            24,
        );
        let plane = Plane { origin: path[0], x_axis: DVec3::X, y_axis: DVec3::Z };
        let body = ctx.kernel.sweep(&plane, &circle(ctx.eval(&self.wire)? / 2.0), &path, false)?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipeFeature {
    pub path: usize,
    pub diameter: String,
}

impl Default for PipeFeature {
    fn default() -> Self {
        PipeFeature { path: 0, diameter: "4".into() }
    }
}

#[typetag::serde(name = "pipe")]
impl Feature for PipeFeature {
    fn kind(&self) -> &'static str {
        "pipe"
    }
    fn name(&self) -> String {
        format!("Pipe (d={})", self.diameter)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("path", "Path sketch", vec!["sketch"], self.path),
            ParamSpec::length("diameter", "Diameter", &self.diameter),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("path", ParamValue::FeatureRef(i)) => self.path = i,
            ("diameter", ParamValue::Expr(s)) => self.diameter = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let paths = ctx.paths_of(self.path)?;
        let r = ctx.eval(&self.diameter)? / 2.0;
        let mut bodies = Vec::new();
        for path in &paths {
            let t = (path[1] - path[0]).normalize_or_zero();
            let helper = if t.dot(DVec3::Z).abs() < 0.9 { DVec3::Z } else { DVec3::X };
            let x = helper.cross(t).normalize();
            let plane = Plane { origin: path[0], x_axis: x, y_axis: t.cross(x) };
            bodies.push(ctx.kernel.sweep(&plane, &circle(r), path, false)?);
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshFeature {
    pub path: String,
    pub scale: String,
}

impl Default for MeshFeature {
    fn default() -> Self {
        MeshFeature { path: "part.stl".into(), scale: "1".into() }
    }
}

#[typetag::serde(name = "mesh")]
impl Feature for MeshFeature {
    fn kind(&self) -> &'static str {
        "mesh"
    }
    fn name(&self) -> String {
        format!("Mesh ({})", self.path)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec {
                name: "path",
                label: "STL file",
                kind: crate::param::ParamKind::Text,
                value: ParamValue::Expr(self.path.clone()),
            },
            ParamSpec::length("scale", "Scale", &self.scale),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("path", ParamValue::Expr(s)) => self.path = s,
            ("scale", ParamValue::Expr(s)) => self.scale = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let k = ctx.eval(&self.scale)?;
        let tris = crate::mesh_loader::load(&self.path).map_err(RegenError::Other)?;
        let tris: Vec<[DVec3; 3]> = tris.iter().map(|t| [t[0] * k, t[1] * k, t[2] * k]).collect();
        if tris.is_empty() {
            return Err(RegenError::Other("no triangles in file".into()));
        }
        Ok(FeatureOutput { bodies: vec![anvil_kernel::ops::from_triangles(&tris, 1e-6)], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplitBodyFeature {
    pub body: usize,
    pub plane: String,
    pub plane_feature: Option<usize>,
    pub offset: String,
    /// "both", "below", or "above".
    pub keep: String,
}

impl Default for SplitBodyFeature {
    fn default() -> Self {
        SplitBodyFeature { body: 1, plane: "XY".into(), plane_feature: None, offset: "5".into(), keep: "both".into() }
    }
}

#[typetag::serde(name = "split_body")]
impl Feature for SplitBodyFeature {
    fn kind(&self) -> &'static str {
        "split_body"
    }
    fn name(&self) -> String {
        format!("Split body {} by {}", self.body, self.plane)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature"], &self.plane),
            ParamSpec::length("offset", "Offset along normal", &self.offset),
            ParamSpec::choice("keep", "Keep", vec!["both", "below", "above"], &self.keep),
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
            ("offset", ParamValue::Expr(s)) => self.offset = s,
            ("keep", ParamValue::Choice(k)) => self.keep = k,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut plane = match (self.plane.as_str(), self.plane_feature) {
            ("Feature", Some(i)) => ctx.plane_of(i)?,
            ("Feature", None) => return Err(RegenError::Other("choose a plane feature".into())),
            (d, _) => datum_plane(d),
        };
        plane.origin += plane.normal() * ctx.eval(&self.offset)?;
        let mut bodies = Vec::new();
        for b in ctx.bodies_of(self.body)? {
            let (lo, hi) = ctx.kernel.split(b, &plane)?;
            match self.keep.as_str() {
                "below" => bodies.push(lo),
                "above" => bodies.push(hi),
                _ => {
                    bodies.push(lo);
                    bodies.push(hi);
                }
            }
        }
        Ok(FeatureOutput { bodies, consumes: vec![self.body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// Plane through three points given as expressions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plane3PointsFeature {
    pub points: String,
}

impl Default for Plane3PointsFeature {
    fn default() -> Self {
        Plane3PointsFeature { points: "0,0,0, 10,0,0, 0,10,5".into() }
    }
}

#[typetag::serde(name = "plane_3pt")]
impl Feature for Plane3PointsFeature {
    fn kind(&self) -> &'static str {
        "plane_3pt"
    }
    fn name(&self) -> String {
        "Plane through 3 points".into()
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![ParamSpec {
            name: "points",
            label: "x,y,z x,y,z x,y,z",
            kind: crate::param::ParamKind::Text,
            value: ParamValue::Expr(self.points.clone()),
        }]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("points", ParamValue::Expr(s)) => self.points = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let v: Vec<f64> = self
            .points
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .map(|t| ctx.eval(t))
            .collect::<Result<_, _>>()?;
        if v.len() != 9 {
            return Err(RegenError::Other("need nine numbers: three points".into()));
        }
        let a = DVec3::new(v[0], v[1], v[2]);
        let b = DVec3::new(v[3], v[4], v[5]);
        let c = DVec3::new(v[6], v[7], v[8]);
        let x = (b - a).normalize_or_zero();
        let n = x.cross(c - a).normalize_or_zero();
        if n.length_squared() < 1e-12 {
            return Err(RegenError::Other("points are collinear".into()));
        }
        Ok(FeatureOutput { plane: Some(Plane { origin: a, x_axis: x, y_axis: n.cross(x) }), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// Midplane between two plane features (or datum planes by name).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidplaneFeature {
    pub first: String,
    pub second: String,
}

impl Default for MidplaneFeature {
    fn default() -> Self {
        MidplaneFeature { first: "XY".into(), second: "0".into() }
    }
}

fn plane_by_ref(s: &str, ctx: &RegenContext) -> Result<Plane, RegenError> {
    match s.trim() {
        "XY" | "XZ" | "YZ" => Ok(datum_plane(s.trim())),
        other => {
            let idx = parse_index_list(other);
            let i =
                *idx.first().ok_or(RegenError::Other(format!("'{other}' is not XY, XZ, YZ, or a feature index")))?;
            ctx.plane_of(i)
        }
    }
}

#[typetag::serde(name = "midplane")]
impl Feature for MidplaneFeature {
    fn kind(&self) -> &'static str {
        "midplane"
    }
    fn name(&self) -> String {
        format!("Midplane ({} | {})", self.first, self.second)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec {
                name: "first",
                label: "First (XY/XZ/YZ or feature #)",
                kind: crate::param::ParamKind::Text,
                value: ParamValue::Expr(self.first.clone()),
            },
            ParamSpec {
                name: "second",
                label: "Second (XY/XZ/YZ or feature #)",
                kind: crate::param::ParamKind::Text,
                value: ParamValue::Expr(self.second.clone()),
            },
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("first", ParamValue::Expr(s)) => self.first = s,
            ("second", ParamValue::Expr(s)) => self.second = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let a = plane_by_ref(&self.first, ctx)?;
        let b = plane_by_ref(&self.second, ctx)?;
        let n = (a.normal() + b.normal()).normalize_or_zero();
        let n = if n.length_squared() < 1e-12 { a.normal() } else { n };
        let origin = (a.origin + b.origin) * 0.5;
        let x = (a.x_axis - n * a.x_axis.dot(n)).normalize_or_zero();
        let x = if x.length_squared() < 1e-12 { n.cross(DVec3::Z).normalize_or_zero() } else { x };
        Ok(FeatureOutput { plane: Some(Plane { origin, x_axis: x, y_axis: n.cross(x) }), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "coil", label: "Coil", tab: "Solid", group: "Create", tooltip: "Helical coil about the Z axis", order: 54, create: || Box::new(CoilFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "pipe", label: "Pipe", tab: "Solid", group: "Create", tooltip: "Round pipe along a path sketch", order: 55, create: || Box::new(PipeFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "mesh", label: "Insert Mesh", tab: "Solid", group: "Insert", tooltip: "Insert an STL file as a body", order: 0, create: || Box::new(MeshFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "split_body", label: "Split Body", tab: "Solid", group: "Modify", tooltip: "Cut a body with a plane", order: 45, create: || Box::new(SplitBodyFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "plane_3pt", label: "Plane 3 Points", tab: "Solid", group: "Construct", tooltip: "Plane through three points", order: 2, create: || Box::new(Plane3PointsFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "midplane", label: "Midplane", tab: "Solid", group: "Construct", tooltip: "Plane halfway between two planes", order: 3, create: || Box::new(MidplaneFeature::default()) } }
