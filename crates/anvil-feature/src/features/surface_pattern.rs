//! Pattern on face: raised dots over one continuous curved surface.
//!
//! Pick the outer surface of a body of revolution, then add this feature.
//! Every facet that shares the picked surface is refined and displaced
//! along its normal, so the dots are part of one watertight mesh. The
//! inner surface and every other face stay as they were.
//!
//! Layouts: `hobnail` (arare) is one dot size in staggered rows with a
//! fixed count per row, so the dots shrink toward the narrow end and line
//! up in spiral columns. `tortoiseshell` (kikko) is a large dot ringed by
//! small dots in each cell. `machinery` is a procedural mesh of half
//! round tubes (rings, runs along the axis, diagonals), bosses, and bolt
//! heads, for the look of dense plumbing on a model at small scale.

use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_kernel::relief::{HeightField, RevolvedParam};
use anvil_kernel::{KernelError, Surface};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurfacePatternFeature {
    pub body: usize,
    /// Curved surface id (the `Surface::Revolved` tag) on that body.
    pub surface: String,
    pub layout: String,
    /// Dots per row around the axis. `0` picks the count from `pitch` at
    /// the wide end of the surface.
    pub columns: String,
    /// Distance between dot centres along a row, at the wide end.
    pub pitch: String,
    /// Dot diameter at the wide end.
    pub dot: String,
    /// Dot diameter at the far end of the surface. `0` scales the dots
    /// with the local radius instead.
    pub dot_end: String,
    pub height: String,
    /// Mesh step of the refined surface.
    pub step: String,
    /// Dot free band at both ends of the surface.
    pub margin: String,
    /// Seed of the machinery layout; another seed gives another mesh.
    #[serde(default = "one_expr")]
    pub seed: String,
}

fn one_expr() -> String {
    "1".into()
}

impl Default for SurfacePatternFeature {
    fn default() -> Self {
        SurfacePatternFeature {
            body: 0,
            surface: "0".into(),
            layout: "hobnail".into(),
            columns: "0".into(),
            pitch: "4.5".into(),
            dot: "3".into(),
            dot_end: "0".into(),
            height: "1".into(),
            step: "0.6".into(),
            margin: "2".into(),
            seed: "1".into(),
        }
    }
}

/// One raised dot on the surface, in `(s, theta)` with a world radius.
#[derive(Clone, Copy, Debug)]
pub struct Dot {
    pub s: f64,
    pub theta: f64,
    /// Radius of the dot footprint on the surface.
    pub rho: f64,
    pub height: f64,
    /// Surface radius at the dot, for the arc distance around the axis.
    pub r: f64,
}

/// A height field built from dots, with a grid so lookups stay cheap.
pub struct DotField {
    cells: HashMap<(i64, i64), Vec<Dot>>,
    cell_s: f64,
    cell_t: f64,
    r_max: f64,
    pub count: usize,
    /// Dots dropped because they reached a fixed part of the surface.
    pub dropped: usize,
}

impl DotField {
    pub fn new(dots: Vec<Dot>, r_max: f64) -> Self {
        let rho_max = dots.iter().map(|d| d.rho).fold(0.1, f64::max);
        let cell_s = rho_max * 2.0;
        let cell_t = (rho_max * 2.0 / r_max.max(1e-6)).min(std::f64::consts::PI);
        let mut cells: HashMap<(i64, i64), Vec<Dot>> = HashMap::new();
        for d in &dots {
            cells.entry(((d.s / cell_s).floor() as i64, (d.theta / cell_t).floor() as i64)).or_default().push(*d);
        }
        DotField { cells, cell_s, cell_t, r_max, count: dots.len(), dropped: 0 }
    }

    fn cell_of(&self, s: f64, theta: f64) -> (i64, i64) {
        ((s / self.cell_s).floor() as i64, (theta / self.cell_t).floor() as i64)
    }

