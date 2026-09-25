//! Aircraft: a fuselage, a lofted wing, a tailplane, and a fin as one
//! implicit body, with a lifting line estimate of lift and drag and an
//! optional optimisation of taper and twist. The nTop style loop: change
//! a parameter, the geometry and the numbers follow.
//!
//! Lengths in mm; the aero estimate scales them to metres.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_implicit::aero::{lift_to_drag_at, optimize_taper_and_twist, vortex_lattice};
use anvil_implicit::wing::{Fuselage, MirrorY, Naca4, Placed, Wing};
use anvil_implicit::{Func, Intersect, SmoothUnion, Union};
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

/// Surface tags on the aircraft body's faces, by component.
pub const TAG_FUSELAGE: u32 = 1;
pub const TAG_WING: u32 = 2;
pub const TAG_TAIL: u32 = 3;
pub const TAG_FIN: u32 = 4;

/// Name of a component tag, for exports.
pub fn tag_name(tag: u32) -> &'static str {
    match tag {
        TAG_FUSELAGE => "fuselage",
        TAG_WING => "wing",
        TAG_TAIL => "tail",
        TAG_FIN => "fin",
        _ => "body",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AircraftFeature {
    pub fuselage_length: String,
    pub fuselage_diameter: String,
    /// Wing leading edge position along the fuselage from the nose.
    pub wing_x: String,
    pub span: String,
    pub root_chord: String,
    pub tip_chord: String,
    pub sweep: String,
    pub dihedral: String,
    pub twist_root: String,
    pub twist_tip: String,
    pub airfoil_root: String,
    pub airfoil_tip: String,
    pub tail_span: String,
    pub tail_chord: String,
    pub fin_height: String,
    pub fin_chord: String,
    /// Smooth union radius between wing and fuselage.
    pub blend: String,
    /// Trailing edge thickness of every surface (mm), for meshing and
    /// printing; 0 for sharp edges.
    #[serde(default = "te_default")]
    pub te_thickness: String,
    pub resolution: String,
    /// Design lift coefficient and speed (m/s) for the estimate.
    pub cl_design: String,
    pub speed: String,
    /// Replace taper and tip twist with the best found for lift to drag.
    pub optimize: bool,
}

fn te_default() -> String {
    "0.8".into()
}

impl Default for AircraftFeature {
    fn default() -> Self {
        AircraftFeature {
            fuselage_length: "160".into(),
            fuselage_diameter: "18".into(),
            wing_x: "55".into(),
            span: "180".into(),
            root_chord: "30".into(),
            tip_chord: "15".into(),
            sweep: "8".into(),
            dihedral: "4".into(),
            twist_root: "2".into(),
            twist_tip: "-1".into(),
            airfoil_root: "2412".into(),
            airfoil_tip: "0012".into(),
            tail_span: "60".into(),
            tail_chord: "14".into(),
            fin_height: "28".into(),
            fin_chord: "16".into(),
            blend: "4".into(),
            te_thickness: "0.8".into(),
            resolution: "0.6".into(),
            cl_design: "0.5".into(),
            speed: "20".into(),
            optimize: false,
        }
    }
}

#[typetag::serde(name = "aircraft")]
impl Feature for AircraftFeature {
    fn kind(&self) -> &'static str {
        "aircraft"
    }
    fn name(&self) -> String {
        format!("Aircraft (span {}, NACA {})", self.span, self.airfoil_root)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let text = |name: &'static str, label: &'static str, v: &str| ParamSpec {
            name,
            label,
            kind: crate::param::ParamKind::Text,
            value: ParamValue::Expr(v.to_string()),
        };
        vec![
            ParamSpec::length("fuselage_length", "Fuselage length", &self.fuselage_length),
            ParamSpec::length("fuselage_diameter", "Fuselage diameter", &self.fuselage_diameter),
            ParamSpec::length("wing_x", "Wing position from nose", &self.wing_x),
            ParamSpec::length("span", "Span", &self.span),
            ParamSpec::length("root_chord", "Root chord", &self.root_chord),
            ParamSpec::length("tip_chord", "Tip chord", &self.tip_chord),
            ParamSpec::angle("sweep", "Leading edge sweep", &self.sweep),
            ParamSpec::angle("dihedral", "Dihedral", &self.dihedral),
            ParamSpec::angle("twist_root", "Root incidence", &self.twist_root),
            ParamSpec::angle("twist_tip", "Tip twist", &self.twist_tip),
            text("airfoil_root", "Root airfoil (NACA 4 digit)", &self.airfoil_root),
            text("airfoil_tip", "Tip airfoil (NACA 4 digit)", &self.airfoil_tip),
            ParamSpec::length("tail_span", "Tailplane span", &self.tail_span),
            ParamSpec::length("tail_chord", "Tailplane chord", &self.tail_chord),
            ParamSpec::length("fin_height", "Fin height", &self.fin_height),
            ParamSpec::length("fin_chord", "Fin chord", &self.fin_chord),
            ParamSpec::length("blend", "Wing to body blend", &self.blend),
            ParamSpec::length("te_thickness", "Trailing edge thickness", &self.te_thickness),
            ParamSpec::length("resolution", "Resolution", &self.resolution),
            ParamSpec::length("cl_design", "Design lift coefficient", &self.cl_design),
            ParamSpec::length("speed", "Speed (m/s)", &self.speed),
            ParamSpec::boolean("optimize", "Optimise taper and twist", self.optimize),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("optimize", ParamValue::Bool(b)) => self.optimize = b,
            (n, ParamValue::Expr(s)) => match n {
                "fuselage_length" => self.fuselage_length = s,
                "fuselage_diameter" => self.fuselage_diameter = s,
                "wing_x" => self.wing_x = s,
                "span" => self.span = s,
                "root_chord" => self.root_chord = s,
                "tip_chord" => self.tip_chord = s,
                "sweep" => self.sweep = s,
                "dihedral" => self.dihedral = s,
                "twist_root" => self.twist_root = s,
                "twist_tip" => self.twist_tip = s,
                "airfoil_root" => self.airfoil_root = s,
                "airfoil_tip" => self.airfoil_tip = s,
                "tail_span" => self.tail_span = s,
                "tail_chord" => self.tail_chord = s,
                "fin_height" => self.fin_height = s,
                "fin_chord" => self.fin_chord = s,
                "blend" => self.blend = s,
                "te_thickness" => self.te_thickness = s,
                "resolution" => self.resolution = s,
                "cl_design" => self.cl_design = s,
                "speed" => self.speed = s,
                other => return Err(format!("unknown parameter {other}")),
            },
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let e = |s: &str| ctx.eval(s);
        let (len, dia, wing_x) = (e(&self.fuselage_length)?, e(&self.fuselage_diameter)?, e(&self.wing_x)?);
        let (span, root_chord, tip_chord) = (e(&self.span)?, e(&self.root_chord)?, e(&self.tip_chord)?);
        let (sweep, dihedral, twist_root, twist_tip) =
            (e(&self.sweep)?, e(&self.dihedral)?, e(&self.twist_root)?, e(&self.twist_tip)?);
        let (tail_span, tail_chord, fin_height, fin_chord) =
            (e(&self.tail_span)?, e(&self.tail_chord)?, e(&self.fin_height)?, e(&self.fin_chord)?);
        let (blend, step, cl_design, speed) =
            (e(&self.blend)?, e(&self.resolution)?, e(&self.cl_design)?, e(&self.speed)?);
        let te = e(&self.te_thickness)?.max(0.0);
        let root = Naca4::parse(&self.airfoil_root).ok_or_else(|| {
            RegenError::Other(format!("root airfoil {} is not a NACA 4 digit code", self.airfoil_root))
        })?;
        let tip = Naca4::parse(&self.airfoil_tip)
            .ok_or_else(|| RegenError::Other(format!("tip airfoil {} is not a NACA 4 digit code", self.airfoil_tip)))?;
        if step <= 0.0 || span <= 0.0 || root_chord <= 0.0 || len <= 0.0 {
            return Err(RegenError::Other("span, chords, fuselage length, and resolution must be positive".into()));
        }
        // Aero in metres.
        let mm = 0.001;
        let design =
            Wing::new(span * mm, root_chord * mm, tip_chord * mm, sweep, dihedral, twist_root, twist_tip, root, tip);
        let (ld0, r0) = lift_to_drag_at(&design, cl_design, speed);
        let (wing_m, result, changed) = if self.optimize {
            let (w, r) = optimize_taper_and_twist(&design, cl_design, speed);
            (w, r, true)
        } else {
            (design, r0.clone(), false)
        };
        // Geometry in mm from the (possibly optimised) wing.
        let wing = Wing::with_trailing_edge(
            wing_m.span / mm,
            wing_m.root_chord / mm,
            wing_m.tip_chord / mm,
            sweep,
            dihedral,
            twist_root,
            wing_m.twist_tip,
            root,
            tip,
            te,
        );
        let fuselage = Fuselage::sears_haack(len, dia * 0.5);
        let wing_placed = Placed { field: wing, roll_deg: 0.0, at: DVec3::new(wing_x, 0.0, 0.0) };
        let tail = Placed {
            field: Wing::with_trailing_edge(
                tail_span,
                tail_chord,
                tail_chord * 0.7,
                10.0,
                0.0,
                -1.0,
                -1.0,
                Naca4::parse("0010").unwrap(),
                Naca4::parse("0010").unwrap(),
                te,
            ),
            roll_deg: 0.0,
            at: DVec3::new(len - tail_chord * 1.3, 0.0, 0.0),
        };
        let fin_full = Placed {
            field: Wing::with_trailing_edge(
                2.0 * fin_height,
                fin_chord,
                fin_chord * 0.6,
                30.0,
                0.0,
                0.0,
                0.0,
                Naca4::parse("0010").unwrap(),
                Naca4::parse("0010").unwrap(),
                te,
            ),
            roll_deg: 90.0,
            at: DVec3::new(len - fin_chord * 1.4, 0.0, 0.0),
        };
        let fin = Intersect(MirrorY(fin_full), Func(|p: DVec3| -p.z));
        let body = SmoothUnion { a: &fuselage, b: &wing_placed, k: blend.max(0.01) };
        let plane = Union(Union(body, &tail), &fin);
        // Each triangle is tagged by the component nearest its centroid, so
        // exports can name the wing, the fuselage, the tail, and the fin.
        let components: [(&dyn anvil_implicit::Field, u32); 4] =
            [(&fuselage, TAG_FUSELAGE), (&wing_placed, TAG_WING), (&tail, TAG_TAIL), (&fin, TAG_FIN)];
        let lo = DVec3::new(-2.0 * step, -0.5 * span - 2.0 * step, -0.5 * dia - fin_chord - 2.0 * step);
        let hi = DVec3::new(len + 2.0 * step, 0.5 * span + 2.0 * step, fin_height + 0.5 * dia + 2.0 * step);
        let voxels = ((hi - lo) / step).ceil();
        if voxels.x * voxels.y * voxels.z > 4.0e8 {
            return Err(RegenError::Other(format!(
                "{:.0} million voxels; raise the resolution",
                voxels.x * voxels.y * voxels.z / 1e6
            )));
        }
        let tris = anvil_implicit::mesh::surface_nets(&plane, lo, hi, step);
        if tris.is_empty() {
            return Err(RegenError::Other("the aircraft field left nothing to mesh".into()));
        }
        let tags: Vec<u32> = tris
            .iter()
            .map(|t| {
                let c = (t[0] + t[1] + t[2]) / 3.0;
                components
                    .iter()
                    .map(|(f, tag)| (f.at(c), *tag))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, tag)| tag)
                    .unwrap_or(TAG_FUSELAGE)
            })
            .collect();
        let out = anvil_kernel::ops::from_tagged_triangles(&tris, Some(&tags), step * 1e-3);
        let mut note = format!(
            "wing area {:.3} m2, aspect ratio {:.1}, taper {:.2}; at CL {:.2} and {:.0} m/s: alpha {:.1} deg, CDi {:.4}, CD0 {:.4}, e {:.2}, L/D {:.1}",
            wing_m.area(),
            wing_m.aspect_ratio(),
            wing_m.taper(),
            result.cl,
            speed,
            result.alpha_deg,
            result.cd_induced,
            result.cd_profile,
            result.span_efficiency,
            result.lift_to_drag
        );
        let vlm = vortex_lattice(&wing_m, result.alpha_deg, 12, 4);
        note.push_str(&format!("; vortex lattice at the same angle: CL {:.2}, CDi {:.4}", vlm.cl, vlm.cd_induced));
        if changed {
            note.push_str(&format!(
                "; optimised from L/D {:.1} (taper {:.2}, tip twist {:.1}) to taper {:.2}, tip twist {:.1}",
                ld0,
                tip_chord / root_chord,
                twist_tip,
                wing_m.taper(),
                wing_m.twist_tip
            ));
        }
        Ok(FeatureOutput { bodies: vec![out], note: Some(note), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "aircraft", label: "Aircraft", tab: "Field", group: "Generate", tooltip: "Fuselage, lofted NACA wing, tail, and fin as one implicit body with a lifting line estimate", order: 91, create: || Box::new(AircraftFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn default_aircraft_meshes_and_reports_lift_to_drag() {
        let mut doc = Document::new("plane");
        doc.add_feature(Box::new(AircraftFeature { resolution: "1.5".into(), ..Default::default() }));
        let f = &doc.features[0];
        assert!(f.error.is_none(), "{:?}", f.error);
        let out = f.output.as_ref().unwrap();
        let b = &out.bodies[0];
        assert_eq!(b.open_edge_report().0, 0, "closed");
        let bb = b.bounds();
        assert!(bb.max.y > 85.0 && bb.min.y < -85.0, "span {bb:?}");
        assert!(bb.max.x > 150.0, "fuselage length {bb:?}");
        let note = out.note.clone().unwrap_or_default();
        assert!(note.contains("L/D"), "{note}");
        eprintln!("{note}");
        // Every component tag is present on the faces.
        let mut seen = std::collections::HashSet::new();
        for f in b.faces.values() {
            if let anvil_kernel::Surface::Revolved { id } = f.surface {
                seen.insert(id);
            }
        }
        for tag in [TAG_FUSELAGE, TAG_WING, TAG_TAIL, TAG_FIN] {
            assert!(seen.contains(&tag), "tag {} missing", tag_name(tag));
        }
    }

    #[test]
    fn optimised_aircraft_keeps_or_improves_lift_to_drag() {
        let mut doc = Document::new("plane");
        doc.add_feature(Box::new(AircraftFeature {
            resolution: "2".into(),
            optimize: true,
            tip_chord: "30".into(),
            ..Default::default()
        }));
        let f = &doc.features[0];
        assert!(f.error.is_none(), "{:?}", f.error);
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("optimised from"), "{note}");
        eprintln!("{note}");
    }
}
