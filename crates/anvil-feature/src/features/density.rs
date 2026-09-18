//! Density body: a body from a scalar field on a grid, the way a
//! topology optimisation result (a density per voxel) becomes a smooth
//! part in nTop. Reads a legacy VTK grid or a CSV point map, smooths
//! it, thresholds it, and meshes the result.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_implicit::vtk::{load_scalar_file, ScalarData, Threshold};
use anvil_implicit::{Field, PointMap};
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DensityBodyFeature {
    /// `.vtk` (STRUCTURED_POINTS or points with scalars) or CSV `x, y, z, value`.
    pub file: String,
    /// Inside where the value is above this.
    pub level: String,
    /// Box blur radius in grid cells before the threshold (grids only).
    pub smooth: String,
    /// Voxel size of the output mesh (mm); 0 uses the grid spacing.
    pub resolution: String,
    /// Scale from the file's units to mm.
    pub scale: String,
}

impl Default for DensityBodyFeature {
    fn default() -> Self {
        DensityBodyFeature {
            file: "density.vtk".into(),
            level: "0.5".into(),
            smooth: "1".into(),
            resolution: "0".into(),
            scale: "1".into(),
        }
    }
}

#[typetag::serde(name = "density_body")]
impl Feature for DensityBodyFeature {
    fn kind(&self) -> &'static str {
        "density_body"
    }
    fn name(&self) -> String {
        format!("Density body ({} above {})", self.file, self.level)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec {
                name: "file",
                label: "VTK grid or CSV point map",
                kind: crate::param::ParamKind::Text,
                value: ParamValue::Expr(self.file.clone()),
            },
            ParamSpec::length("level", "Threshold", &self.level),
            ParamSpec::length("smooth", "Smoothing (cells)", &self.smooth),
            ParamSpec::length("resolution", "Resolution (0 = grid spacing)", &self.resolution),
            ParamSpec::length("scale", "File units to mm", &self.scale),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("file", ParamValue::Expr(s)) => self.file = s,
            ("level", ParamValue::Expr(s)) => self.level = s,
            ("smooth", ParamValue::Expr(s)) => self.smooth = s,
            ("resolution", ParamValue::Expr(s)) => self.resolution = s,
            ("scale", ParamValue::Expr(s)) => self.scale = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let level = ctx.eval(&self.level)?;
        let smooth = ctx.eval(&self.smooth)?.round().max(0.0) as usize;
        let res = ctx.eval(&self.resolution)?;
        let scale = ctx.eval(&self.scale)?;
        if scale <= 0.0 {
            return Err(RegenError::Other("scale must be positive".into()));
        }
        let data = load_scalar_file(std::path::Path::new(&self.file)).map_err(RegenError::Other)?;
        let (lo, hi) = data.range();
        let (field, lo_p, hi_p, spacing, what): (Box<dyn Field + Send>, DVec3, DVec3, f64, String) = match data {
            ScalarData::Grid(g) => {
                let g = anvil_implicit::vtk::GridField {
                    origin: g.origin * scale,
                    spacing: g.spacing * scale,
                    dims: g.dims,
                    values: g.values,
                };
                let sp = g.spacing.min_element();
                let ext = g.extent();
                let what = format!("grid {} x {} x {}", g.dims[0], g.dims[1], g.dims[2]);
                let g = g.smoothed(smooth, 2);
                (Box::new(g.clone()), g.origin, ext, sp, what)
            }
            ScalarData::Points(p) => {
                // Rebuild the map in mm.
                let text = std::fs::read_to_string(&self.file).map_err(|e| RegenError::Other(e.to_string()))?;
                let pts = PointMap::parse_csv(&text).or_else(|_| {
                    Err(RegenError::Other("point data from VTK cannot be rescaled yet; use scale 1".into()))
                });
                let pts = match pts {
                    Ok(v) => v.into_iter().map(|(q, v)| (q * scale, v)).collect(),
                    Err(e) => {
                        if (scale - 1.0).abs() > 1e-12 {
                            return Err(e);
                        }
                        let _ = &p;
                        return Err(RegenError::Other(
                            "VTK point data: use a grid (STRUCTURED_POINTS) for a density body".into(),
                        ));
                    }
                };
                let m = PointMap::new(pts, 0.0, lo);
                let mut plo = DVec3::splat(f64::INFINITY);
                let mut phi = DVec3::splat(f64::NEG_INFINITY);
                for line in text.lines().skip(1) {
                    let f: Vec<f64> =
                        line.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.parse().ok()).collect();
                    if f.len() >= 3 {
                        let q = DVec3::new(f[0], f[1], f[2]) * scale;
                        plo = plo.min(q);
                        phi = phi.max(q);
                    }
                }
                let sp = ((phi - plo).length() / (m.len() as f64).cbrt()).max(1e-3);
                (Box::new(m), plo, phi, sp, format!("{} points", 0))
            }
        };
        let step = if res > 0.0 { res } else { spacing };
        let width = spacing * (smooth as f64 + 1.0) * 2.0;
        let body_field = Threshold { field, level, width };
        let pad = DVec3::splat(2.0 * step);
        let tris = anvil_implicit::mesh::surface_nets(&body_field, lo_p - pad, hi_p + pad, step);
        if tris.is_empty() {
            return Err(RegenError::Other(format!("nothing above {level}; the file's values run {lo} to {hi}")));
        }
        let out = anvil_kernel::ops::from_triangles(&tris, step * 1e-3);
        let vol = anvil_implicit::mesh::volume(&tris);
        Ok(FeatureOutput {
            bodies: vec![out],
            note: Some(format!(
                "{what}, values {lo:.3} to {hi:.3}, above {level}: {} triangles, {vol:.0} mm3",
                tris.len()
            )),
            ..Default::default()
        })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "density_body", label: "Density body", tab: "Solid", group: "Field", tooltip: "A body from a density or scalar grid (VTK) above a threshold, as from a topology optimisation", order: 92, create: || Box::new(DensityBodyFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn vtk_density_becomes_a_closed_body() {
        let dir = std::env::temp_dir().join(format!("anvil_density_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let n = 31;
        let h = 1.0;
        let mut s = format!(
            "# vtk DataFile Version 3.0\nd\nASCII\nDATASET STRUCTURED_POINTS\nDIMENSIONS {n} {n} {n}\nORIGIN 0 0 0\nSPACING {h} {h} {h}\nPOINT_DATA {}\nSCALARS density float 1\nLOOKUP_TABLE default\n",
            n * n * n
        );
        // A bar with a hole: density 1 in a box, 0 in a cylinder through it.
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let inside = i > 3 && i < 27 && j > 10 && j < 20 && k > 8 && k < 22;
                    let hole = ((i as f64 - 15.0).powi(2) + (k as f64 - 15.0).powi(2)).sqrt() < 4.0;
                    s.push_str(if inside && !hole { "1\n" } else { "0\n" });
                }
            }
        }
        let path = dir.join("bar.vtk");
        std::fs::write(&path, s).unwrap();
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(DensityBodyFeature {
            file: path.display().to_string(),
            smooth: "1".into(),
            ..Default::default()
        }));
        let f = &doc.features[0];
        assert!(f.error.is_none(), "{:?}", f.error);
        let b = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(b.open_edge_report().0, 0);
        let v = b.volume();
        // Box 23 x 9 x 13 less the hole: roughly 2700 less 450.
        assert!(v > 1600.0 && v < 2900.0, "{v}");
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("grid 31 x 31 x 31"), "{note}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
