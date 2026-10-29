//! Sand casting gating: Sprue, Runner, Riser.
//!
//! These features lay out the metal delivery system for a sand mold. Each
//! one produces a single solid body that a caster can combine, mirror, or
//! subtract from a mold box with the existing Combine and Move features.

use crate::BODY_TYPES;
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

inventory::submit! { FeatureDescriptor { id: "sprue", label: "Sprue", tab: "Solid", group: "Casting", tooltip: "Pouring cup, tapered sprue, and well", order: 0, create: || Box::new(SprueFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "runner", label: "Runner", tab: "Solid", group: "Casting", tooltip: "Straight gating channel, trapezoid cross section", order: 1, create: || Box::new(RunnerFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "riser", label: "Riser", tab: "Solid", group: "Casting", tooltip: "Feeder reservoir with a neck, blind or open top", order: 2, create: || Box::new(RiserFeature::default()) } }

#[cfg(test)]
mod tests {
    #[test]
    fn one_measured_pour_calibrates_the_mold_constant() {
        let mut doc = crate::Document::new("plate");
        doc.add_feature(Box::new(crate::features::primitives::BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "100".into(),
            depth: "100".into(),
            height: "10".into(),
        }));
        let i = doc.add_feature(Box::new(CastingCheckFeature {
            casting: 0,
            measured_freeze: "120".into(),
            ..Default::default()
        }));
        let note = doc.features[i].output.as_ref().unwrap().note.clone().unwrap();
        // Modulus of a 100 x 100 x 10 plate: 100000 / 24000 = 4.17 mm, so
        // 120 s gives C = 120 / 4.17^2 = 6.912 s/mm2.
        assert!(note.contains("C = 6.912 s/mm2 from the measured 120 s"), "{note}");
        assert!(note.contains("freezes in about 120 s"), "{note}");
    }

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

/// Draft check: how much of a body faces away from the pull direction.
/// A pattern must leave the sand without tearing it, so every face
/// should lean toward the pull by at least the draft angle. The feature
/// changes nothing; it reports the undercut and low draft area in its
/// note.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DraftCheckFeature {
    pub body: usize,
    /// Pull direction: X, Y, Z, -X, -Y, or -Z.
    pub pull: String,
    /// Minimum draft angle in degrees.
    pub min_draft: String,
}

impl Default for DraftCheckFeature {
    fn default() -> Self {
        DraftCheckFeature { body: 1, pull: "Z".into(), min_draft: "1".into() }
    }
}

#[typetag::serde(name = "draft_check")]
impl Feature for DraftCheckFeature {
    fn kind(&self) -> &'static str {
        "draft_check"
    }
    fn name(&self) -> String {
        format!("Draft check (pull {})", self.pull)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", BODY_TYPES.to_vec(), self.body),
            ParamSpec::choice("pull", "Pull direction", vec!["X", "Y", "Z", "-X", "-Y", "-Z"], &self.pull),
            ParamSpec::angle("min_draft", "Minimum draft angle", &self.min_draft),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("pull", ParamValue::Choice(p)) => self.pull = p,
            ("min_draft", ParamValue::Expr(s)) => self.min_draft = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let dir = match self.pull.as_str() {
            "X" => DVec3::X,
            "Y" => DVec3::Y,
            "-X" => -DVec3::X,
            "-Y" => -DVec3::Y,
            "-Z" => -DVec3::Z,
            _ => DVec3::Z,
        };
        let min = ctx.eval(&self.min_draft)?.to_radians().sin();
        let (mut under_n, mut under_a, mut low_n, mut low_a, mut total_a) = (0usize, 0.0, 0usize, 0.0, 0.0);
        for b in ctx.bodies_of(self.body)? {
            for f in b.faces.values() {
                // Newell vector: direction is the normal, length is twice the area.
                let mut nv = DVec3::ZERO;
                let k = f.outer.len();
                for i in 0..k {
                    let p = b.pos(f.outer[i]);
                    let q = b.pos(f.outer[(i + 1) % k]);
                    nv.x += (p.y - q.y) * (p.z + q.z);
                    nv.y += (p.z - q.z) * (p.x + q.x);
                    nv.z += (p.x - q.x) * (p.y + q.y);
                }
                let area = nv.length() * 0.5;
                if area < 1e-12 {
                    continue;
                }
                total_a += area;
                let c = nv.normalize().dot(dir);
                if c < -min {
                    under_n += 1;
                    under_a += area;
                } else if c < min {
                    low_n += 1;
                    low_a += area;
                }
            }
        }
        let note = if under_n == 0 && low_n == 0 {
            format!("no undercuts, every face has at least {} deg of draft", self.min_draft)
        } else {
            format!(
                "undercut: {under_n} faces, {:.0} mm2 ({:.1} percent); below {} deg draft: {low_n} faces, {:.0} mm2",
                under_a,
                100.0 * under_a / total_a.max(1e-9),
                self.min_draft,
                low_a
            )
        };
        Ok(FeatureOutput { note: Some(note), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "draft_check", label: "Draft check", tab: "Solid", group: "Casting", tooltip: "Report faces that face away from the pull direction", order: 10, create: || Box::new(DraftCheckFeature::default()) } }

/// Starting values for the alloys the casting check knows. Every number
/// is a textbook or datasheet figure from docs/research/casting_simulation.md
/// and is meant to be calibrated, not trusted blindly. Units: kg/m3, C,
/// kJ/kg, J/(kg K), W/(m K), and the mold constant in s/mm2 for
/// Chvorinov's rule with the modulus in mm.
#[derive(Clone, Copy, Debug)]
pub struct Alloy {
    pub name: &'static str,
    pub density_solid: f64,
    pub density_liquid: f64,
    pub liquidus: f64,
    pub solidus: f64,
    pub latent_heat: f64,
    pub specific_heat: f64,
    pub conductivity: f64,
    pub pour_min: f64,
    pub pour_max: f64,
    /// Sprue : runner : ingate area ratio.
    pub gating_ratio: [f64; 3],
    pub pressurized: bool,
    pub mold_constant: f64,
}

pub const ALLOYS: [Alloy; 3] = [
    Alloy {
        name: "grey cast iron",
        density_solid: 7150.0,
        density_liquid: 6980.0,
        liquidus: 1190.0,
        solidus: 1150.0,
        latent_heat: 280.0,
        specific_heat: 840.0,
        conductivity: 30.0,
        pour_min: 1360.0,
        pour_max: 1450.0,
        gating_ratio: [1.0, 2.0, 1.0],
        pressurized: true,
        mold_constant: 1.5,
    },
    Alloy {
        name: "A356 aluminium",
        density_solid: 2680.0,
        density_liquid: 2400.0,
        liquidus: 615.0,
        solidus: 555.0,
        latent_heat: 389.0,
        specific_heat: 1100.0,
        conductivity: 80.0,
        pour_min: 680.0,
        pour_max: 730.0,
        gating_ratio: [1.0, 3.0, 3.0],
        pressurized: false,
        mold_constant: 0.8,
    },
    Alloy {
        name: "AZ91 magnesium",
        density_solid: 1810.0,
        density_liquid: 1650.0,
        liquidus: 595.0,
        solidus: 470.0,
        latent_heat: 373.0,
        specific_heat: 1200.0,
        conductivity: 60.0,
        pour_min: 640.0,
        pour_max: 675.0,
        gating_ratio: [1.0, 3.0, 3.0],
        pressurized: false,
        mold_constant: 0.5,
    },
];

fn zero() -> String {
    "0".into()
}

pub fn alloy_by_name(name: &str) -> Alloy {
    ALLOYS.iter().copied().find(|a| a.name == name).unwrap_or(ALLOYS[0])
}

/// Surface area of a body in mm2 (all faces, holes included).
pub fn surface_area(b: &anvil_kernel::Solid) -> f64 {
    let m = anvil_kernel::mesh::tessellate(b);
    m.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let (a, c, d) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            (c - a).cross(d - a).length() * 0.5
        })
        .sum()
}

