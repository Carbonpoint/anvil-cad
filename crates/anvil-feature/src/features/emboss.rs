//! Text, QR Code, Surface Texture, and Press Pull (face extrude).
//!
//! All four make separate bodies, which is what a multi-material slicer
//! needs: the card, the letters, and the code are exported as parts.

use crate::features::datum_plane;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError, PLANE_TYPES};
use anvil_math::{DVec2, Plane};
use serde::{Deserialize, Serialize};

/// Bundled font (DejaVu Sans, Bitstream Vera licence, see assets/).
pub const DEFAULT_FONT: &[u8] = crate::fonts::BUILTIN[0].1;

fn text_param(name: &'static str, label: &'static str, v: &str) -> ParamSpec {
    ParamSpec { name, label, kind: crate::param::ParamKind::Text, value: ParamValue::Expr(v.to_string()) }
}

pub(crate) fn resolve_plane_face(
    plane: &str,
    feature: Option<usize>,
    face: Option<Plane>,
    ctx: &RegenContext,
) -> Result<Plane, RegenError> {
    if plane == "Face" {
        return face.ok_or_else(|| RegenError::Other("no face stored; select a face and add the feature again".into()));
    }
    resolve_plane(plane, feature, ctx)
}

fn resolve_plane(plane: &str, feature: Option<usize>, ctx: &RegenContext) -> Result<Plane, RegenError> {
    match (plane, feature) {
        ("Feature", Some(i)) => ctx.plane_of(i),
        ("Feature", None) => Err(RegenError::Other("choose a plane feature".into())),
        (d, _) => Ok(datum_plane(d)),
    }
}

