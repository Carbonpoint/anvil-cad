//! 3MF reader.
//!
//! A 3MF file is a zip package. Its model part (normally
//! `3D/3dmodel.model`) is XML: `<object>`s that hold a `<mesh>` of
//! `<vertices>` and `<triangles>`, or `<components>` that place other
//! objects, and a `<build>` list of the objects to make. Every placed mesh
//! becomes one `TriMesh`, moved by its build and component transforms.

use super::zip::Zip;
use super::{tri_mesh, Imported};
use anvil_math::DVec3;
use std::collections::HashMap;

/// Read a 3MF file.
pub fn read(path: &std::path::Path) -> Result<Imported, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_bytes(&bytes)
}

/// Read a 3MF package that is already in memory.
pub fn read_bytes(bytes: &[u8]) -> Result<Imported, String> {
    let zip = Zip::open(bytes)?;
    let part = model_part(&zip);
    let xml = zip.read(&part)?;
    let xml = String::from_utf8_lossy(&xml);
    parse_model(&xml)
}

/// The model part named by the package relationships, else the usual name.
fn model_part(zip: &Zip) -> String {
    if let Ok(rels) = zip.read("_rels/.rels") {
        let rels = String::from_utf8_lossy(&rels);
        for tag in Tags::new(&rels) {
            if tag.name == "Relationship" && tag.attr("Type").is_some_and(|t| t.ends_with("/3dmodel")) {
                if let Some(t) = tag.attr("Target") {
                    return t.to_string();
                }
            }
        }
    }
    let usual = "3D/3dmodel.model";
    if zip.names().any(|n| n.eq_ignore_ascii_case(usual)) {
        return usual.into();
    }
    zip.names().find(|n| n.to_ascii_lowercase().ends_with(".model")).unwrap_or(usual).to_string()
}

/// An affine transform in 3MF order: `m00 m01 m02 m10 m11 m12 m20 m21 m22
/// m30 m31 m32`, applied to a row vector.
type Xform = [f64; 12];

const IDENTITY: Xform = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

fn apply(m: &Xform, p: DVec3) -> DVec3 {
    DVec3::new(
        p.x * m[0] + p.y * m[3] + p.z * m[6] + m[9],
        p.x * m[1] + p.y * m[4] + p.z * m[7] + m[10],
        p.x * m[2] + p.y * m[5] + p.z * m[8] + m[11],
    )
}

/// `inner` then `outer`.
fn compose(inner: &Xform, outer: &Xform) -> Xform {
    let r = |i: usize, j: usize, m: &Xform| m[i * 3 + j];
    let mut out = [0.0; 12];
    for i in 0..4 {
        for j in 0..3 {
            let mut s = if i == 3 { r(3, j, outer) } else { 0.0 };
            for k in 0..3 {
                s += r(i, k, inner) * r(k, j, outer);
            }
            out[i * 3 + j] = s;
        }
    }
    out
}

fn parse_xform(s: Option<&str>) -> Result<Xform, String> {
    let Some(s) = s else { return Ok(IDENTITY) };
    let v: Vec<f64> = s
        .split_whitespace()
        .map(|x| x.parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("bad transform '{s}'"))?;
    <[f64; 12]>::try_from(v).map_err(|_| format!("a transform needs 12 numbers: '{s}'"))
}

#[derive(Default)]
struct Object {
    vertices: Vec<DVec3>,
    triangles: Vec<[u32; 3]>,
    has_mesh: bool,
    components: Vec<(String, Xform)>,
}

