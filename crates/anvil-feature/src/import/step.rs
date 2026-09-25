//! STEP (ISO 10303-21) reader.
//!
//! The file is read in two passes. The first turns the text into an
//! entity table, `#id -> ENTITY(values)`. The second walks the solids in
//! that table and builds Bodies.
//!
//! What becomes geometry today:
//! * Bodies: `MANIFOLD_SOLID_BREP`, `FACETED_BREP`, `BREP_WITH_VOIDS`,
//!   and the shells of a `SHELL_BASED_SURFACE_MODEL`.
//! * Faces: `ADVANCED_FACE` or `FACE_SURFACE` on a `PLANE`.
//! * Loops: `EDGE_LOOP` of `ORIENTED_EDGE`s, and `POLY_LOOP`.
//! * Edges: `LINE`, `CIRCLE`, `ELLIPSE`, B-spline curves (plain and
//!   rational), through `SURFACE_CURVE`, `SEAM_CURVE` and `TRIMMED_CURVE`.
//!   A curved edge becomes a polyline.
//! * Units: millimetre, centimetre, metre, and units defined from them,
//!   such as the inch.
//!
//! A face on any other surface is skipped and counted in the notes.

use super::Imported;
use anvil_kernel::{Face, Solid, Surface};
use anvil_math::DVec3;
use std::collections::HashMap;

/// Deepest nesting of lists the parser follows. Real files stay far
/// below this; a hostile file must not overflow the stack.
const MAX_DEPTH: usize = 64;

/// Read a STEP file.
pub fn read(path: &std::path::Path) -> Result<Imported, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_bytes(&bytes)
}

/// Read STEP text that is already in memory.
pub fn read_bytes(bytes: &[u8]) -> Result<Imported, String> {
    let text = String::from_utf8_lossy(bytes);
    let ents = parse(&text)?;
    Ok(Builder::new(&ents).build())
}

// ---------------------------------------------------------------- lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Hash(u64),
    Ident(String),
    Num(f64),
    Str(String),
    Enum(String),
    Unset,
    LParen,
    RParen,
    Comma,
    Eq,
    Semi,
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Lexer { s: s.as_bytes(), i: 0 }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_space(&mut self) -> Result<(), String> {
        loop {
            match self.peek_byte() {
                Some(c) if c.is_ascii_whitespace() => self.i += 1,
                Some(b'/') if self.s.get(self.i + 1) == Some(&b'*') => {
                    let rest = &self.s[self.i + 2..];
                    match rest.windows(2).position(|w| w == b"*/") {
                        Some(k) => self.i += 2 + k + 2,
                        None => return Err("a comment is not closed".into()),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// The next token, or `None` at the end of the text.
    fn next(&mut self) -> Result<Option<Tok>, String> {
        self.skip_space()?;
        let Some(c) = self.peek_byte() else { return Ok(None) };
        let start = self.i;
        self.i += 1;
        let tok = match c {
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b',' => Tok::Comma,
            b'=' => Tok::Eq,
            b';' => Tok::Semi,
            b'$' | b'*' => Tok::Unset,
            b'#' => {
                let digits = self.take_while(|b| b.is_ascii_digit());
                if digits.is_empty() {
                    return Err(format!("a '#' without a number at byte {start}"));
                }
                Tok::Hash(digits.parse().map_err(|_| format!("entity number too large at byte {start}"))?)
            }
            b'\'' => {
                let mut out = Vec::new();
                loop {
                    match self.peek_byte() {
                        None => return Err("a string is not closed".into()),
                        Some(b'\'') if self.s.get(self.i + 1) == Some(&b'\'') => {
                            out.push(b'\'');
                            self.i += 2;
                        }
                        Some(b'\'') => {
                            self.i += 1;
                            break;
                        }
                        Some(b) => {
                            out.push(b);
                            self.i += 1;
                        }
                    }
                }
                Tok::Str(String::from_utf8_lossy(&out).into_owned())
            }
            b'"' => {
                // Binary literal: kept as text, never used for geometry.
                let body = self.take_while(|b| b != b'"');
                if self.peek_byte() != Some(b'"') {
                    return Err("a binary value is not closed".into());
                }
                self.i += 1;
                Tok::Str(body)
            }
            b'.' => {
                let name = self.take_while(|b| b.is_ascii_alphanumeric() || b == b'_');
                if self.peek_byte() != Some(b'.') || name.is_empty() {
                    return Err(format!("a bad enumeration at byte {start}"));
                }
                self.i += 1;
                Tok::Enum(name)
            }
            b'0'..=b'9' | b'+' | b'-' => {
                self.i = start;
                let text = self.take_while(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-' | b'e' | b'E'));
                // "1." and "1.E-06" are valid STEP reals.
                let fixed = text.replace(".E", ".0E").replace(".e", ".0e");
                let fixed = if fixed.ends_with('.') { format!("{fixed}0") } else { fixed };
                Tok::Num(fixed.parse().map_err(|_| format!("a bad number '{text}' at byte {start}"))?)
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'!' => {
                self.i = start;
                let name = self.take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'!'));
                Tok::Ident(name.to_ascii_uppercase())
            }
            other => return Err(format!("an unexpected character '{}' at byte {start}", other as char)),
        };
        Ok(Some(tok))
    }

    fn take_while(&mut self, f: impl Fn(u8) -> bool) -> String {
        let start = self.i;
        while self.peek_byte().is_some_and(&f) {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }
}

// --------------------------------------------------------------- parser

/// One parameter value of an entity.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Ref(u64),
    Num(f64),
    Str(String),
    Enum(String),
    List(Vec<Value>),
    /// A typed value such as `LENGTH_MEASURE(1.E-06)`.
    Typed(String, Vec<Value>),
    Unset,
}

impl Value {
    fn as_ref_id(&self) -> Option<u64> {
        match self {
            Value::Ref(r) => Some(*r),
            _ => None,
        }
    }
    fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(x) => Some(*x),
            Value::Typed(_, v) => v.first()?.as_num(),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Enum(e) if e == "T" => Some(true),
            Value::Enum(e) if e == "F" => Some(false),
            _ => None,
        }
    }
    fn as_list(&self) -> &[Value] {
        match self {
            Value::List(v) => v,
            _ => &[],
        }
    }
    fn refs(&self) -> Vec<u64> {
        self.as_list().iter().filter_map(Value::as_ref_id).collect()
    }
    fn nums(&self) -> Vec<f64> {
        self.as_list().iter().filter_map(Value::as_num).collect()
    }
}

/// An entity instance. A simple entity has one part; a complex entity,
/// written `#n=( A(..) B(..) )`, has several.
#[derive(Clone, Debug)]
struct Entity {
    parts: Vec<(String, Vec<Value>)>,
}

impl Entity {
    fn name(&self) -> &str {
        match self.parts.as_slice() {
            [(n, _)] => n,
            _ => "complex entity",
        }
    }
    fn part(&self, name: &str) -> Option<&[Value]> {
        self.parts.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice())
    }
    fn has(&self, name: &str) -> bool {
        self.parts.iter().any(|(n, _)| n == name)
    }
}

struct Parser<'a> {
    lex: Lexer<'a>,
    look: Option<Tok>,
}