fn point_in_poly(p: DVec2, poly: &[DVec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Group closed loops into (outer, holes) by containment depth. Loops at
/// even depth are outers; loops at odd depth are holes of their parent.
pub fn nest_loops(loops: &[Vec<DVec2>]) -> Vec<(Vec<DVec2>, Vec<Vec<DVec2>>)> {
    let n = loops.len();
    // A loop counts as inside another when most of its sample vertices are,
    // so a hole that touches the outer loop at one vertex still nests.
    let inside = |i: usize, j: usize| -> bool {
        let li = &loops[i];
        let step = (li.len() / 5).max(1);
        let samples: Vec<DVec2> = li.iter().step_by(step).take(5).copied().collect();
        let hits = samples.iter().filter(|&&p| point_in_poly(p, &loops[j])).count();
        hits * 2 > samples.len()
    };
    let depth: Vec<usize> =
        (0..n).map(|i| (0..n).filter(|&j| j != i && loops[j].len() >= 3 && inside(i, j)).count()).collect();
    let mut out: Vec<(usize, Vec<DVec2>, Vec<Vec<DVec2>>)> = Vec::new();
    for i in 0..n {
        if depth[i].is_multiple_of(2) {
            out.push((i, loops[i].clone(), Vec::new()));
        }
    }
    for i in 0..n {
        if !depth[i].is_multiple_of(2) {
            // Parent: the outer at depth[i]-1 that contains it, smallest area.
            let mut best: Option<(usize, f64)> = None;
            for (k, (oi, o, _)) in out.iter().enumerate() {
                if depth[*oi] + 1 == depth[i] && inside(i, *oi) {
                    let _ = o;
                    let area = polygon_area(o).abs();
                    if best.is_none_or(|(_, a)| area < a) {
                        best = Some((k, area));
                    }
                }
            }
            if let Some((k, _)) = best {
                out[k].2.push(loops[i].clone());
            }
        }
    }
    out.into_iter().map(|(_, o, h)| (o, h)).collect()
}

fn zero() -> String {
    "0".into()
}

fn perimeter(p: &[DVec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| (p[(i + 1) % n] - p[i]).length()).sum()
}

/// Grow a closed loop by `d` (negative shrinks), independent of winding.
/// Corner mitres are clamped so sharp glyph corners do not spike.
pub fn offset_loop(poly: &[DVec2], d: f64) -> Vec<DVec2> {
    let n = poly.len();
    let sign = if polygon_area(poly) >= 0.0 { 1.0 } else { -1.0 };
    (0..n)
        .map(|i| {
            let p0 = poly[(i + n - 1) % n];
            let p1 = poly[i];
            let p2 = poly[(i + 1) % n];
            let d1 = (p1 - p0).normalize_or_zero();
            let d2 = (p2 - p1).normalize_or_zero();
            let n1 = DVec2::new(d1.y, -d1.x) * sign;
            let n2 = DVec2::new(d2.y, -d2.x) * sign;
            let bis = (n1 + n2).normalize_or_zero();
            if bis == DVec2::ZERO {
                return p1 + n1 * d;
            }
            let cos_half = bis.dot(n1).max(0.5);
            p1 + bis * (d / cos_half)
        })
        .collect()
}

fn polygon_area(p: &[DVec2]) -> f64 {
    let n = p.len();
    0.5 * (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum::<f64>()
}

// ---------------------------------------------------------------- text

struct Outline {
    contours: Vec<Vec<DVec2>>,
    cur: Vec<DVec2>,
    scale: f64,
    offset: DVec2,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.cur.push(self.offset + DVec2::new(x as f64, y as f64) * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur.push(self.offset + DVec2::new(x as f64, y as f64) * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = *self.cur.last().unwrap_or(&DVec2::ZERO);
        let c = self.offset + DVec2::new(x1 as f64, y1 as f64) * self.scale;
        let p1 = self.offset + DVec2::new(x as f64, y as f64) * self.scale;
        for i in 1..=6 {
            let t = i as f64 / 6.0;
            let q = p0 * (1.0 - t) * (1.0 - t) + c * 2.0 * (1.0 - t) * t + p1 * t * t;
            self.cur.push(q);
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = *self.cur.last().unwrap_or(&DVec2::ZERO);
        let c1 = self.offset + DVec2::new(x1 as f64, y1 as f64) * self.scale;
        let c2 = self.offset + DVec2::new(x2 as f64, y2 as f64) * self.scale;
        let p1 = self.offset + DVec2::new(x as f64, y as f64) * self.scale;
        for i in 1..=8 {
            let t = i as f64 / 8.0;
            let u = 1.0 - t;
            self.cur.push(p0 * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + p1 * t * t * t);
        }
    }
    fn close(&mut self) {
        if self.cur.len() >= 3 {
            let mut c = std::mem::take(&mut self.cur);
            if (c[0] - *c.last().unwrap()).length() < 1e-9 {
                c.pop();
            }
            self.contours.push(c);
        } else {
            self.cur.clear();
        }
    }
}

/// Glyph outlines for a string: one entry per glyph, each a list of
/// closed loops in mm. `size` is the em height in mm.
pub fn text_outlines(
    font_data: &[u8],
    text: &str,
    size: f64,
    align_center: bool,
) -> Result<Vec<Vec<Vec<DVec2>>>, String> {
    let face = ttf_parser::Face::parse(font_data, 0).map_err(|e| format!("font: {e}"))?;
    let scale = size / face.units_per_em() as f64;
    let mut glyphs = Vec::new();
    let mut pen = 0.0;
    for ch in text.chars() {
        let Some(gid) = face.glyph_index(ch) else {
            pen += size * 0.3;
            continue;
        };
        let mut ob = Outline { contours: Vec::new(), cur: Vec::new(), scale, offset: DVec2::new(pen, 0.0) };
        face.outline_glyph(gid, &mut ob);
        ttf_parser::OutlineBuilder::close(&mut ob);
        if !ob.contours.is_empty() {
            glyphs.push(ob.contours);
        }
        pen += face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale;
    }
    if align_center {
        let shift = DVec2::new(-pen / 2.0, 0.0);
        for g in &mut glyphs {
            for c in g {
                for p in c {
                    *p += shift;
                }
            }
        }
    }
    Ok(glyphs)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextFeature {
    pub text: String,
    pub font_path: String,
    pub plane: String,
    pub plane_feature: Option<usize>,
    /// Plane of a picked face when `plane` is "Face".
    #[serde(default)]
    pub face_plane: Option<Plane>,
    pub x: String,
    pub y: String,
    pub z: String,
    pub size: String,
    pub height: String,
    pub center: bool,
    /// Grow every stroke outward by this distance (mm), to make thin fonts
    /// printable. Zero keeps the font as drawn.
    #[serde(default = "zero")]
    pub thicken: String,
    /// "new" (separate bodies), "join", or "cut" into `target`.
    #[serde(default = "crate::features::extrude::default_op")]
    pub operation: String,
    #[serde(default)]
    pub target: usize,
}

impl Default for TextFeature {
    fn default() -> Self {
        TextFeature {
            text: "Anvil".into(),
            font_path: String::new(),
            plane: "XY".into(),
            plane_feature: None,
            face_plane: None,
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            size: "8".into(),
            height: "1".into(),
            center: true,
            thicken: "0".into(),
            operation: "new".into(),
            target: 0,
        }
    }
}

#[typetag::serde(name = "text")]
impl Feature for TextFeature {
    fn kind(&self) -> &'static str {
        "text"
    }
    fn name(&self) -> String {
        format!("Text \"{}\" ({})", self.text, crate::fonts::display_name(&self.font_path))
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            text_param("text", "Text", &self.text),
            ParamSpec {
                name: "font_path",
                label: "Font",
                kind: crate::param::ParamKind::Font,
                value: ParamValue::Expr(self.font_path.clone()),
            },
            ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature", "Face"], &self.plane),
            ParamSpec::length("x", "X on plane", &self.x),
            ParamSpec::length("y", "Y on plane", &self.y),
            ParamSpec::length("z", "Offset along normal", &self.z),
            ParamSpec::length("size", "Font size (em)", &self.size),
            ParamSpec::length("height", "Emboss height", &self.height),
            ParamSpec::boolean("center", "Centre horizontally", self.center),
            ParamSpec::length("thicken", "Thicken strokes (mm)", &self.thicken),
            ParamSpec::choice("operation", "Operation", vec!["new", "join", "cut"], &self.operation),
        ];
        if self.operation != "new" {
            v.push(ParamSpec::feature_ref("target", "Target body", crate::BODY_TYPES.to_vec(), self.target));
        }
        if self.plane == "Feature" {
            v.push(ParamSpec::feature_ref(
                "plane_feature",
                "Plane feature",
                PLANE_TYPES.to_vec(),
                self.plane_feature.unwrap_or(0),
            ));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("operation", ParamValue::Choice(o)) => self.operation = o,
            ("target", ParamValue::FeatureRef(i)) => self.target = i,
            ("text", ParamValue::Expr(s)) => self.text = s,
            ("font_path", ParamValue::Expr(s)) => self.font_path = s,
            ("plane", ParamValue::Choice(p)) => self.plane = p,
            ("plane_feature", ParamValue::FeatureRef(i)) => self.plane_feature = Some(i),
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("z", ParamValue::Expr(s)) => self.z = s,
            ("size", ParamValue::Expr(s)) => self.size = s,
            ("height", ParamValue::Expr(s)) => self.height = s,
            ("center", ParamValue::Bool(b)) => self.center = b,
            ("thicken", ParamValue::Expr(s)) => self.thicken = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut plane = resolve_plane_face(&self.plane, self.plane_feature, self.face_plane, ctx)?;
        plane.origin += plane.normal() * ctx.eval(&self.z)?;
        let off = DVec2::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?);
        let size = ctx.eval(&self.size)?;
        let h = ctx.eval(&self.height)?;
        let font = crate::fonts::load(&self.font_path).map_err(RegenError::Other)?;
        let glyphs = text_outlines(&font, &self.text, size, self.center).map_err(RegenError::Other)?;
        let mut bodies = Vec::new();
        let grow = ctx.eval(&self.thicken)?;
        // Average stroke width per glyph region: 2 * area / perimeter. For a
        // thin stroke of width w and length L this is close to w.
        let mut thinnest = f64::INFINITY;
        for loops in glyphs {
            let loops: Vec<Vec<DVec2>> = loops.into_iter().map(|c| c.into_iter().map(|p| p + off).collect()).collect();
            for (outer, holes) in nest_loops(&loops) {
                let (outer, holes) = if grow.abs() > 1e-9 {
                    (
                        offset_loop(&outer, grow),
                        holes.iter().map(|h| offset_loop(h, -grow)).filter(|h| polygon_area(h).abs() > 1e-6).collect(),
                    )
                } else {
                    (outer, holes)
                };
                let area = polygon_area(&outer).abs() - holes.iter().map(|h| polygon_area(h).abs()).sum::<f64>();
                let perim = perimeter(&outer) + holes.iter().map(|h| perimeter(h)).sum::<f64>();
                if perim > 0.0 && area > 0.0 {
                    thinnest = thinnest.min(2.0 * area / perim);
                }
                bodies.push(ctx.kernel.extrude_with_holes(&plane, &outer, &holes, h)?);
            }
        }
        if bodies.is_empty() {
            return Err(RegenError::Other("no printable glyphs".into()));
        }
        let mut out = crate::features::extrude::apply_operation(ctx, &self.operation, self.target, bodies)?;
        if thinnest.is_finite() {
            out.note = Some(if thinnest < 0.8 {
                // Each side grows by the thicken distance; round up to 0.05 mm.
                let target = grow + ((0.8 - thinnest) / 2.0 / 0.05).ceil() * 0.05;
                format!(
                    "Strokes about {thinnest:.2} mm wide. A 0.4 mm nozzle needs about 0.8 mm: try Archivo Black, a larger size, or Thicken {target:.2}."
                )
            } else {
                format!("Strokes about {thinnest:.2} mm wide: printable with a 0.4 mm nozzle.")
            });
        }
        Ok(out)
    }
    fn place_on_face(&mut self, plane: Plane, body: usize) -> bool {
        self.plane = "Face".into();
        self.face_plane = Some(plane);
        self.z = "0".into();
        self.x = "0".into();
        self.y = "0".into();
        self.operation = "join".into();
        self.target = body;
        let _ = body;
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------- QR

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QrFeature {
    pub data: String,
    pub plane: String,
    pub plane_feature: Option<usize>,
    /// Plane of a picked face when `plane` is "Face".
    #[serde(default)]
    pub face_plane: Option<Plane>,
    pub x: String,
    pub y: String,
    pub z: String,
    pub size: String,
    pub height: String,
    pub quiet_zone: bool,
}

impl Default for QrFeature {
    fn default() -> Self {
        QrFeature {
            data: "https://example.com".into(),
            plane: "XY".into(),
            plane_feature: None,
            face_plane: None,
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            size: "20".into(),
            height: "1".into(),
            quiet_zone: false,
        }
    }
}

#[typetag::serde(name = "qr")]
impl Feature for QrFeature {
    fn kind(&self) -> &'static str {
        "qr"
    }
    fn name(&self) -> String {
        format!("QR code ({} mm)", self.size)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            text_param("data", "Content (URL or text)", &self.data),
            ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature", "Face"], &self.plane),
            ParamSpec::length("x", "Corner X on plane", &self.x),
            ParamSpec::length("y", "Corner Y on plane", &self.y),
            ParamSpec::length("z", "Offset along normal", &self.z),
            ParamSpec::length("size", "Overall size", &self.size),
            ParamSpec::length("height", "Emboss height", &self.height),
            ParamSpec::boolean("quiet_zone", "Include quiet zone in size", self.quiet_zone),
        ];
        if self.plane == "Feature" {
            v.push(ParamSpec::feature_ref(
                "plane_feature",
                "Plane feature",
                PLANE_TYPES.to_vec(),
                self.plane_feature.unwrap_or(0),
            ));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("data", ParamValue::Expr(s)) => self.data = s,
            ("plane", ParamValue::Choice(p)) => self.plane = p,
            ("plane_feature", ParamValue::FeatureRef(i)) => self.plane_feature = Some(i),
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("z", ParamValue::Expr(s)) => self.z = s,
            ("size", ParamValue::Expr(s)) => self.size = s,
            ("height", ParamValue::Expr(s)) => self.height = s,
            ("quiet_zone", ParamValue::Bool(b)) => self.quiet_zone = b,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut plane = resolve_plane_face(&self.plane, self.plane_feature, self.face_plane, ctx)?;
        plane.origin += plane.normal() * ctx.eval(&self.z)?;
        let off = DVec2::new(ctx.eval(&self.x)?, ctx.eval(&self.y)?);
        let size = ctx.eval(&self.size)?;
        let h = ctx.eval(&self.height)?;
        let code = qrcode::QrCode::new(self.data.as_bytes()).map_err(|e| RegenError::Other(format!("QR: {e}")))?;
        let w = code.width();
        let colors = code.to_colors();
        let quiet = if self.quiet_zone { 4 } else { 0 };
        let cells = w + 2 * quiet;
        let m = size / cells as f64;
        let mut bodies = Vec::new();
        // Row runs of dark modules become one box each. Row 0 is the top.
        for row in 0..w {
            let mut col = 0;
            while col < w {
                if colors[row * w + col] != qrcode::Color::Dark {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < w && colors[row * w + col] == qrcode::Color::Dark {
                    col += 1;
                }
                let x0 = off.x + (start + quiet) as f64 * m;
                let x1 = off.x + (col + quiet) as f64 * m;
                let y1 = off.y + size - (row + quiet) as f64 * m;
                let y0 = y1 - m;
                let prof = vec![DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
                bodies.push(ctx.kernel.extrude(&plane, &prof, h)?);
            }
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn place_on_face(&mut self, plane: Plane, body: usize) -> bool {
        self.plane = "Face".into();
        self.face_plane = Some(plane);
        self.z = "0".into();
        self.x = "-(size) / 2".replace("(size)", &self.size);
        self.y = self.x.clone();
        let _ = body;
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------- texture

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureFeature {
    pub shape: String,
    pub plane: String,
    pub plane_feature: Option<usize>,
    /// Plane of a picked face when `plane` is "Face".
    #[serde(default)]
    pub face_plane: Option<Plane>,
    pub x: String,
    pub y: String,
    pub z: String,
    pub width: String,
    pub depth: String,
    pub cell: String,
    pub gap: String,
    pub height: String,
}

impl Default for TextureFeature {
    fn default() -> Self {
        TextureFeature {
            shape: "hex".into(),
            plane: "XY".into(),
            plane_feature: None,
            face_plane: None,
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "40".into(),
            depth: "20".into(),
            cell: "3".into(),
            gap: "0.6".into(),
            height: "0.4".into(),
        }
    }
}

#[typetag::serde(name = "texture")]
impl Feature for TextureFeature {
    fn kind(&self) -> &'static str {
        "texture"
    }
    fn name(&self) -> String {
        format!("Texture ({} {} mm)", self.shape, self.cell)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let mut v = vec![
            ParamSpec::choice("shape", "Shape", vec!["hex", "circle", "diamond", "square", "triangle"], &self.shape),
            ParamSpec::choice("plane", "Plane", vec!["XY", "XZ", "YZ", "Feature", "Face"], &self.plane),
            ParamSpec::length("x", "Area corner X", &self.x),
            ParamSpec::length("y", "Area corner Y", &self.y),
            ParamSpec::length("z", "Offset along normal", &self.z),
            ParamSpec::length("width", "Area width", &self.width),
            ParamSpec::length("depth", "Area depth", &self.depth),
            ParamSpec::length("cell", "Cell size", &self.cell),
            ParamSpec::length("gap", "Gap between cells", &self.gap),
            ParamSpec::length("height", "Relief height", &self.height),
        ];
        if self.plane == "Feature" {
            v.push(ParamSpec::feature_ref(
                "plane_feature",
                "Plane feature",
                PLANE_TYPES.to_vec(),
                self.plane_feature.unwrap_or(0),
            ));
        }
        v
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("shape", ParamValue::Choice(p)) => self.shape = p,
            ("plane", ParamValue::Choice(p)) => self.plane = p,
            ("plane_feature", ParamValue::FeatureRef(i)) => self.plane_feature = Some(i),
            ("x", ParamValue::Expr(s)) => self.x = s,
            ("y", ParamValue::Expr(s)) => self.y = s,
            ("z", ParamValue::Expr(s)) => self.z = s,
            ("width", ParamValue::Expr(s)) => self.width = s,
            ("depth", ParamValue::Expr(s)) => self.depth = s,
            ("cell", ParamValue::Expr(s)) => self.cell = s,
            ("gap", ParamValue::Expr(s)) => self.gap = s,
            ("height", ParamValue::Expr(s)) => self.height = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let mut plane = resolve_plane_face(&self.plane, self.plane_feature, self.face_plane, ctx)?;
        plane.origin += plane.normal() * ctx.eval(&self.z)?;
        let (x0, y0) = (ctx.eval(&self.x)?, ctx.eval(&self.y)?);
        let (w, d) = (ctx.eval(&self.width)?, ctx.eval(&self.depth)?);
        let cell = ctx.eval(&self.cell)?;
        let gap = ctx.eval(&self.gap)?;
        let h = ctx.eval(&self.height)?;
        if cell <= 0.0 || w <= 0.0 || d <= 0.0 {
            return Err(RegenError::Other("cell, width, and depth must be positive".into()));
        }
        let pitch = cell + gap;
        let r = cell / 2.0;
        let shape = |c: DVec2| -> Vec<DVec2> {
            let poly = |n: usize, rot: f64| -> Vec<DVec2> {
                (0..n)
                    .map(|i| {
                        let t = rot + std::f64::consts::TAU * i as f64 / n as f64;
                        c + DVec2::new(t.cos(), t.sin()) * r
                    })
                    .collect()
            };
            match self.shape.as_str() {
                "circle" => poly(16, 0.0),
                "diamond" => poly(4, 0.0),
                "square" => poly(4, std::f64::consts::FRAC_PI_4),
                "triangle" => poly(3, std::f64::consts::FRAC_PI_2),
                _ => poly(6, 0.0),
            }
        };
        let hex = self.shape == "hex";
        let row_pitch = if hex { pitch * 0.866 } else { pitch };
        let rows = ((d - cell) / row_pitch).floor().max(0.0) as usize + 1;
        let mut bodies = Vec::new();
        for j in 0..rows {
            let stagger = if hex && j % 2 == 1 { pitch / 2.0 } else { 0.0 };
            let cy = y0 + r + j as f64 * row_pitch;
            let cols = ((w - cell - stagger) / pitch).floor().max(0.0) as usize + 1;
            for i in 0..cols {
                let cx = x0 + r + stagger + i as f64 * pitch;
                if cx + r > x0 + w + 1e-9 || cy + r > y0 + d + 1e-9 {
                    continue;
                }
                bodies.push(ctx.kernel.extrude(&plane, &shape(DVec2::new(cx, cy)), h)?);
            }
        }
        if bodies.len() > 4000 {
            return Err(RegenError::Other(format!("{} cells is too many; use a larger cell", bodies.len())));
        }
        Ok(FeatureOutput { bodies, ..Default::default() })
    }
    fn place_on_face(&mut self, plane: Plane, body: usize) -> bool {
        self.plane = "Face".into();
        self.face_plane = Some(plane);
        self.z = "0".into();
        self.x = format!("-({}) / 2", self.width);
        self.y = format!("-({}) / 2", self.depth);
        let _ = body;
        true
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------- press pull

/// Extrude a picked face into a new body. The face outline is stored as
/// geometry in the face plane, so it does not follow later edits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaceExtrudeFeature {
    pub plane: Plane,
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    pub distance: String,
    pub source: usize,
}

impl Default for FaceExtrudeFeature {
    fn default() -> Self {
        FaceExtrudeFeature {
            plane: Plane::XY,
            outer: vec![DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)],
            holes: Vec::new(),
            distance: "5".into(),
            source: 0,
        }
    }
}

#[typetag::serde(name = "face_extrude")]
impl Feature for FaceExtrudeFeature {
    fn kind(&self) -> &'static str {
        "face_extrude"
    }
    fn name(&self) -> String {
        format!("Press Pull ({}) from {}", self.distance, self.source)
    }
    fn params(&self) -> Vec<ParamSpec> {
        vec![ParamSpec::length("distance", "Distance (negative goes into the body)", &self.distance)]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("distance", ParamValue::Expr(s)) => self.distance = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let d = ctx.eval(&self.distance)?;
        let body = ctx.kernel.extrude_with_holes(&self.plane, &self.outer, &self.holes, d)?;
        Ok(FeatureOutput { bodies: vec![body], ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "text", label: "Text", tab: "Solid", group: "Create", tooltip: "Embossed text as separate bodies (one per letter)", order: 41, create: || Box::new(TextFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "qr", label: "QR Code", tab: "Solid", group: "Create", tooltip: "Embossed QR code for a URL or text", order: 42, create: || Box::new(QrFeature::default()) } }
inventory::submit! { FeatureDescriptor { id: "texture", label: "Texture", tab: "Solid", group: "Create", tooltip: "Tiled relief pattern (hex, circle, diamond, square, triangle)", order: 43, create: || Box::new(TextureFeature::default()) } }
