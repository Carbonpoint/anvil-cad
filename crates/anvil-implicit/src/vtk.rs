//! Scalar data on a regular grid, and the legacy VTK reader that fills it
//! from a solver's output. A density from a topology optimisation or a
//! stress map on a voxel grid becomes a field: `GridField` interpolates
//! it, `Threshold` turns a density into a body, `GridField::smoothed`
//! takes the checkerboard out first.

use crate::pointmap::PointMap;
use crate::Field;
use anvil_math::DVec3;

/// A scalar on a regular grid, trilinear in between, clamped outside.
#[derive(Clone, Debug)]
pub struct GridField {
    pub origin: DVec3,
    pub spacing: DVec3,
    pub dims: [usize; 3],
    /// Values with x fastest, then y, then z (the VTK order).
    pub values: Vec<f64>,
}

impl GridField {
    pub fn new(origin: DVec3, spacing: DVec3, dims: [usize; 3], values: Vec<f64>) -> Result<GridField, String> {
        if dims.contains(&0) {
            return Err("grid has a zero dimension".into());
        }
        if values.len() != dims[0] * dims[1] * dims[2] {
            return Err(format!("grid wants {} values, got {}", dims[0] * dims[1] * dims[2], values.len()));
        }
        Ok(GridField { origin, spacing, dims, values })
    }

    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.dims[1] + j) * self.dims[0] + i
    }

    pub fn range(&self) -> (f64, f64) {
        self.values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)))
    }

    /// Far corner of the grid.
    pub fn extent(&self) -> DVec3 {
        self.origin
            + DVec3::new(
                (self.dims[0] - 1) as f64 * self.spacing.x,
                (self.dims[1] - 1) as f64 * self.spacing.y,
                (self.dims[2] - 1) as f64 * self.spacing.z,
            )
    }

    /// Box blur over `radius` cells in each direction, applied `passes`
    /// times (three passes approximate a Gaussian). Takes the
    /// checkerboard out of an optimiser's density.
    pub fn smoothed(&self, radius: usize, passes: usize) -> GridField {
        if radius == 0 || passes == 0 {
            return self.clone();
        }
        let mut v = self.values.clone();
        let [nx, ny, nz] = self.dims;
        let mut tmp = vec![0.0; v.len()];
        for _ in 0..passes {
            for axis in 0..3 {
                for k in 0..nz {
                    for j in 0..ny {
                        for i in 0..nx {
                            let mut sum = 0.0;
                            let mut n = 0.0;
                            for d in -(radius as i64)..=(radius as i64) {
                                let (ii, jj, kk) = match axis {
                                    0 => (i as i64 + d, j as i64, k as i64),
                                    1 => (i as i64, j as i64 + d, k as i64),
                                    _ => (i as i64, j as i64, k as i64 + d),
                                };
                                if ii < 0 || jj < 0 || kk < 0 || ii >= nx as i64 || jj >= ny as i64 || kk >= nz as i64 {
                                    continue;
                                }
                                sum += v[self.idx(ii as usize, jj as usize, kk as usize)];
                                n += 1.0;
                            }
                            tmp[self.idx(i, j, k)] = sum / n;
                        }
                    }
                }
                std::mem::swap(&mut v, &mut tmp);
            }
        }
        GridField { values: v, ..self.clone() }
    }
}

impl Field for GridField {
    fn at(&self, p: DVec3) -> f64 {
        let g = (p - self.origin) / self.spacing;
        let c = |x: f64, n: usize| x.clamp(0.0, (n as f64 - 1.0).max(0.0));
        let (x, y, z) = (c(g.x, self.dims[0]), c(g.y, self.dims[1]), c(g.z, self.dims[2]));
        let (i, j, k) = (x.floor() as usize, y.floor() as usize, z.floor() as usize);
        let (fx, fy, fz) = (x - i as f64, y - j as f64, z - k as f64);
        let at = |i: usize, j: usize, k: usize| {
            self.values[self.idx(i.min(self.dims[0] - 1), j.min(self.dims[1] - 1), k.min(self.dims[2] - 1))]
        };
        let mut acc = 0.0;
        for (di, wx) in [(0, 1.0 - fx), (1, fx)] {
            for (dj, wy) in [(0, 1.0 - fy), (1, fy)] {
                for (dk, wz) in [(0, 1.0 - fz), (1, fz)] {
                    let w = wx * wy * wz;
                    if w > 0.0 {
                        acc += w * at(i + di, j + dj, k + dk);
                    }
                }
            }
        }
        acc
    }
}

