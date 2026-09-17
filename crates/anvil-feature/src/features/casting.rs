//! Sand casting gating: Sprue, Runner, Riser.
//!
//! These features lay out the metal delivery system for a sand mold. Each
//! one produces a single solid body that a caster can combine, mirror, or
//! subtract from a mold box with the existing Combine and Move features.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_kernel::BooleanOp;
use anvil_math::{Axis, DVec2, DVec3, Plane};
use serde::{Deserialize, Serialize};

/// Pouring cup, tapered sprue, and well, revolved about a vertical axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SprueFeature {
    pub x: String,
    pub y: String,
    /// Z of the rim of the pouring cup, the top of the whole feature.
    pub top_z: String,
    pub cup_diameter: String,
    pub cup_depth: String,
    pub sprue_top_diameter: String,
    pub sprue_bottom_diameter: String,
    pub sprue_height: String,
    pub well_diameter: String,
    pub well_depth: String,
}

impl Default for SprueFeature {
    fn default() -> Self {
        SprueFeature {
            x: "0".into(),
            y: "0".into(),
            top_z: "0".into(),
            cup_diameter: "40".into(),
            cup_depth: "20".into(),
            sprue_top_diameter: "18".into(),
            sprue_bottom_diameter: "12".into(),
            sprue_height: "120".into(),
            well_diameter: "30".into(),
            well_depth: "15".into(),
        }
    }
}

