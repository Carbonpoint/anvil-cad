//! Construction geometry: Offset plane, Plane at angle.

use crate::features::datum_axis;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, PlaneRef, RegenContext, RegenError};
use anvil_math::{Axis, DMat3, DQuat, DVec3, Plane};
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

/// Where two planes meet, as an axis through the point of that line
/// nearest the first plane's origin.
fn plane_meet(a: &Plane, b: &Plane) -> Option<Axis> {
    let (na, nb) = (a.normal(), b.normal());
    let dir = na.cross(nb);
    if dir.length_squared() < 1e-12 {
        return None;
    }
    // The point on both planes nearest the first plane's origin.
    let (da, db) = (na.dot(a.origin), nb.dot(b.origin));
    let m = DMat3::from_cols(na, nb, dir).transpose();
    let p = m.inverse() * DVec3::new(da, db, dir.dot(a.origin));
    Some(Axis::new(p, dir))
}

/// The centre line of a curved surface of a body: the axis the facet
/// normals all cross at right angles, placed by least squares.
pub fn surface_axis(solid: &anvil_kernel::Solid, tag: u32) -> Option<Axis> {
    let facets: Vec<(DVec3, DVec3)> = solid
        .faces
        .values()
        .filter(|f| matches!(f.surface, anvil_kernel::Surface::Cylindrical { id } | anvil_kernel::Surface::Revolved { id } if id == tag))
        .filter_map(|f| {
            let (n, area) = solid.face_normal_area(f);
            (area > 1e-12).then(|| {
                let c = f.outer.iter().map(|&v| solid.pos(v)).sum::<DVec3>() / f.outer.len() as f64;
                (c, n)
            })
        })
        .collect();
    if facets.len() < 3 {
        return None;
    }
    if let Some(anvil_kernel::SurfaceGeom::Revolved { axis, .. }) = solid.surfaces.get(&tag) {
        return Some(*axis);
    }
    // Direction: the unit vector most nearly at right angles to every
    // normal, the smallest eigenvector of the sum of n n^T, by inverse
    // iteration.
    let mut m = DMat3::ZERO;
    for (_, n) in &facets {
        m += DMat3::from_cols(*n * n.x, *n * n.y, *n * n.z);
    }
    let shifted = m + DMat3::from_diagonal(DVec3::splat(1e-9));
    if shifted.determinant().abs() < 1e-30 {
        return None;
    }
    let inv = shifted.inverse();
    let mut dir = DVec3::new(0.3, 0.5, 0.8).normalize();
    for _ in 0..50 {
        dir = (inv * dir).normalize_or_zero();
    }
    if dir.length_squared() < 0.5 {
        return None;
    }
    // Point: nearest to every normal line (through a facet centre along
    // its normal), across the direction.
    let mut a = DMat3::ZERO;
    let mut b = DVec3::ZERO;
    for (c, n) in &facets {
        let u = (*n - dir * n.dot(dir)).normalize_or_zero();
        // Lines through c along u: (I - u u^T) projects across the line.
        let proj = DMat3::IDENTITY - DMat3::from_cols(u * u.x, u * u.y, u * u.z);
        let proj = proj - DMat3::from_cols(dir * dir.x, dir * dir.y, dir * dir.z);
        a += proj;
        b += proj * *c;
    }
    let a = a + DMat3::from_cols(dir * dir.x, dir * dir.y, dir * dir.z);
    let centre = facets.iter().map(|(c, _)| *c).sum::<DVec3>() / facets.len() as f64;
    let b = b + dir * dir.dot(centre);
    (a.determinant().abs() > 1e-12).then(|| Axis::new(a.inverse() * b, dir))
}

/// A construction axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AxisFeature {
    /// "Two planes", "Edge", "Surface", or "Normal to plane".
    pub method: String,
    #[serde(default)]
    pub first: PlaneRef,
    #[serde(default)]
    pub second: PlaneRef,
    /// A picked edge, as its two end points.
    #[serde(default)]
    pub edge: Option<[DVec3; 2]>,
    /// A picked curved face: the feature that made it and its surface tag.
    #[serde(default)]
    pub surface: Option<(usize, u32)>,
}