/// A body from a scalar: inside where `field` is above `level`. The
/// value is scaled by `width` so it reads as a distance over about one
/// transition width (use the grid spacing times the smoothing radius).
pub struct Threshold<F> {
    pub field: F,
    pub level: f64,
    pub width: f64,
}

impl<F: Field> Field for Threshold<F> {
    fn at(&self, p: DVec3) -> f64 {
        (self.level - self.field.at(p)) * self.width
    }
}

/// What a scalar file held.
pub enum ScalarData {
    Grid(GridField),
    Points(PointMap),
}

impl ScalarData {
    pub fn range(&self) -> (f64, f64) {
        match self {
            ScalarData::Grid(g) => g.range(),
            ScalarData::Points(p) => p.range(),
        }
    }
    pub fn into_field(self) -> Box<dyn Field + Send> {
        match self {
            ScalarData::Grid(g) => Box::new(g),
            ScalarData::Points(p) => Box::new(p),
        }
    }
}

/// Read a legacy ASCII VTK file: `STRUCTURED_POINTS` with `POINT_DATA`
/// scalars becomes a grid; `POLYDATA` or `UNSTRUCTURED_GRID` points with
/// scalars become a point map. The first SCALARS array is used.
pub fn parse_vtk(text: &str) -> Result<ScalarData, String> {
    let mut tokens = text.split_whitespace().peekable();
    let mut dims: Option<[usize; 3]> = None;
    let mut origin = DVec3::ZERO;
    let mut spacing = DVec3::ONE;
    let mut points: Vec<DVec3> = Vec::new();
    let mut n_point_data = 0usize;
    let mut scalars: Option<Vec<f64>> = None;
    let mut dataset = String::new();
    let num = |t: Option<&str>, what: &str| -> Result<f64, String> {
        t.ok_or(format!("{what}: missing"))?.parse::<f64>().map_err(|e| format!("{what}: {e}"))
    };
    while let Some(tok) = tokens.next() {
        match tok {
            "DATASET" => dataset = tokens.next().unwrap_or("").to_string(),
            "DIMENSIONS" => {
                let a = num(tokens.next(), "DIMENSIONS")? as usize;
                let b = num(tokens.next(), "DIMENSIONS")? as usize;
                let c = num(tokens.next(), "DIMENSIONS")? as usize;
                dims = Some([a, b, c]);
            }
            "ORIGIN" => {
                origin = DVec3::new(
                    num(tokens.next(), "ORIGIN")?,
                    num(tokens.next(), "ORIGIN")?,
                    num(tokens.next(), "ORIGIN")?,
                )
            }
            "SPACING" | "ASPECT_RATIO" => {
                spacing = DVec3::new(num(tokens.next(), tok)?, num(tokens.next(), tok)?, num(tokens.next(), tok)?)
            }
            "POINTS" => {
                let n = num(tokens.next(), "POINTS")? as usize;
                let _ty = tokens.next();
                points.reserve(n);
                for _ in 0..n {
                    points.push(DVec3::new(
                        num(tokens.next(), "point")?,
                        num(tokens.next(), "point")?,
                        num(tokens.next(), "point")?,
                    ));
                }
            }
            "POINT_DATA" => n_point_data = num(tokens.next(), "POINT_DATA")? as usize,
            "SCALARS" if scalars.is_none() => {
                let _name = tokens.next();
                let _ty = tokens.next();
                // Optional component count, then LOOKUP_TABLE name.
                let mut comps = 1usize;
                if let Some(&next) = tokens.peek() {
                    if next != "LOOKUP_TABLE" {
                        comps = next.parse::<usize>().unwrap_or(1);
                        tokens.next();
                    }
                }
                if tokens.peek() == Some(&"LOOKUP_TABLE") {
                    tokens.next();
                    tokens.next();
                }
                let n = if n_point_data > 0 {
                    n_point_data
                } else if let Some(d) = dims {
                    d[0] * d[1] * d[2]
                } else {
                    points.len()
                };
                let mut vals = Vec::with_capacity(n);
                for _ in 0..n {
                    let v = num(tokens.next(), "scalar")?;
                    for _ in 1..comps {
                        tokens.next();
                    }
                    vals.push(v);
                }
                scalars = Some(vals);
            }
            _ => {}
        }
    }
    let vals = scalars.ok_or("no SCALARS in the file")?;
    if dataset == "STRUCTURED_POINTS" || (dims.is_some() && points.is_empty()) {
        let d = dims.ok_or("no DIMENSIONS")?;
        Ok(ScalarData::Grid(GridField::new(origin, spacing, d, vals)?))
    } else {
        if points.len() != vals.len() {
            return Err(format!("{} points but {} scalars", points.len(), vals.len()));
        }
        Ok(ScalarData::Points(PointMap::new(points.into_iter().zip(vals).collect(), 0.0, 0.0)))
    }
}