impl<'a> Parser<'a> {
    fn next(&mut self) -> Result<Option<Tok>, String> {
        match self.look.take() {
            Some(t) => Ok(Some(t)),
            None => self.lex.next(),
        }
    }
    fn peek(&mut self) -> Result<Option<&Tok>, String> {
        if self.look.is_none() {
            self.look = self.lex.next()?;
        }
        Ok(self.look.as_ref())
    }
    fn need(&mut self) -> Result<Tok, String> {
        self.next()?.ok_or_else(|| "the file ends in the middle of an entity".to_string())
    }
    fn expect(&mut self, want: Tok) -> Result<(), String> {
        let got = self.need()?;
        if got == want {
            Ok(())
        } else {
            Err(format!("expected {want:?}, found {got:?}"))
        }
    }

    /// `( value, value, ... )`, the opening parenthesis already read.
    fn list(&mut self, depth: usize) -> Result<Vec<Value>, String> {
        if depth > MAX_DEPTH {
            return Err("lists nest too deeply".into());
        }
        let mut out = Vec::new();
        if self.peek()? == Some(&Tok::RParen) {
            self.next()?;
            return Ok(out);
        }
        loop {
            out.push(self.value(depth)?);
            match self.need()? {
                Tok::Comma => {}
                Tok::RParen => return Ok(out),
                t => return Err(format!("expected ',' or ')', found {t:?}")),
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        Ok(match self.need()? {
            Tok::Hash(r) => Value::Ref(r),
            Tok::Num(x) => Value::Num(x),
            Tok::Str(s) => Value::Str(s),
            Tok::Enum(e) => Value::Enum(e),
            Tok::Unset => Value::Unset,
            Tok::LParen => Value::List(self.list(depth + 1)?),
            Tok::Ident(name) => {
                self.expect(Tok::LParen)?;
                Value::Typed(name, self.list(depth + 1)?)
            }
            t => return Err(format!("unexpected {t:?} in a parameter list")),
        })
    }

    /// One `NAME(params)` record.
    fn record(&mut self, name: String) -> Result<(String, Vec<Value>), String> {
        self.expect(Tok::LParen)?;
        Ok((name, self.list(1)?))
    }
}

/// Parse the whole file into its entity table.
fn parse(text: &str) -> Result<HashMap<u64, Entity>, String> {
    let mut p = Parser { lex: Lexer::new(text), look: None };
    match p.next()? {
        Some(Tok::Ident(s)) if s == "ISO-10303-21" => {}
        _ => return Err("not a STEP file: it does not start with ISO-10303-21".into()),
    }
    p.expect(Tok::Semi)?;
    let mut ents = HashMap::new();
    let mut saw_data = false;
    loop {
        match p.need()? {
            Tok::Ident(s) if s == "END-ISO-10303-21" => break,
            Tok::Ident(s) if s == "HEADER" => {
                p.expect(Tok::Semi)?;
                loop {
                    match p.need()? {
                        Tok::Ident(s) if s == "ENDSEC" => break,
                        Tok::Ident(name) => {
                            p.record(name)?;
                        }
                        t => return Err(format!("unexpected {t:?} in the header")),
                    }
                    p.expect(Tok::Semi)?;
                }
                p.expect(Tok::Semi)?;
            }
            Tok::Ident(s) if s == "DATA" => {
                saw_data = true;
                // DATA may carry a name and schema list in newer files.
                if p.peek()? == Some(&Tok::LParen) {
                    p.next()?;
                    p.list(1)?;
                }
                p.expect(Tok::Semi)?;
                loop {
                    let id = match p.need()? {
                        Tok::Ident(s) if s == "ENDSEC" => break,
                        Tok::Hash(id) => id,
                        t => return Err(format!("expected an entity, found {t:?}")),
                    };
                    p.expect(Tok::Eq)?;
                    let parts = match p.need()? {
                        Tok::Ident(name) => vec![p.record(name)?],
                        Tok::LParen => {
                            let mut parts = Vec::new();
                            loop {
                                match p.need()? {
                                    Tok::RParen => break,
                                    Tok::Ident(name) => parts.push(p.record(name)?),
                                    t => return Err(format!("unexpected {t:?} in complex entity #{id}")),
                                }
                            }
                            if parts.is_empty() {
                                return Err(format!("complex entity #{id} is empty"));
                            }
                            parts
                        }
                        t => return Err(format!("unexpected {t:?} after #{id}=")),
                    };
                    p.expect(Tok::Semi)?;
                    ents.insert(id, Entity { parts });
                }
                p.expect(Tok::Semi)?;
            }
            t => return Err(format!("unexpected {t:?} between sections")),
        }
    }
    if !saw_data {
        return Err("the STEP file has no DATA section".into());
    }
    Ok(ents)
}

// -------------------------------------------------------------- geometry

/// A local frame: origin, normal (z), and x axis.
#[derive(Clone, Copy, Debug)]
struct Frame {
    o: DVec3,
    z: DVec3,
    x: DVec3,
}

impl Frame {
    fn y(&self) -> DVec3 {
        self.z.cross(self.x)
    }
}

/// What a curve looks like as points, and whether it closes on itself.
struct Samples {
    pts: Vec<DVec3>,
    closed: bool,
}

struct Builder<'a> {
    ents: &'a HashMap<u64, Entity>,
    /// Millimetres per file unit.
    scale: f64,
    /// Each edge sampled once, from its first vertex to its second, so
    /// that two faces sharing it get the same points.
    edges: HashMap<u64, Option<Vec<DVec3>>>,
    skipped: std::collections::BTreeMap<String, usize>,
    notes: Vec<String>,
    /// Curves drawn as straight lines because their type is unknown.
    straight: std::collections::BTreeMap<String, usize>,
}

impl<'a> Builder<'a> {
    fn new(ents: &'a HashMap<u64, Entity>) -> Self {
        let mut b = Builder {
            ents,
            scale: 1.0,
            edges: HashMap::new(),
            skipped: Default::default(),
            notes: Vec::new(),
            straight: Default::default(),
        };
        b.scale = b.length_scale();
        b
    }

