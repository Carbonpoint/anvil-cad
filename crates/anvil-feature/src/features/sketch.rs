//! Sketch feature: a 2D sketch on a datum plane.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_math::Plane;
use anvil_sketch::Sketch;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SketchFeature {
    pub plane_name: String,
    pub sketch: Sketch,
}

impl SketchFeature {
    pub fn plane_from_name(name: &str) -> Plane {
        match name {
            "XZ" => Plane::XZ,
            "YZ" => Plane::YZ,
            _ => Plane::XY,
        }
    }

    /// A sketch holding one rectangle, used by the demo and tests.
    pub fn rectangle(plane_name: &str, w: f64, h: f64) -> Self {
        let mut sketch = Sketch::new(Self::plane_from_name(plane_name));
        sketch.add_rectangle(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
        SketchFeature { plane_name: plane_name.into(), sketch }
    }
}

impl Default for SketchFeature {
    fn default() -> Self {
        Self::rectangle("XY", 40.0, 25.0)
    }
}

#[typetag::serde(name = "sketch")]
impl Feature for SketchFeature {
    fn type_id(&self) -> &'static str {
        "sketch"
    }
    fn name(&self) -> String {
        format!("Sketch ({})", self.plane_name)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ"], &self.plane_name)]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("plane", ParamValue::Choice(p)) => {
                self.plane_name = p;
                self.sketch.plane = Self::plane_from_name(&self.plane_name);
            }
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, _ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut s = self.sketch.clone();
        let rep = s.solve();
        if rep.status == anvil_sketch::SolveStatus::NotConverged {
            return Err(RegenError::Other(format!("sketch did not converge (residual {:.3e})", rep.residual)));
        }
        Ok(FeatureOutput { profiles: s.profiles(), plane: Some(s.plane), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "sketch",
        label: "Sketch",
        tab: "Home",
        group: "Construction",
        tooltip: "Create a 2D sketch on a datum plane",
        order: 10,
        create: || Box::new(SketchFeature::default()),
    }
}