fn parse_model(xml: &str) -> Result<Imported, String> {
    let mut scale = 1.0;
    let mut saw_model = false;
    let mut objects: HashMap<String, Object> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut build: Vec<(String, Xform)> = Vec::new();
    let mut cur: Option<String> = None;
    let num = |t: &Tag, k: &str| -> Result<f64, String> {
        t.attr(k)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|x| x.is_finite())
            .ok_or_else(|| format!("<{}> has a bad {k}", t.name))
    };
    let idx = |t: &Tag, k: &str| -> Result<u32, String> {
        t.attr(k).and_then(|v| v.trim().parse::<u32>().ok()).ok_or_else(|| format!("<triangle> has a bad {k}"))
    };
    for tag in Tags::new(xml) {
        match (tag.name, tag.closing) {
            ("model", false) => {
                saw_model = true;
                scale = match tag.attr("unit").unwrap_or("millimeter") {
                    "millimeter" => 1.0,
                    "micron" => 0.001,
                    "centimeter" => 10.0,
                    "meter" => 1000.0,
                    "inch" => 25.4,
                    "foot" => 304.8,
                    u => return Err(format!("the model unit '{u}' is not a 3MF unit")),
                };
            }
            ("object", false) => {
                let id = tag.attr("id").ok_or("<object> without an id")?.to_string();
                order.push(id.clone());
                objects.insert(id.clone(), Object::default());
                cur = (!tag.empty).then_some(id);
            }
            ("object", true) => cur = None,
            ("mesh", false) => {
                if let Some(o) = cur.as_ref().and_then(|c| objects.get_mut(c)) {
                    o.has_mesh = true;
                }
            }
            ("vertex", false) => {
                let p = DVec3::new(num(&tag, "x")?, num(&tag, "y")?, num(&tag, "z")?) * scale;
                if let Some(o) = cur.as_ref().and_then(|c| objects.get_mut(c)) {
                    o.vertices.push(p);
                }
            }
            ("triangle", false) => {
                let t = [idx(&tag, "v1")?, idx(&tag, "v2")?, idx(&tag, "v3")?];
                if let Some(o) = cur.as_ref().and_then(|c| objects.get_mut(c)) {
                    o.triangles.push(t);
                }
            }
            ("component", false) => {
                let id = tag.attr("objectid").ok_or("<component> without an objectid")?.to_string();
                let x = parse_xform(tag.attr("transform"))?;
                if let Some(o) = cur.as_ref().and_then(|c| objects.get_mut(c)) {
                    o.components.push((id, x));
                }
            }
            ("item", false) => {
                let id = tag.attr("objectid").ok_or("<item> without an objectid")?.to_string();
                build.push((id, parse_xform(tag.attr("transform"))?));
            }
            _ => {}
        }
    }
    if !saw_model {
        return Err("the package holds no 3MF <model>".into());
    }
    // Transforms are in model units; the vertices are already scaled.
    let scale_x = |mut m: Xform| {
        for t in &mut m[9..12] {
            *t *= scale;
        }
        m
    };
    if build.is_empty() {
        build = order.iter().filter(|id| objects[*id].has_mesh).map(|id| (id.clone(), IDENTITY)).collect();
    }
    let mut out = Imported::default();
    let mut placed_components = 0usize;
    let mut bad_triangles = 0usize;
    let mut stack: Vec<(String, Xform, usize)> = build.into_iter().rev().map(|(id, m)| (id, scale_x(m), 0)).collect();
    while let Some((id, m, depth)) = stack.pop() {
        let Some(o) = objects.get(&id) else {
            out.notes.push(format!("object {id} is named but not defined"));
            continue;
        };
        if depth > 16 {
            return Err("components nest too deeply".into());
        }
        if o.has_mesh && !o.vertices.is_empty() {
            let n = o.vertices.len() as u32;
            let positions: Vec<DVec3> = o.vertices.iter().map(|p| apply(&m, *p)).collect();
            let mut indices = Vec::with_capacity(o.triangles.len() * 3);
            for t in &o.triangles {
                if t.iter().all(|&i| i < n) && t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
                    indices.extend_from_slice(t);
                } else {
                    bad_triangles += 1;
                }
            }
            out.meshes.push(tri_mesh(positions, indices));
        }
        for (child, cm) in o.components.iter().rev() {
            placed_components += 1;
            stack.push((child.clone(), compose(&scale_x(*cm), &m), depth + 1));
        }
    }
    if placed_components > 0 {
        out.notes.push(format!("{placed_components} components were placed by their transforms"));
    }
    if bad_triangles > 0 {
        out.notes.push(format!("{bad_triangles} triangles with bad vertex numbers were skipped"));
    }
    Ok(out)
}

// ------------------------------------------------------------------ XML

/// One start or end tag, with its attributes.
struct Tag<'a> {
    /// Local name, without a namespace prefix.
    name: &'a str,
    closing: bool,
    /// True for `<x/>`.
    empty: bool,
    attrs: Vec<(&'a str, String)>,
}

