//! Shell: hollow a body, leaving walls of one thickness, with one face
//! left open if you pick one. Draft: tilt the side faces of a body so it
//! leaves a mold.

use crate::{
    Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, PlaneRef, RegenContext, RegenError, BODY_TYPES,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShellFeature {
    pub body: usize,
    pub thickness: String,
    /// The face to leave open: a flat face of the body, picked in the view.
    /// `None` makes a closed shell with a hollow inside.
    #[serde(default)]
    pub open: PlaneRef,
}

impl Default for ShellFeature {
    fn default() -> Self {
        ShellFeature { body: 1, thickness: "2".into(), open: PlaneRef::None }
    }
}

#[typetag::serde(name = "shell")]
impl Feature for ShellFeature {
    fn kind(&self) -> &'static str {
        "shell"
    }
    fn name(&self) -> String {
        match self.open {
            PlaneRef::None => format!("Shell body {} ({} closed)", self.body, self.thickness),
            _ => format!("Shell body {} ({}, open)", self.body, self.thickness),
        }
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::length("thickness", "Thickness", &self.thickness),
            ParamSpec::plane("open", "Open face (none = closed)", self.open.clone()),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("thickness", ParamValue::Expr(s)) => self.thickness = s,
            ("open", v) => self.open = crate::features::construct::plane_value(name, v)?,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn place_on_face(&mut self, plane: anvil_math::Plane, body: usize) -> bool {
        self.body = body;
        self.open = PlaneRef::Face { feature: body, body: 0, plane };
        true
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let t = ctx.eval(&self.thickness)?;
        let solid = ctx.bodies_of(self.body)?[0].clone();
        // The open face: every face of the body on the referenced plane.
        let open = match &self.open {
            PlaneRef::None => Vec::new(),
            r => {
                let plane = ctx.plane_of_ref(r)?;
                let n = plane.normal();
                let faces: Vec<anvil_kernel::FaceId> = solid
                    .faces
                    .iter()
                    .filter(|(_, f)| {
                        let (fn_, area) = solid.face_normal_area(f);
                        area > 1e-12
                            && fn_.dot(n).abs() > 1.0 - 1e-6
                            && ((solid.pos(f.outer[0]) - plane.origin).dot(n)).abs() < 1e-6 * (1.0 + t)
                    })
                    .map(|(id, _)| id)
                    .collect();
                if faces.is_empty() {
                    return Err(RegenError::Other("the open face is not a face of this body".into()));
                }
                faces
            }
        };
        let out = ctx.kernel.shell(&solid, t, &open)?;
        Ok(FeatureOutput { bodies: vec![out], consumes: vec![self.body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// Draft: tilt the side faces of a body so it leaves a mold. The neutral
/// plane stays put; its normal is the pull direction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DraftFeature {
    pub body: usize,
    /// Degrees. Positive narrows the body away from the neutral plane.
    pub angle: String,
    pub neutral: PlaneRef,
}

impl Default for DraftFeature {
    fn default() -> Self {
        DraftFeature { body: 1, angle: "2".into(), neutral: PlaneRef::Datum("XY".into()) }
    }
}

#[typetag::serde(name = "draft")]
impl Feature for DraftFeature {
    fn kind(&self) -> &'static str {
        "draft"
    }
    fn name(&self) -> String {
        format!("Draft body {} ({} deg from {})", self.body, self.angle, self.neutral.short())
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::angle("angle", "Draft angle", &self.angle),
            ParamSpec::plane("neutral", "Neutral plane (its normal is the pull)", self.neutral.clone()),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("angle", ParamValue::Expr(s)) => self.angle = s,
            ("neutral", v) => self.neutral = crate::features::construct::plane_value(name, v)?,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn place_on_face(&mut self, plane: anvil_math::Plane, body: usize) -> bool {
        self.body = body;
        self.neutral = PlaneRef::Face { feature: body, body: 0, plane };
        true
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let angle = ctx.eval(&self.angle)?.to_radians();
        let neutral = ctx.plane_of_ref(&self.neutral)?;
        let solid = ctx.bodies_of(self.body)?[0].clone();
        let (out, n) = ctx.kernel.draft(&solid, &neutral, angle)?;
        Ok(FeatureOutput {
            bodies: vec![out],
            consumes: vec![self.body],
            note: Some(format!("{n} faces drafted")),
            ..Default::default()
        })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "draft", label: "Draft", tab: "Solid", group: "Modify", tooltip: "Tilt the side faces so the body leaves a mold; the neutral plane stays put", order: 33, create: || Box::new(DraftFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "shell", label: "Shell", tab: "Solid", group: "Modify", tooltip: "Hollow a body with a wall thickness; pick a face to leave open", order: 32, create: || Box::new(ShellFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::primitives::BoxFeature;
    use crate::Document;
    use anvil_math::{DVec3, Plane};

    #[test]
    fn a_box_with_its_top_picked_becomes_a_tray() {
        let mut doc = Document::new("tray");
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "40".into(),
            depth: "30".into(),
            height: "20".into(),
        }));
        let top = Plane { origin: DVec3::new(20.0, 15.0, 20.0), x_axis: DVec3::X, y_axis: DVec3::Y };
        let i = doc.add_feature(Box::new(ShellFeature {
            body: 0,
            thickness: "2".into(),
            open: PlaneRef::Face { feature: 0, body: 0, plane: top },
        }));
        assert!(doc.features[i].error.is_none(), "{:?}", doc.features[i].error);
        let v = doc.features[i].output.as_ref().unwrap().bodies[0].volume();
        let want = 40.0 * 30.0 * 20.0 - 36.0 * 26.0 * 18.0;
        assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
        // The box grows taller: the tray follows, its open top with it.
        doc.edit_feature(0, |f| {
            f.set_param("height", ParamValue::Expr("30".into())).unwrap();
        });
        let v = doc.features[i].output.as_ref().unwrap().bodies[0].volume();
        let want = 40.0 * 30.0 * 30.0 - 36.0 * 26.0 * 28.0;
        assert!((v - want).abs() < 1e-6 * want, "after the edit: {v} vs {want}");
    }
}