    fn get(&self, id: u64) -> Option<&'a Entity> {
        self.ents.get(&id)
    }

    /// The parameters of `#id` if it is a simple entity called `name`.
    fn params(&self, id: u64, name: &str) -> Option<&'a [Value]> {
        let e = self.get(id)?;
        (e.name() == name).then(|| e.parts[0].1.as_slice())
    }

    // ------------------------------------------------------------ units

    /// Millimetres per length unit of the file.
    fn length_scale(&mut self) -> f64 {
        // The unit named by the geometric context wins.
        let mut ctx_units: Vec<u64> = Vec::new();
        for e in self.ents.values() {
            if let Some(v) = e.part("GLOBAL_UNIT_ASSIGNED_CONTEXT") {
                ctx_units.extend(v.first().map(Value::refs).unwrap_or_default());
            }
        }
        let is_length = |id: &u64| self.get(*id).is_some_and(|e| e.has("LENGTH_UNIT"));
        let mut ids: Vec<u64> = ctx_units.into_iter().filter(is_length).collect();
        if ids.is_empty() {
            let mut all: Vec<u64> = self.ents.iter().filter(|(_, e)| e.has("LENGTH_UNIT")).map(|(k, _)| *k).collect();
            all.sort();
            ids = all;
        }
        for id in ids {
            if let Some(s) = self.unit_mm(id, 0) {
                return s;
            }
        }
        self.notes.push("no length unit found; millimetres assumed".into());
        1.0
    }

    /// Millimetres per unit for the unit entity `#id`.
    fn unit_mm(&self, id: u64, depth: usize) -> Option<f64> {
        if depth > 8 {
            return None;
        }
        let e = self.get(id)?;
        if let Some(v) = e.part("SI_UNIT") {
            let prefix = match v.first() {
                Some(Value::Enum(p)) => match p.as_str() {
                    "MILLI" => 1e-3,
                    "CENTI" => 1e-2,
                    "DECI" => 1e-1,
                    "KILO" => 1e3,
                    "MICRO" => 1e-6,
                    "NANO" => 1e-9,
                    _ => return None,
                },
                _ => 1.0,
            };
            return match v.get(1) {
                Some(Value::Enum(n)) if n == "METRE" => Some(prefix * 1000.0),
                _ => None,
            };
        }
        if let Some(v) = e.part("CONVERSION_BASED_UNIT") {
            let m = self.get(v.get(1)?.as_ref_id()?)?;
            let (_, mv) = m.parts.iter().find(|(n, _)| n.ends_with("MEASURE_WITH_UNIT"))?;
            let factor = mv.first()?.as_num()?;
            let base = self.unit_mm(mv.get(1)?.as_ref_id()?, depth + 1)?;
            return Some(factor * base);
        }
        None
    }

    // ------------------------------------------------------- primitives

    fn point(&self, id: u64) -> Option<DVec3> {
        let v = self.params(id, "CARTESIAN_POINT")?;
        let c = v.get(1)?.nums();
        let p = match c.as_slice() {
            [x, y, z] => DVec3::new(*x, *y, *z),
            [x, y] => DVec3::new(*x, *y, 0.0),
            _ => return None,
        };
        p.is_finite().then_some(p * self.scale)
    }

    fn direction(&self, id: u64) -> Option<DVec3> {
        let v = self.params(id, "DIRECTION")?;
        let c = v.get(1)?.nums();
        let d = match c.as_slice() {
            [x, y, z] => DVec3::new(*x, *y, *z),
            [x, y] => DVec3::new(*x, *y, 0.0),
            _ => return None,
        };
        let d = d.normalize_or_zero();
        (d.length_squared() > 0.5).then_some(d)
    }

    fn frame(&self, id: u64) -> Option<Frame> {
        let v = self.params(id, "AXIS2_PLACEMENT_3D")?;
        let o = self.point(v.get(1)?.as_ref_id()?)?;
        let z = v.get(2).and_then(Value::as_ref_id).and_then(|r| self.direction(r)).unwrap_or(DVec3::Z);
        let x0 = v.get(3).and_then(Value::as_ref_id).and_then(|r| self.direction(r)).unwrap_or(DVec3::X);
        let mut x = (x0 - z * x0.dot(z)).normalize_or_zero();
        if x.length_squared() < 0.5 {
            x = z.any_orthonormal_vector();
        }
        Some(Frame { o, z, x })
    }

    fn vertex(&self, id: u64) -> Option<DVec3> {
        let v = self.params(id, "VERTEX_POINT")?;
        self.point(v.get(1)?.as_ref_id()?)
    }

    // ----------------------------------------------------------- curves

    /// Points along a whole curve, in its own direction.
    fn curve_samples(&mut self, id: u64, depth: usize) -> Option<Samples> {
        if depth > 8 {
            return None;
        }
        let e = self.get(id)?;
        let name = e.name().to_string();
        let v = e.parts.first().map(|(_, v)| v.as_slice()).unwrap_or(&[]);
        match name.as_str() {
            "LINE" => None, // straight: the edge's two vertices are enough
            "CIRCLE" => {
                let f = self.frame(v.get(1)?.as_ref_id()?)?;
                let r = v.get(2)?.as_num()? * self.scale;
                let n = segments_for(r);
                let pts = (0..n)
                    .map(|k| {
                        let a = std::f64::consts::TAU * k as f64 / n as f64;
                        f.o + (f.x * a.cos() + f.y() * a.sin()) * r
                    })
                    .collect();
                Some(Samples { pts, closed: true })
            }
            "ELLIPSE" => {
                let f = self.frame(v.get(1)?.as_ref_id()?)?;
                let (a, b) = (v.get(2)?.as_num()? * self.scale, v.get(3)?.as_num()? * self.scale);
                let n = segments_for(a.max(b));
                let pts = (0..n)
                    .map(|k| {
                        let t = std::f64::consts::TAU * k as f64 / n as f64;
                        f.o + f.x * (a * t.cos()) + f.y() * (b * t.sin())
                    })
                    .collect();
                Some(Samples { pts, closed: true })
            }
            "SURFACE_CURVE" | "SEAM_CURVE" | "TRIMMED_CURVE" => {
                let inner = v.get(1)?.as_ref_id()?;
                self.curve_samples(inner, depth + 1)
            }
            "B_SPLINE_CURVE_WITH_KNOTS" => {
                let degree = v.get(1)?.as_num()? as usize;
                let ctrl: Vec<DVec3> = v.get(2)?.refs().into_iter().filter_map(|r| self.point(r)).collect();
                let mults = v.get(6)?.nums();
                let knots = v.get(7)?.nums();
                bspline(degree, &ctrl, None, &mults, &knots)
            }
            _ if e.has("B_SPLINE_CURVE") && e.has("B_SPLINE_CURVE_WITH_KNOTS") => {
                let bs = e.part("B_SPLINE_CURVE")?;
                let kn = e.part("B_SPLINE_CURVE_WITH_KNOTS")?;
                let degree = bs.first()?.as_num()? as usize;
                let ctrl: Vec<DVec3> = bs.get(1)?.refs().into_iter().filter_map(|r| self.point(r)).collect();
                let weights = e.part("RATIONAL_B_SPLINE_CURVE").and_then(|w| w.first()).map(Value::nums);
                bspline(degree, &ctrl, weights.as_deref(), &kn.first()?.nums(), &kn.get(1)?.nums())
            }
            other => {
                *self.straight.entry(other.to_string()).or_insert(0) += 1;
                None
            }
        }
    }

    /// Points of an `EDGE_CURVE` from its first vertex to its second.
    fn edge(&mut self, id: u64) -> Option<Vec<DVec3>> {
        if let Some(e) = self.edges.get(&id) {
            return e.clone();
        }
        let got = self.edge_uncached(id);
        self.edges.insert(id, got.clone());
        got
    }

    fn edge_uncached(&mut self, id: u64) -> Option<Vec<DVec3>> {
        let v = self.params(id, "EDGE_CURVE")?;
        let a = self.vertex(v.get(1)?.as_ref_id()?)?;
        let b = self.vertex(v.get(2)?.as_ref_id()?)?;
        let sense = v.get(4).and_then(Value::as_bool).unwrap_or(true);
        let curve = v.get(3)?.as_ref_id()?;
        match self.curve_samples(curve, 0) {
            Some(s) => Some(sub_polyline(&s, a, b, sense)),
            None => Some(vec![a, b]),
        }
    }

    /// A closed loop as points, first point not repeated at the end.
    fn loop_points(&mut self, id: u64) -> Option<Vec<DVec3>> {
        let e = self.get(id)?;
        let v = e.parts.first().map(|(_, v)| v.as_slice())?;
        let mut pts: Vec<DVec3> = Vec::new();
        match e.name() {
            "POLY_LOOP" => {
                pts = v.get(1)?.refs().into_iter().filter_map(|r| self.point(r)).collect();
            }
            "EDGE_LOOP" => {
                for oe in v.get(1)?.refs() {
                    let ov = self.params(oe, "ORIENTED_EDGE")?;
                    let forward = ov.get(4).and_then(Value::as_bool).unwrap_or(true);
                    let mut seg = self.edge(ov.get(3)?.as_ref_id()?)?;
                    if !forward {
                        seg.reverse();
                    }
                    // Each edge starts where the last one ended.
                    let skip = usize::from(!pts.is_empty());
                    pts.extend(seg.into_iter().skip(skip));
                }
            }
            _ => return None,
        }
        clean_loop(&mut pts);
        (pts.len() >= 3).then_some(pts)
    }

    // ------------------------------------------------------------ faces

    /// Add one face to `out`. Returns the surface type name when the face
    /// was skipped.
    fn face(&mut self, id: u64, flip: bool, out: &mut FaceSink) -> Result<(), String> {
        let e = self.get(id).ok_or_else(|| "a missing face".to_string())?;
        let (bounds, surface, same) = match e.name() {
            "ADVANCED_FACE" | "FACE_SURFACE" => {
                let v = &e.parts[0].1;
                (
                    v.get(1).map(Value::refs).unwrap_or_default(),
                    v.get(2).and_then(Value::as_ref_id),
                    v.get(3).and_then(Value::as_bool).unwrap_or(true),
                )
            }
            other => return Err(other.to_string()),
        };
        let surface = surface.ok_or_else(|| "face without a surface".to_string())?;
        let s = self.get(surface).ok_or_else(|| "a missing surface".to_string())?;
        if s.name() != "PLANE" {
            return Err(s.name().to_string());
        }
        let frame =
            self.frame(s.parts[0].1.get(1).and_then(Value::as_ref_id).ok_or("PLANE")?).ok_or("PLANE".to_string())?;
        let normal = if same != flip { frame.z } else { -frame.z };
        let mut outer: Option<Vec<DVec3>> = None;
        let mut loops: Vec<Vec<DVec3>> = Vec::new();
        for b in bounds {
            let Some(be) = self.get(b) else { continue };
            let (is_outer, v) = match be.name() {
                "FACE_OUTER_BOUND" => (true, &be.parts[0].1),
                "FACE_BOUND" => (false, &be.parts[0].1),
                _ => continue,
            };
            let Some(lp) = v.get(1).and_then(Value::as_ref_id).and_then(|l| self.loop_points(l)) else { continue };
            if is_outer && outer.is_none() {
                outer = Some(lp);
            } else {
                loops.push(lp);
            }
        }
        // Without a marked outer bound, the largest loop is the outer one.
        let outer = match outer {
            Some(o) => o,
            None => {
                let k = (0..loops.len()).max_by(|&a, &b| {
                    area_along(&loops[a], normal).abs().total_cmp(&area_along(&loops[b], normal).abs())
                });
                match k {
                    Some(k) => loops.remove(k),
                    None => return Err("PLANE with no usable loop".into()),
                }
            }
        };
        out.add(outer, loops, normal);
        Ok(())
    }

    /// Faces of a shell. `flip` reverses them, for an oriented shell.
    fn shell(&mut self, id: u64, flip: bool, out: &mut FaceSink) {
        let Some(e) = self.get(id) else { return };
        let v = &e.parts[0].1;
        let (faces, flip) = match e.name() {
            "CLOSED_SHELL" | "OPEN_SHELL" => (v.get(1).map(Value::refs).unwrap_or_default(), flip),
            "ORIENTED_CLOSED_SHELL" | "ORIENTED_OPEN_SHELL" => {
                let inner = v.get(2).and_then(Value::as_ref_id);
                let o = v.get(3).and_then(Value::as_bool).unwrap_or(true);
                if let Some(i) = inner {
                    self.shell(i, flip ^ !o, out);
                }
                return;
            }
            _ => return,
        };
        for f in faces {
            if let Err(kind) = self.face(f, flip, out) {
                *self.skipped.entry(kind).or_insert(0) += 1;
                out.open = true;
            }
        }
    }

    fn build(mut self) -> Imported {
        let mut ids: Vec<u64> = self.ents.keys().copied().collect();
        ids.sort();
        let mut solids = Vec::new();
        let mut open_bodies = 0;
        for id in ids {
            let e = &self.ents[&id];
            let v = e.parts.first().map(|(_, v)| v.clone()).unwrap_or_default();
            let shells: Vec<(u64, bool)> = match e.name() {
                "MANIFOLD_SOLID_BREP" | "FACETED_BREP" => {
                    v.get(1).and_then(Value::as_ref_id).map(|s| (s, false)).into_iter().collect()
                }
                "BREP_WITH_VOIDS" => {
                    let mut s: Vec<(u64, bool)> =
                        v.get(1).and_then(Value::as_ref_id).map(|s| (s, false)).into_iter().collect();
                    s.extend(v.get(2).map(Value::refs).unwrap_or_default().into_iter().map(|r| (r, false)));
                    s
                }
                "SHELL_BASED_SURFACE_MODEL" => {
                    v.get(1).map(Value::refs).unwrap_or_default().into_iter().map(|r| (r, false)).collect()
                }
                _ => continue,
            };
            let mut sink = FaceSink::new(self.scale);
            for (s, flip) in shells {
                self.shell(s, flip, &mut sink);
            }
            if sink.open && sink.faces > 0 {
                open_bodies += 1;
            }
            if let Some(solid) = sink.finish() {
                solids.push(solid);
            }
        }
        let mut notes = std::mem::take(&mut self.notes);
        for (kind, n) in &self.skipped {
            let s = if *n == 1 { "face" } else { "faces" };
            let w = if *n == 1 { "was" } else { "were" };
            notes.push(format!("{n} {s} on a {kind} {w} skipped"));
        }
        for (kind, n) in &self.straight {
            notes.push(format!("{n} edges on a {kind} were drawn as straight lines"));
        }
        if open_bodies > 0 {
            notes.push(format!("{open_bodies} bodies are open where faces were skipped"));
        }
        if self.ents.values().any(|e| e.name() == "NEXT_ASSEMBLY_USAGE_OCCURRENCE") {
            notes.push("this is an assembly; part placements are not applied".into());
        }
        Imported { solids, meshes: Vec::new(), notes }
    }
}