/// Load a scalar file by extension: `.vtk` legacy, otherwise CSV.
pub fn load_scalar_file(path: &std::path::Path) -> Result<ScalarData, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("vtk")).unwrap_or(false) {
        parse_vtk(&text)
    } else {
        Ok(ScalarData::Points(PointMap::from_csv(&text, 0.0, 0.0)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{surface_nets, volume};

    fn sphere_density_vtk(n: usize, h: f64) -> String {
        let mut s = format!(
            "# vtk DataFile Version 3.0\ndensity\nASCII\nDATASET STRUCTURED_POINTS\nDIMENSIONS {n} {n} {n}\nORIGIN 0 0 0\nSPACING {h} {h} {h}\nPOINT_DATA {}\nSCALARS density float 1\nLOOKUP_TABLE default\n",
            n * n * n
        );
        let c = 0.5 * h * (n - 1) as f64;
        let r = 0.35 * h * (n - 1) as f64;
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let p = DVec3::new(i as f64 * h, j as f64 * h, k as f64 * h);
                    let d = (p - DVec3::splat(c)).length();
                    s.push_str(if d < r { "1\n" } else { "0\n" });
                }
            }
        }
        s
    }

    #[test]
    fn vtk_grid_thresholds_to_a_sphere() {
        let text = sphere_density_vtk(41, 0.5);
        let ScalarData::Grid(g) = parse_vtk(&text).unwrap() else { panic!("expected a grid") };
        assert_eq!(g.dims, [41, 41, 41]);
        assert_eq!(g.range(), (0.0, 1.0));
        let smooth = g.smoothed(1, 2);
        let field = Threshold { field: &smooth, level: 0.5, width: 2.0 };
        let tris = surface_nets(&field, DVec3::splat(-1.0), g.extent() + DVec3::splat(1.0), 0.5);
        let r = 0.35 * 0.5 * 40.0;
        let exact = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        let v = volume(&tris);
        assert!((v - exact).abs() / exact < 0.08, "{v} vs {exact}");
    }

    #[test]
    fn vtk_points_become_a_point_map() {
        let text = "# vtk DataFile Version 3.0\nstress\nASCII\nDATASET POLYDATA\nPOINTS 3 float\n0 0 0\n1 0 0\n0 1 0\nPOINT_DATA 3\nSCALARS stress float\nLOOKUP_TABLE default\n10 20 30\n";
        let ScalarData::Points(p) = parse_vtk(text).unwrap() else { panic!("expected points") };
        assert_eq!(p.len(), 3);
        assert!((p.at(DVec3::new(1.0, 0.0, 0.0)) - 20.0).abs() < 1e-9);
    }
}