    /// Spherical cap height of the nearest dot at `(s, theta)`.
    pub fn height_at(&self, s: f64, theta: f64) -> f64 {
        let ks = (s / self.cell_s).floor() as i64;
        let kt = (theta / self.cell_t).floor() as i64;
        let nt = (std::f64::consts::TAU / self.cell_t).ceil() as i64;
        let mut best = 0.0f64;
        for ds in -1..=1 {
            for dt in -2..=2 {
                let key = (ks + ds, (kt + dt).rem_euclid(nt));
                let Some(list) = self.cells.get(&key) else { continue };
                for d in list {
                    let mut dth = theta - d.theta;
                    while dth > std::f64::consts::PI {
                        dth -= std::f64::consts::TAU;
                    }
                    while dth < -std::f64::consts::PI {
                        dth += std::f64::consts::TAU;
                    }
                    let dist2 = (s - d.s).powi(2) + (d.r * dth).powi(2);
                    if dist2 >= d.rho * d.rho {
                        continue;
                    }
                    // A ball of radius R pressed in to depth h.
                    let big_r = (d.rho * d.rho + d.height * d.height) / (2.0 * d.height);
                    let h = (big_r * big_r - dist2).sqrt() - (big_r - d.height);
                    best = best.max(h);
                }
            }
        }
        best
    }
}

impl HeightField for DotField {
    fn height(&self, s: f64, theta: f64) -> f64 {
        self.height_at(s, theta)
    }

    /// Drop every dot whose footprint reaches a fixed vertex, so a dot is
    /// either whole or absent. Fixed vertices sit on the edges of cut
    /// facets and on the region boundary, about one mesh step apart.
    fn exclude_near(&mut self, fixed: &[(f64, f64)], _param: &RevolvedParam) {
        let nt = (std::f64::consts::TAU / self.cell_t).ceil() as i64;
        let mut doomed: Vec<(i64, i64, usize)> = Vec::new();
        let debug = std::env::var("ANVIL_RELIEF_DEBUG").is_ok();
        for &(s, theta) in fixed {
            let (ks, kt) = self.cell_of(s, theta);
            for ds in -1..=1 {
                for dt in -2..=2 {
                    let key = (ks + ds, (kt + dt).rem_euclid(nt));
                    let Some(list) = self.cells.get(&key) else { continue };
                    for (n, d) in list.iter().enumerate() {
                        let mut dth = theta - d.theta;
                        while dth > std::f64::consts::PI {
                            dth -= std::f64::consts::TAU;
                        }
                        while dth < -std::f64::consts::PI {
                            dth += std::f64::consts::TAU;
                        }
                        let dist2 = (s - d.s).powi(2) + (d.r * dth).powi(2);
                        // A small margin so a dot never ends exactly on a fixed edge.
                        let reach = d.rho * 1.05;
                        if dist2 < reach * reach {
                            doomed.push((key.0, key.1, n));
                        }
                    }
                }
            }
        }
        doomed.sort_unstable();
        doomed.dedup();
        // Remove from the back of each cell list so indices stay valid.
        for (a, b, n) in doomed.into_iter().rev() {
            if let Some(list) = self.cells.get_mut(&(a, b)) {
                if n < list.len() {
                    let d = list.remove(n);
                    if debug {
                        eprintln!("  dropped dot at theta {:.1} deg, s {:.1}, r {:.1}", d.theta.to_degrees(), d.s, d.r);
                    }
                    self.dropped += 1;
                }
            }
        }
        self.count -= self.dropped.min(self.count);
        let _ = self.r_max;
    }
}

/// A half round tube on the surface between two `(s, theta)` points.
/// `theta` may run past a full turn so a ring arc can cross zero.
#[derive(Clone, Copy, Debug)]
pub struct Tube {
    pub s0: f64,
    pub t0: f64,
    pub s1: f64,
    pub t1: f64,
    pub rho: f64,
    pub height: f64,
    /// Surface radius, for the arc distance around the axis.
    pub r: f64,
}

impl Tube {
    /// Distance from `(s, theta)` to the tube centreline, on the surface.
    fn distance(&self, s: f64, theta: f64) -> f64 {
        let (u0, v0) = (self.s0, self.t0 * self.r);
        let (u1, v1) = (self.s1, self.t1 * self.r);
        let full = std::f64::consts::TAU * self.r;
        let mut best = f64::INFINITY;
        for k in -1..=1 {
            let (u, v) = (s, theta * self.r + k as f64 * full);
            let (du, dv) = (u1 - u0, v1 - v0);
            let len2 = du * du + dv * dv;
            let t = if len2 > 1e-12 { ((u - u0) * du + (v - v0) * dv) / len2 } else { 0.0 }.clamp(0.0, 1.0);
            let (pu, pv) = (u0 + t * du, v0 + t * dv);
            best = best.min(((u - pu).powi(2) + (v - pv).powi(2)).sqrt());
        }
        best
    }
}

/// Tubes plus bosses: the machinery layout.
pub struct MachineryField {
    pub tubes: Vec<Tube>,
    pub bosses: DotField,
    pub dropped_tubes: usize,
}