/// Casting check: the textbook numbers for a casting, its sprue, and its
/// riser. Mass, surface area, casting modulus, Chvorinov solidification
/// time, pouring temperature against the alloy's range, Torricelli fill
/// time through the sprue choke, the gating areas from the alloy's
/// ratio, and the riser modulus rule. Nothing is changed; the result is
/// the feature note.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CastingCheckFeature {
    pub casting: usize,
    pub alloy: String,
    pub pour_temp: String,
    /// Chvorinov mold constant in s/mm2; 0 takes the alloy's starting value.
    pub mold_constant: String,
    /// Freeze time measured on one real pour, in seconds; 0 when there is
    /// none. When set, it gives the mold constant: C = t / M^2.
    #[serde(default = "zero")]
    pub measured_freeze: String,
    pub has_sprue: bool,
    pub sprue: usize,
    /// Diameter of the sprue choke (its smallest section).
    pub choke_diameter: String,
    pub has_riser: bool,
    pub riser: usize,
}

impl Default for CastingCheckFeature {
    fn default() -> Self {
        CastingCheckFeature {
            casting: 1,
            alloy: "grey cast iron".into(),
            pour_temp: "1400".into(),
            mold_constant: "0".into(),
            measured_freeze: "0".into(),
            has_sprue: false,
            sprue: 0,
            choke_diameter: "12".into(),
            has_riser: false,
            riser: 0,
        }
    }
}