impl Tag<'_> {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }
}

/// Iterates the tags of an XML text. Text between tags, comments,
/// processing instructions and CDATA are skipped. This is not a full XML
/// parser; it is enough for 3MF.
struct Tags<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Tags<'a> {
    fn new(s: &'a str) -> Self {
        Tags { s, i: 0 }
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn unescape(v: &str) -> String {
    if !v.contains('&') {
        return v.to_string();
    }
    v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

impl<'a> Iterator for Tags<'a> {
    type Item = Tag<'a>;
    fn next(&mut self) -> Option<Tag<'a>> {
        loop {
            let rest = self.s.get(self.i..)?;
            let lt = rest.find('<')?;
            let start = self.i + lt;
            let body = &self.s[start + 1..];
            // Comments, CDATA and declarations end with their own marker.
            for (open, close) in [("!--", "-->"), ("![CDATA[", "]]>"), ("?", "?>"), ("!", ">")] {
                if body.starts_with(open) {
                    let end = body.find(close)?;
                    self.i = start + 1 + end + close.len();
                    break;
                }
            }
            if self.i > start {
                continue;
            }
            let gt = body.find('>')?;
            self.i = start + 1 + gt + 1;
            let mut inner = &body[..gt];
            let closing = inner.starts_with('/');
            if closing {
                inner = &inner[1..];
            }
            let empty = inner.ends_with('/');
            if empty {
                inner = &inner[..inner.len() - 1];
            }
            let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
            let name = local(&inner[..name_end]);
            let mut attrs = Vec::new();
            let mut a = &inner[name_end..];
            while let Some(eq) = a.find('=') {
                let key = local(a[..eq].trim());
                let after = a[eq + 1..].trim_start();
                let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
                let Some(close) = after[1..].find(q) else { break };
                attrs.push((key, unescape(&after[1..1 + close])));
                a = &after[1 + close + 1..];
            }
            return Some(Tag { name, closing, empty, attrs });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="centimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
<!-- a comment with <tags> inside -->
<resources>
<object id="1" type="model"><mesh><vertices>
<vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/><vertex x="0" y="0" z="1"/>
</vertices><triangles>
<triangle v1="0" v2="2" v3="1"/><triangle v1="0" v2="1" v3="3"/><triangle v1="0" v2="3" v3="2"/><triangle v1="1" v2="2" v3="3"/>
</triangles></mesh></object>
<object id="2" type="model"><components>
<component objectid="1"/>
<component objectid="1" transform="1 0 0 0 1 0 0 0 1 5 0 0"/>
</components></object>
</resources>
<build><item objectid="2"/></build>
</model>"#;

    #[test]
    fn components_are_placed_and_units_scaled() {
        let got = parse_model(MODEL).unwrap();
        assert_eq!(got.meshes.len(), 2, "{:?}", got.notes);
        // Centimetres: the tetrahedron edge is 10 mm, the copy moved 50 mm.
        let xmax = |m: &anvil_kernel::TriMesh| m.positions.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        assert!((xmax(&got.meshes[0]) - 10.0).abs() < 1e-9);
        assert!((xmax(&got.meshes[1]) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn an_unknown_unit_is_refused() {
        let bad = MODEL.replace("centimeter", "furlong");
        assert!(parse_model(&bad).is_err());
    }

    #[test]
    fn a_zip_built_by_the_writer_opens() {
        // Stored entries, like anvil-io's writer.
        let bytes = crate::import::zip::tests::store(&[("3D/3dmodel.model", MODEL.as_bytes())]);
        let got = read_bytes(&bytes).unwrap();
        assert_eq!(got.meshes.len(), 2);
    }

    #[test]
    fn a_deflated_entry_opens() {
        use std::io::Write;
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(MODEL.as_bytes()).unwrap();
        let packed = enc.finish().unwrap();
        let bytes = crate::import::zip::tests::build(&[("3D/3dmodel.model", &packed, MODEL.len() as u32, 8)]);
        let got = read_bytes(&bytes).unwrap();
        assert_eq!(got.meshes.len(), 2);
    }
}
