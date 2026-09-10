//! Sketch feature: a 2D sketch on a plane.
//!
//! The plane comes from one of three sources:
//! * a datum plane (XY, XZ, YZ),
//! * an earlier feature that defines a plane (offset plane, another sketch),
//! * a custom plane stored in the sketch, set by clicking a face.
//!
//! A custom plane is geometry, not a reference. If the face moves, the
//! sketch does not follow. Persistent face references are a roadmap item.

use crate::features::datum_plane;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, PLANE_TYPES};
use anvil_math::Plane;
use anvil_sketch::Sketch;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaneSource {
    Datum(String),
    Feature(usize),
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchFeature {
    pub source: PlaneSource,
    pub sketch: Sketch,
}

impl SketchFeature {
    pub fn plane_from_name(name: &str) -> Plane {
        datum_plane(name)
    }

    pub fn on_datum(name: &str) -> Self {
        SketchFeature { source: PlaneSource::Datum(name.into()), sketch: Sketch::new(datum_plane(name)) }
    }

    pub fn on_plane(plane: Plane) -> Self {
        SketchFeature { source: PlaneSource::Custom, sketch: Sketch::new(plane) }
    }

    pub fn on_feature(idx: usize) -> Self {
        SketchFeature { source: PlaneSource::Feature(idx), sketch: Sketch::new(Plane::XY) }
    }

    /// A sketch holding one rectangle, used by the demo and tests.
    pub fn rectangle(plane_name: &str, w: f64, h: f64) -> Self {
        let mut f = Self::on_datum(plane_name);
        f.sketch.add_rectangle(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
        f
    }

    fn source_label(&self) -> String {
        match &self.source {
            PlaneSource::Datum(d) => d.clone(),
            PlaneSource::Feature(i) => format!("plane of {i}"),
            PlaneSource::Custom => "face".into(),
        }
    }
}

impl Default for SketchFeature {
    fn default() -> Self {
        Self::on_datum("XY")
    }
}

#[typetag::serde(name = "sketch")]
impl Feature for SketchFeature {
    fn kind(&self) -> &'static str {
        "sketch"
    }
    fn name(&self) -> String {
        format!("Sketch ({})", self.source_label())
    }
    fn params(&self) -> Vec<ParamSpec> {
        let choice = match &self.source {
            PlaneSource::Datum(d) => d.as_str(),
            PlaneSource::Feature(_) => "Feature",
            PlaneSource::Custom => "Custom",
        };
        let mut v = vec![ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature", "Custom"], choice)];
        if let PlaneSource::Feature(i) = &self.source {
            v.push(ParamSpec::feature_ref("plane_feature", "Plane feature", PLANE_TYPES.to_vec(), *i));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("plane", ParamValue::Choice(p)) => {
                self.source = match p.as_str() {
                    "Feature" => PlaneSource::Feature(0),
                    "Custom" => PlaneSource::Custom,
                    d => {
                        self.sketch.plane = datum_plane(d);
                        PlaneSource::Datum(d.to_string())
                    }
                };
            }
            ("plane_feature", ParamValue::FeatureRef(i)) => self.source = PlaneSource::Feature(i),
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut s = self.sketch.clone();
        s.plane = match &self.source {
            PlaneSource::Datum(d) => datum_plane(d),
            PlaneSource::Feature(i) => ctx.plane_of(*i)?,
            PlaneSource::Custom => self.sketch.plane,
        };
        let rep = s.solve();
        if rep.status == anvil_sketch::SolveStatus::NotConverged {
            return Err(RegenError::Other(format!("sketch did not converge (residual {:.3e})", rep.residual)));
        }
        Ok(FeatureOutput { profiles: s.profiles(), paths: s.open_chains(), plane: Some(s.plane), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "sketch",
        label: "Sketch",
        tab: "Solid",
        group: "Create",
        tooltip: "Create a sketch: pick a datum plane or a face, then draw",
        order: 0,
        create: || Box::new(SketchFeature::default()),
    }
}