#[typetag::serde(name = "casting_check")]
impl Feature for CastingCheckFeature {
    fn kind(&self) -> &'static str {
        "casting_check"
    }
    fn name(&self) -> String {
        format!("Casting check ({})", self.alloy)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let names: Vec<&'static str> = ALLOYS.iter().map(|a| a.name).collect();
        let mut v = vec![
            ParamSpec::feature_ref("casting", "Casting body", BODY_TYPES.to_vec(), self.casting),
            ParamSpec::choice("alloy", "Alloy", names, &self.alloy),
            ParamSpec::length("pour_temp", "Pouring temperature (C)", &self.pour_temp),
            ParamSpec::length("mold_constant", "Mold constant s/mm2 (0 = alloy default)", &self.mold_constant),
            ParamSpec::length("measured_freeze", "Measured freeze time s (0 = none)", &self.measured_freeze),
            ParamSpec::boolean("has_sprue", "Check a sprue", self.has_sprue),
        ];
        if self.has_sprue {
            v.push(ParamSpec::feature_ref("sprue", "Sprue", vec!["sprue"], self.sprue));
            v.push(ParamSpec::length("choke_diameter", "Choke diameter", &self.choke_diameter));
        }
        v.push(ParamSpec::boolean("has_riser", "Check a riser", self.has_riser));
        if self.has_riser {
            v.push(ParamSpec::feature_ref("riser", "Riser", vec!["riser"], self.riser));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("casting", ParamValue::FeatureRef(i)) => self.casting = i,
            ("alloy", ParamValue::Choice(a)) => self.alloy = a,
            ("pour_temp", ParamValue::Expr(s)) => self.pour_temp = s,
            ("mold_constant", ParamValue::Expr(s)) => self.mold_constant = s,
            ("measured_freeze", ParamValue::Expr(s)) => self.measured_freeze = s,
            ("has_sprue", ParamValue::Bool(b)) => self.has_sprue = b,
            ("sprue", ParamValue::FeatureRef(i)) => self.sprue = i,
            ("choke_diameter", ParamValue::Expr(s)) => self.choke_diameter = s,
            ("has_riser", ParamValue::Bool(b)) => self.has_riser = b,
            ("riser", ParamValue::FeatureRef(i)) => self.riser = i,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let alloy = alloy_by_name(&self.alloy);
        let pour = ctx.eval(&self.pour_temp)?;
        let c_user = ctx.eval(&self.mold_constant)?;
        let c = if c_user > 0.0 { c_user } else { alloy.mold_constant };
        let bodies = ctx.bodies_of(self.casting)?;
        let volume: f64 = bodies.iter().map(|b| b.volume()).sum();
        let area: f64 = bodies.iter().map(surface_area).sum();
        if volume <= 0.0 || area <= 0.0 {
            return Err(RegenError::Other("the casting body has no volume".into()));
        }
        let mass_kg = volume * 1e-9 * alloy.density_solid;
        let modulus = volume / area;
        // One measured pour fixes the mold constant for this alloy and sand.
        let measured = ctx.eval(&self.measured_freeze)?;
        let calibrated = (measured > 0.0).then(|| measured / (modulus * modulus));
        let c = calibrated.unwrap_or(c);
        let freeze = c * modulus * modulus;
        let mut parts = vec![
            format!(
                "{}: {:.0} cm3, {:.2} kg, area {:.0} cm2, modulus {:.2} mm, freezes in about {:.0} s ({})",
                alloy.name,
                volume * 1e-3,
                mass_kg,
                area * 1e-2,
                modulus,
                freeze,
                match calibrated {
                    Some(k) => format!(
                        "C = {k:.3} s/mm2 from the measured {measured:.0} s; use it as the mold constant for this sand"
                    ),
                    None => format!("C = {c} s/mm2, calibrate from one pour"),
                }
            ),
            if pour >= alloy.pour_min && pour <= alloy.pour_max {
                format!("pour {pour:.0} C is inside {:.0} to {:.0} C", alloy.pour_min, alloy.pour_max)
            } else {
                format!("pour {pour:.0} C is OUTSIDE {:.0} to {:.0} C", alloy.pour_min, alloy.pour_max)
            },
        ];
        if self.has_sprue {
            let sprue = ctx.bodies_of(self.sprue)?;
            let choke_d = ctx.eval(&self.choke_diameter)?;
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for b in sprue {
                let bb = b.bounds();
                lo = lo.min(bb.min.z);
                hi = hi.max(bb.max.z);
            }
            let head_m = (hi - lo).max(0.0) * 1e-3;
            let v = (2.0 * 9.81 * head_m).sqrt();
            let choke_area = std::f64::consts::PI * choke_d * choke_d / 4.0;
            // Tapered sprue efficiency about 0.74.
            let flow = choke_area * 1e-6 * v * 0.74;
            let fill = if flow > 0.0 { volume * 1e-9 / flow } else { f64::INFINITY };
            let r = alloy.gating_ratio;
            parts.push(format!(
                "sprue head {:.0} mm gives {:.2} m/s at the choke; a {choke_d:.0} mm choke fills the casting in {fill:.1} s; gating {}:{}:{} ({}) wants runner {:.0} mm2 and ingates {:.0} mm2 in total",
                head_m * 1e3,
                v,
                r[0],
                r[1],
                r[2],
                if alloy.pressurized { "pressurized" } else { "unpressurized" },
                choke_area * r[1] / r[0],
                choke_area * r[2] / r[0]
            ));
        }
        if self.has_riser {
            let riser = ctx.bodies_of(self.riser)?;
            let rv: f64 = riser.iter().map(|b| b.volume()).sum();
            let ra: f64 = riser.iter().map(surface_area).sum();
            if rv > 0.0 && ra > 0.0 {
                let rm = rv / ra;
                let need = 1.2 * modulus;
                parts.push(format!(
                    "riser modulus {rm:.2} mm against 1.2 x casting {need:.2} mm: {}",
                    if rm >= need { "OK" } else { "TOO SMALL, the riser freezes first" }
                ));
            }
        }
        Ok(FeatureOutput { note: Some(parts.join("; ")), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "casting_check", label: "Casting check", tab: "Solid", group: "Casting", tooltip: "Mass, modulus, freeze time, fill time, gating areas, riser rule", order: 20, create: || Box::new(CastingCheckFeature::default()) } }