impl HeightField for MachineryField {
    fn height(&self, s: f64, theta: f64) -> f64 {
        let mut best = self.bosses.height_at(s, theta);
        for t in &self.tubes {
            let d = t.distance(s, theta);
            if d < t.rho {
                let h = t.height * (1.0 - (d / t.rho).powi(2)).sqrt();
                best = best.max(h);
            }
        }
        best
    }
    fn exclude_near(&mut self, fixed: &[(f64, f64)], param: &RevolvedParam) {
        self.bosses.exclude_near(fixed, param);
        let before = self.tubes.len();
        self.tubes.retain(|t| fixed.iter().all(|&(s, th)| t.distance(s, th) >= t.rho * 1.05));
        self.dropped_tubes = before - self.tubes.len();
    }
}

/// A small deterministic random source (LCG), so a seed always gives the
/// same mesh.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

/// Lay out the machinery mesh: ring arcs every `pitch` along the axis,
/// runs along the axis every `pitch` around it, some diagonals, bosses,
/// and bolt heads. `dot` is the tube diameter, `height` the tube height.
pub fn layout_machinery(
    param: &RevolvedParam,
    pitch: f64,
    dot: f64,
    height: f64,
    margin: f64,
    seed: u64,
) -> MachineryField {
    use std::f64::consts::TAU;
    let len = param.length();
    let s0 = margin.max(0.0);
    let s1 = len - margin.max(0.0);
    let r_max = param.run.iter().map(|p| p.x).fold(0.0, f64::max).max(1e-6);
    let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut tubes = Vec::new();
    let mut dots = Vec::new();
    if s1 <= s0 || pitch <= 0.0 || dot <= 0.0 {
        return MachineryField { tubes, bosses: DotField::new(dots, r_max), dropped_tubes: 0 };
    }
    let rho = dot * 0.5;
    let rows = ((s1 - s0) / pitch).floor().max(1.0) as usize;
    let row_pitch = (s1 - s0) / rows as f64;
    // Ring arcs: two or three per row, each 40 to 200 degrees.
    for j in 0..rows {
        let s = s0 + row_pitch * (j as f64 + 0.5) + rng.range(-0.25, 0.25) * row_pitch;
        let r = param.radius(s).max(1e-6);
        let arcs = 2 + (rng.next() * 2.0) as usize;
        let mut theta = rng.range(0.0, TAU);
        for _ in 0..arcs {
            let span = rng.range(0.7, 3.5);
            let scale = rng.range(0.55, 1.0);
            tubes.push(Tube { s0: s, t0: theta, s1: s, t1: theta + span, rho: rho * scale, height: height * scale, r });
            theta += span + rng.range(0.3, 1.2);
        }
    }
    // Runs along the axis.
    let cols = ((TAU * r_max / pitch).round() as usize).max(3);
    for i in 0..cols {
        let theta = (i as f64 + rng.range(0.1, 0.9)) * TAU / cols as f64;
        let a = s0 + rng.range(0.0, 0.5) * (s1 - s0);
        let b = (a + rng.range(0.25, 0.7) * (s1 - s0)).min(s1);
        let r = param.radius((a + b) * 0.5).max(1e-6);
        let scale = rng.range(0.5, 0.9);
        tubes.push(Tube { s0: a, t0: theta, s1: b, t1: theta, rho: rho * scale, height: height * scale, r });
        // Every other run gets a bend: a short diagonal off its end.
        if i % 2 == 0 {
            let dth = rng.range(-1.0, 1.0) * pitch / r;
            let ds = rng.range(0.15, 0.35) * (s1 - s0);
            let (c, d) = if b + ds <= s1 { (b, b + ds) } else { (a, (a - ds).max(s0)) };
            tubes.push(Tube { s0: c, t0: theta, s1: d, t1: theta + dth, rho: rho * scale, height: height * scale, r });
        }
    }
    // Bosses and bolt heads.
    let area = TAU * r_max * (s1 - s0);
    let n_boss = (area / (pitch * pitch) * 0.6) as usize;
    for _ in 0..n_boss {
        let s = rng.range(s0 + rho, s1 - rho);
        let theta = rng.range(0.0, TAU);
        let r = param.radius(s).max(1e-6);
        let big = rng.next() < 0.3;
        let size = if big { rho * rng.range(1.4, 2.2) } else { rho * rng.range(0.35, 0.6) };
        let h = if big { height * rng.range(0.8, 1.3) } else { height * 0.5 };
        dots.push(Dot { s, theta, rho: size, height: h, r });
    }
    MachineryField { tubes, bosses: DotField::new(dots, r_max), dropped_tubes: 0 }
}