impl Default for AxisFeature {
    fn default() -> Self {
        AxisFeature {
            method: "Two planes".into(),
            first: PlaneRef::Datum("XZ".into()),
            second: PlaneRef::Datum("YZ".into()),
            edge: None,
            surface: None,
        }
    }
}

const AXIS_METHODS: [&str; 4] = ["Two planes", "Edge", "Surface", "Normal to plane"];

#[typetag::serde(name = "axis")]
impl Feature for AxisFeature {
    fn kind(&self) -> &'static str {
        "axis"
    }
    fn name(&self) -> String {
        match self.method.as_str() {
            "Two planes" => format!("Axis ({} x {})", self.first.short(), self.second.short()),
            "Normal to plane" => format!("Axis (normal to {})", self.first.short()),
            m => format!("Axis ({})", m.to_lowercase()),
        }
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![ParamSpec::choice("method", "Method", AXIS_METHODS.to_vec(), &self.method)];
        match self.method.as_str() {
            "Two planes" => {
                v.push(ParamSpec::plane("first", "First plane", self.first.clone()));
                v.push(ParamSpec::plane("second", "Second plane", self.second.clone()));
            }
            "Normal to plane" => v.push(ParamSpec::plane("first", "Plane", self.first.clone())),
            _ => {}
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("method", ParamValue::Choice(m)) => self.method = m,
            ("first", v) => self.first = plane_value(name, v)?,
            ("second", v) => self.second = plane_value(name, v)?,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn set_edges(&mut self, edges: Vec<[DVec3; 2]>, _body: usize) -> bool {
        match edges.first() {
            Some(e) => {
                self.edge = Some(*e);
                self.method = "Edge".into();
                true
            }
            None => false,
        }
    }
    fn place_on_surface(&mut self, surface: anvil_kernel::Surface, body: usize) -> bool {
        match surface {
            anvil_kernel::Surface::Cylindrical { id } | anvil_kernel::Surface::Revolved { id } => {
                self.surface = Some((body, id));
                self.method = "Surface".into();
                true
            }
            anvil_kernel::Surface::Plane => false,
        }
    }
    fn place_on_face(&mut self, plane: Plane, body: usize) -> bool {
        self.first = PlaneRef::Face { feature: body, body: 0, plane };
        self.method = "Normal to plane".into();
        true
    }
    fn remap_refs(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<&'static str> {
        let mut broken = Vec::new();
        for (name, r) in [("first", &mut self.first), ("second", &mut self.second)] {
            if r.depends_on().is_some() {
                match r.remapped(map) {
                    Some(n) => *r = n,
                    None => {
                        broken.push(name);
                        *r = PlaneRef::None;
                    }
                }
            }
        }
        if let Some((body, tag)) = self.surface {
            match map(body) {
                Some(n) => self.surface = Some((n, tag)),
                None => {
                    broken.push("surface");
                    self.surface = None;
                }
            }
        }
        broken
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let axis = match self.method.as_str() {
            "Two planes" => {
                let (a, b) = (ctx.plane_of_ref(&self.first)?, ctx.plane_of_ref(&self.second)?);
                plane_meet(&a, &b).ok_or_else(|| RegenError::Other("the two planes are parallel".into()))?
            }
            "Edge" => {
                let [a, b] = self.edge.ok_or_else(|| RegenError::Other("select an edge, then add the axis".into()))?;
                if (b - a).length() < 1e-9 {
                    return Err(RegenError::Other("the edge has no length".into()));
                }
                Axis::new(a, b - a)
            }
            "Surface" => {
                let (body, tag) =
                    self.surface.ok_or_else(|| RegenError::Other("select a round face, then add the axis".into()))?;
                let solids = ctx.bodies_of(body)?;
                solids
                    .iter()
                    .find_map(|s| surface_axis(s, tag))
                    .ok_or_else(|| RegenError::Other("the face has no centre line".into()))?
            }
            "Normal to plane" => {
                let p = ctx.plane_of_ref(&self.first)?;
                Axis::new(p.origin, p.normal())
            }
            m => return Err(RegenError::Other(format!("unknown method {m}"))),
        };
        Ok(FeatureOutput { axis: Some(axis), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// A construction point.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointFeature {
    /// "Coordinates", "Three planes", or "Axis and plane".
    pub method: String,
    pub x: String,
    pub y: String,
    pub z: String,
    #[serde(default)]
    pub planes: [PlaneRef; 3],
    #[serde(default)]
    pub axis: usize,
}

impl Default for PointFeature {
    fn default() -> Self {
        PointFeature {
            method: "Coordinates".into(),
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            planes: [PlaneRef::Datum("XY".into()), PlaneRef::Datum("XZ".into()), PlaneRef::Datum("YZ".into())],
            axis: 0,
        }
    }
}

const POINT_METHODS: [&str; 3] = ["Coordinates", "Three planes", "Axis and plane"];

#[typetag::serde(name = "point")]
impl Feature for PointFeature {
    fn kind(&self) -> &'static str {
        "point"
    }
    fn name(&self) -> String {
        match self.method.as_str() {
            "Coordinates" => format!("Point ({}, {}, {})", self.x, self.y, self.z),
            m => format!("Point ({})", m.to_lowercase()),
        }
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![ParamSpec::choice("method", "Method", POINT_METHODS.to_vec(), &self.method)];
        match self.method.as_str() {
            "Coordinates" => {
                v.push(ParamSpec::length("x", "X", &self.x));
                v.push(ParamSpec::length("y", "Y", &self.y));
                v.push(ParamSpec::length("z", "Z", &self.z));
            }
            "Three planes" => {
                v.push(ParamSpec::plane("plane1", "First plane", self.planes[0].clone()));
                v.push(ParamSpec::plane("plane2", "Second plane", self.planes[1].clone()));
                v.push(ParamSpec::plane("plane3", "Third plane", self.planes[2].clone()));
            }
            _ => {
                v.push(ParamSpec::feature_ref("axis", "Axis", crate::AXIS_TYPES.to_vec(), self.axis));
                v.push(ParamSpec::plane("plane1", "Plane", self.planes[0].clone()));
            }
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("method", ParamValue::Choice(m)) => self.method = m,
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("z", ParamValue::Expr(s)) => self.z = s,
            ("axis", ParamValue::FeatureRef(i)) => self.axis = i,
            ("plane1", v) => self.planes[0] = plane_value(name, v)?,
            ("plane2", v) => self.planes[1] = plane_value(name, v)?,
            ("plane3", v) => self.planes[2] = plane_value(name, v)?,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let p = match self.method.as_str() {
            "Coordinates" => DVec3::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?, ctx.eval(&self.z)?),
            "Three planes" => {
                let ps: Vec<Plane> = self.planes.iter().map(|r| ctx.plane_of_ref(r)).collect::<Result<_, _>>()?;
                let n: Vec<DVec3> = ps.iter().map(|p| p.normal()).collect();
                let m = DMat3::from_cols(n[0], n[1], n[2]).transpose();
                if m.determinant().abs() < 1e-12 {
                    return Err(RegenError::Other("two of the planes are parallel".into()));
                }
                m.inverse() * DVec3::new(n[0].dot(ps[0].origin), n[1].dot(ps[1].origin), n[2].dot(ps[2].origin))
            }
            _ => {
                let a = ctx.axis_of(self.axis)?;
                let pl = ctx.plane_of_ref(&self.planes[0])?;
                let n = pl.normal();
                let den = a.dir.dot(n);
                if den.abs() < 1e-12 {
                    return Err(RegenError::Other("the axis runs along the plane".into()));
                }
                a.origin + a.dir * ((pl.origin - a.origin).dot(n) / den)
            }
        };
        Ok(FeatureOutput { point: Some(p), ..Default::default() })
    }
    fn remap_refs(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<&'static str> {
        let mut broken = Vec::new();
        for (k, name) in ["plane1", "plane2", "plane3"].iter().enumerate() {
            if self.planes[k].depends_on().is_some() {
                match self.planes[k].remapped(map) {
                    Some(r) => self.planes[k] = r,
                    None => {
                        broken.push(*name);
                        self.planes[k] = PlaneRef::None;
                    }
                }
            }
        }
        if self.method == "Axis and plane" {
            match map(self.axis) {
                Some(i) => self.axis = i,
                None => {
                    broken.push("axis");
                    self.axis = usize::MAX;
                }
            }
        }
        broken
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "axis", label: "Axis", tab: "Solid", group: "Construct", tooltip: "Axis where two planes meet, along an edge, through a round face, or normal to a plane", order: 4, create: || Box::new(AxisFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "point", label: "Point", tab: "Solid", group: "Construct", tooltip: "Point by coordinates, where three planes meet, or where an axis meets a plane", order: 5, create: || Box::new(PointFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "offset_plane", label: "Offset Plane", tab: "Solid", group: "Construct", tooltip: "Plane parallel to a base plane at a distance", order: 0, create: || Box::new(OffsetPlaneFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "angle_plane", label: "Plane at Angle", tab: "Solid", group: "Construct", tooltip: "Plane rotated about a datum axis", order: 1, create: || Box::new(AnglePlaneFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::primitives::{BoxFeature, CylinderFeature};
    use crate::features::transform::CircPatternFeature;
    use crate::Document;

    fn axis(doc: &Document, i: usize) -> Axis {
        doc.features[i].output.as_ref().and_then(|o| o.axis).unwrap_or_else(|| panic!("{:?}", doc.features[i].error))
    }

    #[test]
    fn two_datum_planes_meet_in_an_axis() {
        let mut doc = Document::new("axis");
        let i = doc.add_feature(Box::new(AxisFeature::default()));
        let a = axis(&doc, i);
        assert!(a.dir.cross(DVec3::Z).length() < 1e-12, "{a:?}");
        assert!(a.origin.truncate().length() < 1e-12, "{a:?}");
    }

    #[test]
    fn a_pattern_turns_about_an_axis_feature() {
        let mut doc = Document::new("pattern");
        doc.add_feature(Box::new(BoxFeature {
            x: "20".into(),
            y: "0".into(),
            z: "0".into(),
            width: "2".into(),
            depth: "2".into(),
            height: "2".into(),
        }));
        // An axis parallel to Z through (10, 0): two offset planes meet there.
        doc.add_feature(Box::new(OffsetPlaneFeature { base: "YZ".into(), offset: "10".into() }));
        let ax = doc.add_feature(Box::new(AxisFeature {
            first: PlaneRef::Feature(1),
            second: PlaneRef::Datum("XZ".into()),
            ..Default::default()
        }));
        let p = doc.add_feature(Box::new(CircPatternFeature {
            body: 0,
            count: "2".into(),
            total_angle: "360".into(),
            axis: "Feature".into(),
            axis_feature: ax,
        }));
        assert!(doc.features[p].error.is_none(), "{:?}", doc.features[p].error);
        // Half a turn about x = 10 takes x in 20..22 to -2..0.
        let b = doc.features[p].output.as_ref().unwrap().bodies[0].bounds();
        assert!((b.min.x + 2.0).abs() < 1e-9 && b.max.x.abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn a_round_face_gives_its_centre_line() {
        let mut doc = Document::new("cyl");
        doc.add_feature(Box::new(CylinderFeature::default()));
        let solid = doc.features[0].output.as_ref().unwrap().bodies[0].clone();
        let tag = solid
            .faces
            .values()
            .find_map(|f| match f.surface {
                anvil_kernel::Surface::Cylindrical { id } | anvil_kernel::Surface::Revolved { id } => Some(id),
                _ => None,
            })
            .expect("a cylinder has a round face");
        let a = surface_axis(&solid, tag).unwrap();
        let centre = solid.bounds().center();
        assert!(a.dir.cross(DVec3::Z).length() < 1e-6, "{a:?}");
        assert!((a.origin - centre).truncate().length() < 1e-6, "{a:?} vs {centre:?}");
    }

    #[test]
    fn three_planes_meet_in_a_point() {
        let mut doc = Document::new("point");
        doc.add_feature(Box::new(OffsetPlaneFeature { base: "XY".into(), offset: "3".into() }));
        doc.add_feature(Box::new(OffsetPlaneFeature { base: "YZ".into(), offset: "5".into() }));
        let i = doc.add_feature(Box::new(PointFeature {
            method: "Three planes".into(),
            planes: [PlaneRef::Feature(0), PlaneRef::Feature(1), PlaneRef::Datum("XZ".into())],
            ..Default::default()
        }));
        let p = doc.features[i].output.as_ref().and_then(|o| o.point).unwrap();
        assert!((p - DVec3::new(5.0, 0.0, 3.0)).length() < 1e-9, "{p}");
    }
}