/// Collects planar faces into one Solid, welding equal points.
struct FaceSink {
    solid: Solid,
    index: HashMap<[i64; 3], anvil_kernel::VertexId>,
    /// Weld distance in millimetres.
    q: f64,
    faces: usize,
    /// True when a face of this body was skipped.
    open: bool,
}

impl FaceSink {
    fn new(scale: f64) -> Self {
        FaceSink { solid: Solid::new(), index: HashMap::new(), q: 1e-6 * scale.max(1.0), faces: 0, open: false }
    }

    fn vid(&mut self, p: DVec3) -> anvil_kernel::VertexId {
        let key = [(p.x / self.q).round() as i64, (p.y / self.q).round() as i64, (p.z / self.q).round() as i64];
        if let Some(v) = self.index.get(&key) {
            return *v;
        }
        let v = self.solid.add_vertex(p);
        self.index.insert(key, v);
        v
    }

    /// Add a planar face. The outer loop is turned counter-clockwise
    /// about `normal`, the holes clockwise.
    fn add(&mut self, mut outer: Vec<DVec3>, mut holes: Vec<Vec<DVec3>>, normal: DVec3) {
        if area_along(&outer, normal) < 0.0 {
            outer.reverse();
        }
        for h in &mut holes {
            if area_along(h, normal) > 0.0 {
                h.reverse();
            }
        }
        let mut ring = |pts: &[DVec3]| -> Vec<anvil_kernel::VertexId> {
            let mut ids: Vec<anvil_kernel::VertexId> = Vec::new();
            for &p in pts {
                let v = self.vid(p);
                if ids.last() != Some(&v) {
                    ids.push(v);
                }
            }
            while ids.len() > 1 && ids.first() == ids.last() {
                ids.pop();
            }
            ids
        };
        let outer = ring(&outer);
        if outer.len() < 3 {
            return;
        }
        let inner: Vec<Vec<anvil_kernel::VertexId>> = holes.iter().map(|h| ring(h)).filter(|h| h.len() >= 3).collect();
        self.solid.faces.insert(Face { outer, inner, surface: Surface::Plane });
        self.faces += 1;
    }