/// Lay out dots on a surface of revolution.
#[allow(clippy::too_many_arguments)]
pub fn layout_dots(
    param: &RevolvedParam,
    layout: &str,
    columns: usize,
    pitch: f64,
    dot: f64,
    dot_end: f64,
    height: f64,
    margin: f64,
) -> Vec<Dot> {
    let len = param.length();
    let s0 = margin.max(0.0);
    let s1 = len - margin.max(0.0);
    let mut dots = Vec::new();
    if s1 <= s0 || pitch <= 0.0 || dot <= 0.0 {
        return dots;
    }
    // Start rows at the wider end so the count per row fits the widest ring.
    let r_start = param.radius(s0);
    let r_end = param.radius(s1);
    let (from, to) = if r_start >= r_end { (s0, s1) } else { (s1, s0) };
    let r0 = param.radius(from).max(1e-6);
    let n = if columns > 0 { columns } else { ((std::f64::consts::TAU * r0 / pitch).round() as usize).max(3) };
    let row_pitch = pitch * 0.866;
    let rows = ((s1 - s0) / row_pitch).floor() as usize + 1;
    let dir = if to >= from { 1.0 } else { -1.0 };
    for j in 0..rows {
        let s = from + dir * row_pitch * j as f64;
        if !(s0..=s1).contains(&s) {
            continue;
        }
        let r = param.radius(s).max(1e-6);
        let frac = if (s1 - s0) > 1e-9 { (s - from).abs() / (s1 - s0) } else { 0.0 };
        let size = if dot_end > 0.0 { dot + (dot_end - dot) * frac } else { (dot * r / r0).max(dot * 0.35) };
        let h = height * (size / dot).clamp(0.2, 1.5);
        let stagger = if j % 2 == 1 { 0.5 } else { 0.0 };
        for i in 0..n {
            let theta = (i as f64 + stagger) * std::f64::consts::TAU / n as f64;
            match layout {
                "tortoiseshell" => {
                    // Big centre dot plus a ring of small dots in each cell.
                    let cell = std::f64::consts::TAU * r / n as f64;
                    let big = (size * 0.5).min(cell * 0.22);
                    dots.push(Dot { s, theta, rho: big, height: h, r });
                    let ring_r = cell * 0.36;
                    let small = cell * 0.07;
                    for k in 0..10 {
                        let a = k as f64 / 10.0 * std::f64::consts::TAU;
                        let ds = ring_r * a.cos();
                        let dt = ring_r * a.sin() / r;
                        dots.push(Dot { s: s + ds, theta: theta + dt, rho: small, height: h * 0.5, r });
                    }
                }
                _ => dots.push(Dot { s, theta, rho: size * 0.5, height: h, r }),
            }
        }
    }
    for d in &mut dots {
        d.theta = d.theta.rem_euclid(std::f64::consts::TAU);
    }
    dots
}