#[typetag::serde(name = "sprue")]
impl Feature for SprueFeature {
    fn kind(&self) -> &'static str {
        "sprue"
    }
    fn name(&self) -> String {
        format!("Sprue (h={})", self.sprue_height)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Axis X", &self.x),
            ParamSpec::length("y", "Axis Y", &self.y),
            ParamSpec::length("top_z", "Top Z", &self.top_z),
            ParamSpec::length("cup_diameter", "Pouring cup diameter", &self.cup_diameter),
            ParamSpec::length("cup_depth", "Pouring cup depth", &self.cup_depth),
            ParamSpec::length("sprue_top_diameter", "Sprue top diameter", &self.sprue_top_diameter),
            ParamSpec::length("sprue_bottom_diameter", "Sprue bottom diameter", &self.sprue_bottom_diameter),
            ParamSpec::length("sprue_height", "Sprue height", &self.sprue_height),
            ParamSpec::length("well_diameter", "Well diameter", &self.well_diameter),
            ParamSpec::length("well_depth", "Well depth", &self.well_depth),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        let ParamValue::Expr(s) = value else { return Err("expected an expression".into()) };
        match name {
            "x" => self.x = s,
            "y" => self.y = s,
            "top_z" => self.top_z = s,
            "cup_diameter" => self.cup_diameter = s,
            "cup_depth" => self.cup_depth = s,
            "sprue_top_diameter" => self.sprue_top_diameter = s,
            "sprue_bottom_diameter" => self.sprue_bottom_diameter = s,
            "sprue_height" => self.sprue_height = s,
            "well_diameter" => self.well_diameter = s,
            "well_depth" => self.well_depth = s,
            n => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let x = ctx.eval(&self.x)?;
        let y = ctx.eval(&self.y)?;
        let top_z = ctx.eval(&self.top_z)?;
        let cup_r = ctx.eval(&self.cup_diameter)? / 2.0;
        let cup_depth = ctx.eval(&self.cup_depth)?;
        let sprue_top_r = ctx.eval(&self.sprue_top_diameter)? / 2.0;
        let sprue_bottom_r = ctx.eval(&self.sprue_bottom_diameter)? / 2.0;
        let sprue_height = ctx.eval(&self.sprue_height)?;
        let well_r = ctx.eval(&self.well_diameter)? / 2.0;
        let well_depth = ctx.eval(&self.well_depth)?;

        if cup_r <= 0.0 || sprue_top_r <= 0.0 || sprue_bottom_r <= 0.0 || well_r <= 0.0 {
            return Err(RegenError::Other("sprue diameters must be positive".into()));
        }
        if cup_depth <= 0.0 || sprue_height <= 0.0 || well_depth <= 0.0 {
            return Err(RegenError::Other("sprue heights must be positive".into()));
        }

        // Profile in a vertical plane, measured as (radius, local height),
        // local height 0 at the well bottom. It stays on x >= 0 and touches
        // the axis at the top and bottom, so revolving it makes one solid:
        // a frustum for the pouring cup, a frustum for the tapered sprue,
        // and a cylinder for the well.
        let z_well_bottom = 0.0;
        let z_sprue_bottom = well_depth;
        let z_cup_bottom = well_depth + sprue_height;
        let z_top = well_depth + sprue_height + cup_depth;
        let profile = vec![
            DVec2::new(0.0, z_top),
            DVec2::new(cup_r, z_top),
            DVec2::new(sprue_top_r, z_cup_bottom),
            DVec2::new(sprue_bottom_r, z_sprue_bottom),
            DVec2::new(well_r, z_sprue_bottom),
            DVec2::new(well_r, z_well_bottom),
            DVec2::new(0.0, z_well_bottom),
        ];

        let origin = DVec3::new(x, y, top_z - z_top);
        let plane = Plane { origin, x_axis: DVec3::X, y_axis: DVec3::Z };
        let axis = Axis::new(origin, DVec3::Z);
        let body = ctx.kernel.revolve(&plane, &profile, &axis, std::f64::consts::TAU)?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// Straight gating channel with a trapezoid cross section, wider at the
/// bottom so the pattern draws cleanly out of the sand.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunnerFeature {
    pub start_x: String,
    pub start_y: String,
    pub end_x: String,
    pub end_y: String,
    /// Z of the bottom of the runner, the same at both ends.
    pub end_z: String,
    pub width: String,
    pub height: String,
    /// Fraction (0 to 1) the bottom width shrinks from start to end.
    pub taper: String,
}

impl Default for RunnerFeature {
    fn default() -> Self {
        RunnerFeature {
            start_x: "0".into(),
            start_y: "0".into(),
            end_x: "100".into(),
            end_y: "0".into(),
            end_z: "0".into(),
            width: "20".into(),
            height: "15".into(),
            taper: "0".into(),
        }
    }
}

/// The top of the trapezoid cross section, as a fraction of the bottom
/// width. Fixed draft so the runner lifts cleanly out of the cope.
const RUNNER_TOP_RATIO: f64 = 0.7;

/// Closed trapezoid profile, bottom width `w`, centred on the path line.
fn runner_section(w: f64, height: f64) -> Vec<DVec2> {
    let top = w * RUNNER_TOP_RATIO;
    vec![
        DVec2::new(-w / 2.0, 0.0),
        DVec2::new(w / 2.0, 0.0),
        DVec2::new(top / 2.0, height),
        DVec2::new(-top / 2.0, height),
    ]
}

#[typetag::serde(name = "runner")]
impl Feature for RunnerFeature {
    fn kind(&self) -> &'static str {
        "runner"
    }
    fn name(&self) -> String {
        format!("Runner ({} x {})", self.width, self.height)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("start_x", "Start X", &self.start_x),
            ParamSpec::length("start_y", "Start Y", &self.start_y),
            ParamSpec::length("end_x", "End X", &self.end_x),
            ParamSpec::length("end_y", "End Y", &self.end_y),
            ParamSpec::length("end_z", "Bottom Z", &self.end_z),
            ParamSpec::length("width", "Width (bottom)", &self.width),
            ParamSpec::length("height", "Height", &self.height),
            ParamSpec::length("taper", "Taper (0 to 1)", &self.taper),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        let ParamValue::Expr(s) = value else { return Err("expected an expression".into()) };
        match name {
            "start_x" => self.start_x = s,
            "start_y" => self.start_y = s,
            "end_x" => self.end_x = s,
            "end_y" => self.end_y = s,
            "end_z" => self.end_z = s,
            "width" => self.width = s,
            "height" => self.height = s,
            "taper" => self.taper = s,
            n => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let z = ctx.eval(&self.end_z)?;
        let start = DVec3::new(ctx.eval(&self.start_x)?, ctx.eval(&self.start_y)?, z);
        let end = DVec3::new(ctx.eval(&self.end_x)?, ctx.eval(&self.end_y)?, z);
        let width = ctx.eval(&self.width)?;
        let height = ctx.eval(&self.height)?;
        let taper = ctx.eval(&self.taper)?;

        if width <= 0.0 || height <= 0.0 {
            return Err(RegenError::Other("runner width and height must be positive".into()));
        }
        if !(0.0..1.0).contains(&taper) {
            return Err(RegenError::Other("taper must be between 0 and 1".into()));
        }

        let run = end - start;
        if run.length() < 1e-9 {
            return Err(RegenError::Other("runner start and end are the same point".into()));
        }
        let dir = run.normalize();
        // Horizontal direction across the run, perpendicular to it.
        let mut across = DVec3::Z.cross(dir);
        if across.length_squared() < 1e-12 {
            across = DVec3::X.cross(dir);
        }
        let across = across.normalize();

        let end_width = width * (1.0 - taper);
        let plane_a = Plane { origin: start, x_axis: across, y_axis: DVec3::Z };
        let plane_b = Plane { origin: end, x_axis: across, y_axis: DVec3::Z };
        let body = ctx
            .kernel
            .loft(&[(plane_a, runner_section(width, height)), (plane_b, runner_section(end_width, height))])?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

/// Cylindrical riser with a narrow neck, unioned from two cylinders. A
/// blind riser gets a domed top from a sphere cap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiserFeature {
    pub x: String,
    pub y: String,
    pub base_z: String,
    pub diameter: String,
    pub height: String,
    pub neck_diameter: String,
    pub neck_length: String,
    /// true: domed top (blind riser). false: flat, open top.
    pub blind: bool,
}

impl Default for RiserFeature {
    fn default() -> Self {
        RiserFeature {
            x: "0".into(),
            y: "0".into(),
            base_z: "0".into(),
            diameter: "50".into(),
            height: "80".into(),
            neck_diameter: "25".into(),
            neck_length: "10".into(),
            blind: true,
        }
    }
}

/// The neck is extended into the riser body by this much so the boolean
/// union sees real overlapping volume rather than two solids that only
/// touch along a shared face. Negligible next to any real riser size.
const RISER_NECK_OVERLAP: f64 = 1e-4;

#[typetag::serde(name = "riser")]
impl Feature for RiserFeature {
    fn kind(&self) -> &'static str {
        "riser"
    }
    fn name(&self) -> String {
        format!("Riser (d={}, h={})", self.diameter, self.height)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::length("x", "Centre X", &self.x),
            ParamSpec::length("y", "Centre Y", &self.y),
            ParamSpec::length("base_z", "Base Z", &self.base_z),
            ParamSpec::length("diameter", "Riser diameter", &self.diameter),
            ParamSpec::length("height", "Riser height", &self.height),
            ParamSpec::length("neck_diameter", "Neck diameter", &self.neck_diameter),
            ParamSpec::length("neck_length", "Neck length", &self.neck_length),
            ParamSpec::boolean("blind", "Blind (domed top)", self.blind),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("base_z", ParamValue::Expr(s)) => self.base_z = s,
            ("diameter", ParamValue::Expr(s)) => self.diameter = s,
            ("height", ParamValue::Expr(s)) => self.height = s,
            ("neck_diameter", ParamValue::Expr(s)) => self.neck_diameter = s,
            ("neck_length", ParamValue::Expr(s)) => self.neck_length = s,
            ("blind", ParamValue::Bool(b)) => self.blind = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let x = ctx.eval(&self.x)?;
        let y = ctx.eval(&self.y)?;
        let base_z = ctx.eval(&self.base_z)?;
        let diameter = ctx.eval(&self.diameter)?;
        let height = ctx.eval(&self.height)?;
        let neck_diameter = ctx.eval(&self.neck_diameter)?;
        let neck_length = ctx.eval(&self.neck_length)?;

        if diameter <= 0.0 || height <= 0.0 || neck_diameter <= 0.0 || neck_length <= 0.0 {
            return Err(RegenError::Other("riser dimensions must be positive".into()));
        }

        let neck_plane = Plane { origin: DVec3::new(x, y, base_z), ..Plane::XY };
        let neck =
            ctx.kernel.cylinder(&neck_plane, DVec2::ZERO, neck_diameter / 2.0, neck_length + RISER_NECK_OVERLAP)?;
        let riser_plane = Plane { origin: DVec3::new(x, y, base_z + neck_length), ..Plane::XY };
        let riser = ctx.kernel.cylinder(&riser_plane, DVec2::ZERO, diameter / 2.0, height)?;
        let mut body = ctx.kernel.boolean(&neck, &riser, BooleanOp::Union)?;

        if self.blind {
            let top = DVec3::new(x, y, base_z + neck_length + height);
            let cap = ctx.kernel.sphere(top, diameter / 2.0)?;
            body = ctx.kernel.boolean(&body, &cap, BooleanOp::Union)?;
        }

        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "sprue", label: "Sprue", tab: "Casting", group: "Gating", tooltip: "Pouring cup, tapered sprue, and well", order: 0, create: || Box::new(SprueFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "runner", label: "Runner", tab: "Casting", group: "Gating", tooltip: "Straight gating channel, trapezoid cross section", order: 1, create: || Box::new(RunnerFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "riser", label: "Riser", tab: "Casting", group: "Gating", tooltip: "Feeder reservoir with a neck, blind or open top", order: 2, create: || Box::new(RiserFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn sprue_makes_one_body_of_the_right_volume() {
        let mut doc = Document::new("test");
        let i = doc.add_feature(Box::new(SprueFeature::default()));
        assert!(doc.features[i].error.is_none(), "{:?}", doc.features[i].error);
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 1);

        // Hand calc, default params: cup_d=40 (r=20), cup_depth=20,
        // sprue_top_d=18 (r=9), sprue_bottom_d=12 (r=6), sprue_height=120,
        // well_d=30 (r=15), well_depth=15.
        // Frustum volume: V = pi*h/3 * (r1^2 + r1*r2 + r2^2).
        fn frustum(h: f64, r1: f64, r2: f64) -> f64 {
            std::f64::consts::PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2)
        }
        let cup = frustum(20.0, 20.0, 9.0);
        let sprue = frustum(120.0, 9.0, 6.0);
        let well = std::f64::consts::PI * 15.0 * 15.0 * 15.0;
        let exact = cup + sprue + well;
        let got = bodies[0].volume();
        assert!((got - exact).abs() / exact < 0.05, "got {got}, exact {exact}");
    }

    #[test]
    fn runner_makes_one_body_of_the_right_volume() {
        let mut doc = Document::new("test");
        // Default taper is 0: the runner is a straight trapezoid prism, so
        // volume is exactly cross-section area times length.
        let i = doc.add_feature(Box::new(RunnerFeature::default()));
        assert!(doc.features[i].error.is_none(), "{:?}", doc.features[i].error);
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 1);

        // Hand calc, default params: start (0,0,0), end (100,0,0), so
        // length = 100. width=20, height=15, top = 0.7*20 = 14.
        // Trapezoid area = height * (bottom + top) / 2.
        let length = 100.0;
        let width = 20.0;
        let height = 15.0;
        let top = width * RUNNER_TOP_RATIO;
        let area = height * (width + top) / 2.0;
        let exact = area * length;
        let got = bodies[0].volume();
        assert!((got - exact).abs() / exact < 0.05, "got {got}, exact {exact}");
    }

    #[test]
    fn riser_makes_one_body_of_the_right_volume() {
        let mut doc = Document::new("test");
        let i = doc.add_feature(Box::new(RiserFeature::default()));
        assert!(doc.features[i].error.is_none(), "{:?}", doc.features[i].error);
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 1);

        // Hand calc, default params: diameter=50 (R=25), height=80,
        // neck_diameter=25 (r=12.5), neck_length=10, blind=true.
        // Neck cylinder + riser cylinder + a hemisphere dome cap
        // (radius = riser radius, since the sphere sits centred on top and
        // only its upper half protrudes above the riser).
        let neck_r = 12.5;
        let neck_len = 10.0;
        let riser_r = 25.0;
        let riser_h = 80.0;
        let neck_vol = std::f64::consts::PI * neck_r * neck_r * neck_len;
        let riser_vol = std::f64::consts::PI * riser_r * riser_r * riser_h;
        let dome_vol = 2.0 / 3.0 * std::f64::consts::PI * riser_r * riser_r * riser_r;
        let exact = neck_vol + riser_vol + dome_vol;
        let got = bodies[0].volume();
        assert!((got - exact).abs() / exact < 0.05, "got {got}, exact {exact}");
    }

    #[test]
    fn riser_open_top_has_no_dome() {
        let mut doc = Document::new("test");
        let feature = RiserFeature { blind: false, ..Default::default() };
        let i = doc.add_feature(Box::new(feature));
        assert!(doc.features[i].error.is_none(), "{:?}", doc.features[i].error);
        let bodies = doc.bodies();
        assert_eq!(bodies.len(), 1);

        let neck_r = 12.5;
        let neck_len = 10.0;
        let riser_r = 25.0;
        let riser_h = 80.0;
        let neck_vol = std::f64::consts::PI * neck_r * neck_r * neck_len;
        let riser_vol = std::f64::consts::PI * riser_r * riser_r * riser_h;
        let exact = neck_vol + riser_vol;
        let got = bodies[0].volume();
        assert!((got - exact).abs() / exact < 0.05, "got {got}, exact {exact}");
    }
}