    fn finish(mut self) -> Option<Solid> {
        if self.faces == 0 {
            return None;
        }
        self.solid.rebuild_edges();
        Some(self.solid)
    }
}

/// Twice the signed area of a loop, measured about `normal`.
fn area_along(pts: &[DVec3], normal: DVec3) -> f64 {
    let n = pts.len();
    let mut s = DVec3::ZERO;
    for i in 0..n {
        s += pts[i].cross(pts[(i + 1) % n]);
    }
    s.dot(normal)
}

/// Drop repeated points and a closing point equal to the first.
fn clean_loop(pts: &mut Vec<DVec3>) {
    pts.dedup_by(|a, b| (*a - *b).length() < 1e-9);
    while pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9 {
        pts.pop();
    }
}

/// Segments for a full turn of radius `r` (mm): the chord stays within
/// 0.01 mm of the arc, with at least 24 and at most 256 segments.
fn segments_for(r: f64) -> usize {
    let tol = 0.01;
    if !r.is_finite() || r <= tol {
        return 24;
    }
    let step = 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos();
    ((std::f64::consts::TAU / step).ceil() as usize).clamp(24, 256)
}

/// The part of a sampled curve from `a` to `b`, walking forward in the
/// curve's direction when `forward`, else backward. The ends are exactly
/// `a` and `b`.
fn sub_polyline(s: &Samples, a: DVec3, b: DVec3, forward: bool) -> Vec<DVec3> {
    let n = s.pts.len();
    if n < 2 {
        return vec![a, b];
    }
    let near = |p: DVec3| {
        (0..n).min_by(|&i, &j| (s.pts[i] - p).length_squared().total_cmp(&(s.pts[j] - p).length_squared())).unwrap_or(0)
    };
    let (ia, ib) = (near(a), near(b));
    let full = (a - b).length() < 1e-9;
    if ia == ib && !full {
        // A short arc between two samples: a straight piece is enough.
        return vec![a, b];
    }
    let mut out = vec![a];
    let step = |i: usize| -> Option<usize> {
        if forward {
            if i + 1 < n {
                Some(i + 1)
            } else if s.closed {
                Some(0)
            } else {
                None
            }
        } else if i > 0 {
            Some(i - 1)
        } else if s.closed {
            Some(n - 1)
        } else {
            None
        }
    };
    // Walk from the sample after `a` up to the sample before `b`. The
    // nearest sample to an end may lie on either side of it, so a sample
    // that is not strictly between the ends is left out by distance.
    let mut i = ia;
    let mut guard = 0;
    while let Some(k) = step(i) {
        guard += 1;
        if guard > n || (k == ib && !(full && guard == 1)) {
            break;
        }
        if full && k == ia {
            break;
        }
        out.push(s.pts[k]);
        i = k;
    }
    out.push(b);
    // A sample that sits on an end point adds nothing.
    out.dedup_by(|p, q| (*p - *q).length() < 1e-9);
    if out.len() == 1 {
        out.push(b);
    }
    out
}