#[typetag::serde(name = "surface_pattern")]
impl Feature for SurfacePatternFeature {
    fn kind(&self) -> &'static str {
        "surface_pattern"
    }
    fn name(&self) -> String {
        format!("Pattern on face ({} {} mm)", self.layout, self.dot)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            ParamSpec::feature_ref("body", "Body", crate::BODY_TYPES.to_vec(), self.body),
            ParamSpec::length("surface", "Surface id (pick a face)", &self.surface),
            ParamSpec::choice("layout", "Layout", vec!["hobnail", "tortoiseshell", "machinery"], &self.layout),
            ParamSpec::length("pitch", "Pitch", &self.pitch),
            ParamSpec::length("columns", "Dots per row (0 = from pitch)", &self.columns),
            ParamSpec::length("dot", "Dot diameter", &self.dot),
            ParamSpec::length("dot_end", "Dot diameter at far end (0 = scale)", &self.dot_end),
            ParamSpec::length("height", "Dot height", &self.height),
            ParamSpec::length("margin", "Margin at ends", &self.margin),
            ParamSpec::length("step", "Mesh step", &self.step),
            ParamSpec::length("seed", "Seed (machinery)", &self.seed),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("body", ParamValue::FeatureRef(i)) => self.body = i,
            ("surface", ParamValue::Expr(s)) => self.surface = s,
            ("layout", ParamValue::Choice(s)) => self.layout = s,
            ("pitch", ParamValue::Expr(s)) => self.pitch = s,
            ("columns", ParamValue::Expr(s)) => self.columns = s,
            ("dot", ParamValue::Expr(s)) => self.dot = s,
            ("dot_end", ParamValue::Expr(s)) => self.dot_end = s,
            ("height", ParamValue::Expr(s)) => self.height = s,
            ("margin", ParamValue::Expr(s)) => self.margin = s,
            ("step", ParamValue::Expr(s)) => self.step = s,
            ("seed", ParamValue::Expr(s)) => self.seed = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn place_on_surface(&mut self, surface: Surface, body: usize) -> bool {
        match surface {
            Surface::Revolved { id } => {
                self.surface = id.to_string();
                self.body = body;
                true
            }
            _ => false,
        }
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let surface = ctx.eval(&self.surface)?.round().max(0.0) as u32;
        let columns = ctx.eval(&self.columns)?.round().max(0.0) as usize;
        let pitch = ctx.eval(&self.pitch)?;
        let dot = ctx.eval(&self.dot)?;
        let dot_end = ctx.eval(&self.dot_end)?;
        let height = ctx.eval(&self.height)?;
        let step = ctx.eval(&self.step)?;
        let margin = ctx.eval(&self.margin)?;
        let seed = ctx.eval(&self.seed)?.round().max(0.0) as u64;
        let src = ctx.bodies_of(self.body)?;
        let tag = Surface::Revolved { id: surface };
        let mut bodies = Vec::with_capacity(src.len());
        let mut note = None;
        for b in src {
            let Some(geom) = b.surfaces.get(&surface) else {
                bodies.push(b.clone());
                continue;
            };
            let Some(first) = b.faces.values().find(|f| f.surface == tag) else {
                bodies.push(b.clone());
                continue;
            };
            let at = first.outer.iter().map(|&v| b.pos(v)).sum::<anvil_math::DVec3>() / first.outer.len() as f64;
            let param = RevolvedParam::new(geom, Some((at, b.face_normal(first))))
                .ok_or_else(|| KernelError::InvalidInput("surface run is too short".into()))?;
            if self.layout == "machinery" {
                let mut field = layout_machinery(&param, pitch, dot, height, margin, seed);
                let out = ctx.kernel.relief(b, surface, step, &mut field)?;
                note = Some(format!(
                    "{} tubes ({} dropped at cut edges), {} bosses, {} faces",
                    field.tubes.len(),
                    field.dropped_tubes,
                    field.bosses.count,
                    out.faces.len()
                ));
                bodies.push(out);
                continue;
            }
            let dots = layout_dots(&param, &self.layout, columns, pitch, dot, dot_end, height, margin);
            let r_max = param.run.iter().map(|p| p.x).fold(0.0, f64::max);
            let mut field = DotField::new(dots, r_max);
            let out = ctx.kernel.relief(b, surface, step, &mut field)?;
            note = Some(format!(
                "{} dots ({} dropped at cut edges), {} faces",
                field.count,
                field.dropped,
                out.faces.len()
            ));
            bodies.push(out);
        }
        if note.is_none() {
            return Err(RegenError::Other(
                "pick a curved face of revolution on the body first (click the face, then Pattern on face)".into(),
            ));
        }
        Ok(FeatureOutput { bodies, note, consumes: vec![self.body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    FeatureDescriptor {
        id: "surface_pattern",
        label: "Pattern on face",
        tab: "Solid",
        group: "Modify",
        tooltip: "Raised dots (hobnail, tortoiseshell) or a machinery mesh of tubes over one curved surface",
        order: 75,
        create: || Box::new(SurfacePatternFeature::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::revolve::RevolveFeature;
    use crate::features::sketch::SketchFeature;
    use crate::Document;
    use anvil_math::DVec2;

    #[test]
    fn hobnail_on_a_cup_outer_wall() {
        let mut doc = Document::new("cup");
        // A polyline cup: flat bottom, straight outer wall, rim, inner wall.
        let mut sk = SketchFeature::on_datum("XZ");
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(30.0, 0.0),
            DVec2::new(30.0, 40.0),
            DVec2::new(27.0, 40.0),
            DVec2::new(27.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        let ids: Vec<_> = pts.iter().map(|p| sk.sketch.add_point(p.x, p.y)).collect();
        for i in 0..ids.len() {
            sk.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
        }
        doc.add_feature(Box::new(sk));
        doc.add_feature(Box::new(RevolveFeature { sketch: 0, axis: "Y".into(), ..Default::default() }));
        assert!(doc.features[1].error.is_none(), "{:?}", doc.features[1].error);
        let body = &doc.features[1].output.as_ref().unwrap().bodies[0];
        let outer = *body
            .surfaces
            .iter()
            .find(|(_, g)| {
                let anvil_kernel::SurfaceGeom::Revolved { run, .. } = g;
                run.iter().all(|p| (p.x - 30.0).abs() < 1e-9)
            })
            .map(|(k, _)| k)
            .expect("outer wall");
        let v0 = body.volume();
        let faces0 = body.faces.len();
        let mut pat = SurfacePatternFeature { body: 1, ..Default::default() };
        assert!(pat.place_on_surface(Surface::Revolved { id: outer }, 1));
        doc.add_feature(Box::new(pat));
        let f = &doc.features[2];
        assert!(f.error.is_none(), "{:?}", f.error);
        let out = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(out.open_edge_report(), (0, 0), "watertight");
        assert!(out.volume() > v0, "dots add volume");
        assert!(out.faces.len() > faces0 * 10, "surface was refined");
        assert!(f.output.as_ref().unwrap().note.as_deref().unwrap_or("").contains("dots"));
        // Only one body is visible: the pattern consumed the revolve.
        assert_eq!(doc.bodies().len(), 1);
    }

    #[test]
    fn machinery_on_a_drum() {
        let mut doc = Document::new("drum");
        let mut sk = SketchFeature::on_datum("XZ");
        let pts = [DVec2::new(0.0, 0.0), DVec2::new(22.0, 0.0), DVec2::new(22.0, 25.0), DVec2::new(0.0, 25.0)];
        let ids: Vec<_> = pts.iter().map(|p| sk.sketch.add_point(p.x, p.y)).collect();
        for i in 0..ids.len() {
            sk.sketch.add_line(ids[i], ids[(i + 1) % ids.len()]);
        }
        doc.add_feature(Box::new(sk));
        doc.add_feature(Box::new(RevolveFeature { sketch: 0, axis: "Y".into(), ..Default::default() }));
        let body = &doc.features[1].output.as_ref().unwrap().bodies[0];
        let wall = *body
            .surfaces
            .iter()
            .find(|(_, g)| {
                let anvil_kernel::SurfaceGeom::Revolved { run, .. } = g;
                run.iter().all(|p| (p.x - 22.0).abs() < 1e-9)
            })
            .map(|(k, _)| k)
            .expect("wall");
        let v0 = body.volume();
        let mut pat = SurfacePatternFeature {
            body: 1,
            layout: "machinery".into(),
            pitch: "4".into(),
            dot: "2.2".into(),
            height: "1.1".into(),
            step: "0.5".into(),
            margin: "2.5".into(),
            seed: "7".into(),
            ..Default::default()
        };
        assert!(pat.place_on_surface(Surface::Revolved { id: wall }, 1));
        doc.add_feature(Box::new(pat));
        let f = &doc.features[2];
        assert!(f.error.is_none(), "{:?}", f.error);
        let out = &f.output.as_ref().unwrap().bodies[0];
        assert_eq!(out.open_edge_report(), (0, 0), "watertight");
        let added = out.volume() - v0;
        // The mesh adds a few percent of the drum's volume, never more.
        assert!(added > 0.005 * v0 && added < 0.15 * v0, "added {added} of {v0}");
        let note = f.output.as_ref().unwrap().note.clone().unwrap_or_default();
        assert!(note.contains("tubes") && note.contains("bosses"), "{note}");
        // The same seed gives the same mesh; another seed differs.
        let mut again = doc.clone();
        again.features.truncate(2);
        let mut pat2 = SurfacePatternFeature {
            body: 1,
            layout: "machinery".into(),
            pitch: "4".into(),
            dot: "2.2".into(),
            height: "1.1".into(),
            step: "0.5".into(),
            margin: "2.5".into(),
            seed: "8".into(),
            ..Default::default()
        };
        assert!(pat2.place_on_surface(Surface::Revolved { id: wall }, 1));
        again.add_feature(Box::new(pat2));
        let out2 = &again.features[2].output.as_ref().unwrap().bodies[0];
        assert!((out2.volume() - out.volume()).abs() > 1e-6, "seed changes the mesh");
    }
}