/// Points along a B-spline (rational when `weights` is given), from its
/// knot vector in STEP form: distinct knots with their multiplicities.
fn bspline(degree: usize, ctrl: &[DVec3], weights: Option<&[f64]>, mults: &[f64], knots: &[f64]) -> Option<Samples> {
    if degree == 0 || ctrl.len() <= degree || mults.len() != knots.len() || degree > 16 {
        return None;
    }
    let mut kv: Vec<f64> = Vec::new();
    for (m, k) in mults.iter().zip(knots) {
        let m = *m as usize;
        if m > degree + 1 || kv.len() + m > ctrl.len() + degree + 1 {
            return None;
        }
        kv.extend(std::iter::repeat_n(*k, m));
    }
    if kv.len() != ctrl.len() + degree + 1 {
        return None;
    }
    let w: Vec<f64> = match weights {
        Some(w) if w.len() == ctrl.len() => w.to_vec(),
        _ => vec![1.0; ctrl.len()],
    };
    let (t0, t1) = (kv[degree], kv[ctrl.len()]);
    if t1.partial_cmp(&t0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let spans = ctrl.len() - degree;
    let n = (spans * 16).clamp(16, 512);
    let mut pts = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let t = t0 + (t1 - t0) * k as f64 / n as f64;
        pts.push(de_boor(degree, &kv, ctrl, &w, t)?);
    }
    let closed = (pts[0] - pts[n]).length() < 1e-9;
    if closed {
        pts.pop();
    }
    Some(Samples { pts, closed })
}

fn de_boor(p: usize, kv: &[f64], ctrl: &[DVec3], w: &[f64], t: f64) -> Option<DVec3> {
    let n = ctrl.len();
    // The span k with kv[k] <= t < kv[k + 1], clamped to the valid range.
    let mut k = p;
    while k + 1 < n && kv[k + 1] <= t {
        k += 1;
    }
    let mut d: Vec<(DVec3, f64)> = (0..=p).map(|j| (ctrl[j + k - p] * w[j + k - p], w[j + k - p])).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = j + k - p;
            let den = kv[i + p + 1 - r] - kv[i];
            let a = if den.abs() < 1e-300 { 0.0 } else { (t - kv[i]) / den };
            d[j] = (d[j - 1].0 * (1.0 - a) + d[j].0 * a, d[j - 1].1 * (1.0 - a) + d[j].1 * a);
        }
    }
    let (pw, ww) = d[p];
    (ww.abs() > 1e-300).then(|| pw / ww).filter(|q| q.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(data: &str) -> String {
        format!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('t'),'2;1');\nENDSEC;\nDATA;\n{data}\nENDSEC;\nEND-ISO-10303-21;\n"
        )
    }

    #[test]
    fn strings_may_hold_commas_quotes_and_line_breaks() {
        let t = wrap("#1=PRODUCT('a, ''b''\nc','x',(),$);");
        let e = parse(&t).unwrap();
        assert_eq!(e[&1].parts[0].1[0], Value::Str("a, 'b'\nc".into()));
    }

    #[test]
    fn complex_entities_and_real_forms_parse() {
        let t = wrap(
            "#9=( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );\n#2=CARTESIAN_POINT('',(1.,-2.5E1,3.E-01));",
        );
        let e = parse(&t).unwrap();
        assert!(e[&9].has("SI_UNIT"));
        assert_eq!(e[&2].parts[0].1[1].nums(), vec![1.0, -25.0, 0.3]);
    }

    #[test]
    fn a_metre_file_is_scaled_to_millimetres() {
        let t = wrap(
            "#1=( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.) );\n#2=( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNIT_ASSIGNED_CONTEXT((#1)) REPRESENTATION_CONTEXT('','') );",
        );
        let e = parse(&t).unwrap();
        assert_eq!(Builder::new(&e).scale, 1000.0);
    }

    #[test]
    fn an_inch_file_is_scaled_to_millimetres() {
        let t = wrap(
            "#1=( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );\n#2=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4),#1);\n#3=( CONVERSION_BASED_UNIT('INCH',#2) LENGTH_UNIT() NAMED_UNIT(#4) );\n#4=DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);\n#5=( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNIT_ASSIGNED_CONTEXT((#3)) REPRESENTATION_CONTEXT('','') );",
        );
        let e = parse(&t).unwrap();
        assert!((Builder::new(&e).scale - 25.4).abs() < 1e-12);
    }

    #[test]
    fn a_full_circle_edge_samples_all_the_way_round() {
        let s = Samples {
            pts: (0..8)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 8.0;
                    DVec3::new(a.cos(), a.sin(), 0.0)
                })
                .collect(),
            closed: true,
        };
        let p = sub_polyline(&s, DVec3::X, DVec3::X, true);
        assert_eq!(p.len(), 9, "{p:?}");
        let q = sub_polyline(&s, DVec3::X, DVec3::Y, true);
        assert_eq!(q.len(), 3, "a quarter: start, one sample, end: {q:?}");
        let r = sub_polyline(&s, DVec3::X, DVec3::Y, false);
        assert_eq!(r.len(), 7, "three quarters the other way: {r:?}");
    }

    #[test]
    fn a_clamped_bspline_starts_and_ends_at_its_end_points() {
        let ctrl = [DVec3::ZERO, DVec3::new(1.0, 2.0, 0.0), DVec3::new(3.0, 2.0, 0.0), DVec3::new(4.0, 0.0, 0.0)];
        let s = bspline(3, &ctrl, None, &[4.0, 4.0], &[0.0, 1.0]).unwrap();
        assert!((s.pts[0] - ctrl[0]).length() < 1e-12);
        assert!((s.pts[s.pts.len() - 1] - ctrl[3]).length() < 1e-12);
    }

    #[test]
    fn a_truncated_file_is_an_error() {
        let t = wrap("#1=CARTESIAN_POINT('',(0.,0.,0.));");
        let cut = &t[..t.find("0.,0.").unwrap()];
        assert!(parse(cut).is_err());
        assert!(parse("ISO-10303-21;\nHEADER;\nENDSEC;\n").is_err(), "no DATA and no end");
    }
}
