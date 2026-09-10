//! Import and export.
//!
//! * `.anvil`: native document, JSON today. See ADR 0002 for the planned
//!   zip container with binary blobs.
//! * `.stl`: binary STL export of tessellated bodies.
//! * STEP, IGES, 3MF, DXF: planned. See `docs/research/04-...md`.

use anvil_feature::Document;
use anvil_kernel::TriMesh;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json_error::Error),
}

mod serde_json_error {
    pub use anvil_feature::serde_json_error::Error;
}

pub fn save_document(doc: &Document, path: &Path) -> Result<(), IoError> {
    let json = doc.to_json()?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_document(path: &Path) -> Result<Document, IoError> {
    let s = std::fs::read_to_string(path)?;
    Ok(Document::from_json(&s)?)
}

/// Write a binary STL. Units are millimetres.
pub fn write_stl(mesh: &TriMesh, path: &Path) -> Result<(), IoError> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut header = [0u8; 80];
    let tag = b"Anvil CAD binary STL";
    header[..tag.len()].copy_from_slice(tag);
    f.write_all(&header)?;
    f.write_all(&(mesh.triangle_count() as u32).to_le_bytes())?;
    for t in mesh.indices.as_chunks::<3>().0 {
        let a = mesh.positions[t[0] as usize];
        let b = mesh.positions[t[1] as usize];
        let c = mesh.positions[t[2] as usize];
        let n = (b - a).cross(c - a).normalize_or_zero();
        for v in [n, a, b, c] {
            for x in [v.x, v.y, v.z] {
                f.write_all(&(x as f32).to_le_bytes())?;
            }
        }
        f.write_all(&0u16.to_le_bytes())?;
    }
    f.flush()?;
    Ok(())
}

/// Read a binary or ASCII STL into triangles (millimetres).
pub fn read_stl(path: &Path) -> Result<Vec<[anvil_math::DVec3; 3]>, IoError> {
    use anvil_math::DVec3;
    let bytes = std::fs::read(path)?;
    let looks_ascii = bytes.starts_with(b"solid") && bytes.len() >= 84 && {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        84 + count * 50 != bytes.len()
    };
    let mut tris = Vec::new();
    if looks_ascii || bytes.len() < 84 {
        let text = String::from_utf8_lossy(&bytes);
        let mut cur: Vec<DVec3> = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() == Some("vertex") {
                let v: Vec<f64> = it.take(3).filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    cur.push(DVec3::new(v[0], v[1], v[2]));
                }
                if cur.len() == 3 {
                    tris.push([cur[0], cur[1], cur[2]]);
                    cur.clear();
                }
            }
        }
    } else {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        let f = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f64;
        for i in 0..count {
            let base = 84 + i * 50 + 12;
            if base + 36 > bytes.len() {
                break;
            }
            let p = |k: usize| DVec3::new(f(base + k * 12), f(base + k * 12 + 4), f(base + k * 12 + 8));
            tris.push([p(0), p(1), p(2)]);
        }
    }
    Ok(tris)
}

/// Tessellate every body in a document into one mesh.
pub fn document_mesh(doc: &Document) -> TriMesh {
    let mut m = TriMesh::default();
    for b in doc.bodies() {
        m.append(anvil_kernel::mesh::tessellate(b));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_feature::features::{extrude::ExtrudeFeature, sketch::SketchFeature};

    #[test]
    fn stl_round_trip_size() {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(SketchFeature::rectangle("XY", 40.0, 25.0)));
        doc.add_feature(Box::new(ExtrudeFeature::default()));
        let mesh = document_mesh(&doc);
        assert_eq!(mesh.triangle_count(), 12);
        let dir = std::env::temp_dir().join("anvil_io_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("cube.stl");
        write_stl(&mesh, &p).unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 84 + 12 * 50);
        let tris = read_stl(&p).unwrap();
        assert_eq!(tris.len(), 12);
        let solid = anvil_kernel::ops::from_triangles(&tris, 1e-6);
        assert!((solid.volume() - doc.bodies()[0].volume()).abs() < 1e-6);
    }
}

/// One exportable part: a feature's visible bodies merged into one mesh.
pub struct Part {
    pub feature: usize,
    pub name: String,
    pub mesh: TriMesh,
}

/// Group visible bodies by the feature that made them.
pub fn document_parts(doc: &Document) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for (fi, body) in doc.visible_bodies() {
        let m = anvil_kernel::mesh::tessellate(body);
        match parts.iter_mut().find(|p| p.feature == fi) {
            Some(p) => p.mesh.append(m),
            None => parts.push(Part { feature: fi, name: doc.features[fi].feature.name(), mesh: m }),
        }
    }
    parts
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>().trim_matches('_').to_string()
}

/// Write one binary STL per feature: `<stem>_<index>_<name>.stl`.
/// Returns the paths written.
pub fn write_stl_parts(doc: &Document, stem: &Path) -> Result<Vec<std::path::PathBuf>, IoError> {
    let mut out = Vec::new();
    for p in document_parts(doc) {
        let file = stem.with_file_name(format!(
            "{}_{}_{}.stl",
            stem.file_stem().and_then(|s| s.to_str()).unwrap_or("part"),
            p.feature,
            safe_name(&p.name)
        ));
        write_stl(&p.mesh, &file)?;
        out.push(file);
    }
    Ok(out)
}

/// Write a 3MF with one object per feature. Slicers such as Bambu Studio
/// and PrusaSlicer load each object as a separate part.
pub fn write_3mf(doc: &Document, path: &Path) -> Result<usize, IoError> {
    let parts = document_parts(doc);
    let mut model = String::new();
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n<resources>\n");
    for (k, p) in parts.iter().enumerate() {
        let id = k + 1;
        model.push_str(&format!(
            "<object id=\"{id}\" name=\"{}\" type=\"model\"><mesh><vertices>\n",
            xml_escape(&p.name)
        ));
        for v in &p.mesh.positions {
            model.push_str(&format!("<vertex x=\"{:.5}\" y=\"{:.5}\" z=\"{:.5}\"/>\n", v.x, v.y, v.z));
        }
        model.push_str("</vertices><triangles>\n");
        for t in p.mesh.indices.as_chunks::<3>().0 {
            model.push_str(&format!("<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n", t[0], t[1], t[2]));
        }
        model.push_str("</triangles></mesh></object>\n");
    }
    model.push_str("</resources>\n<build>\n");
    for k in 0..parts.len() {
        model.push_str(&format!("<item objectid=\"{}\"/>\n", k + 1));
    }
    model.push_str("</build>\n</model>\n");
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    let bytes = zip_store(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ]);
    std::fs::write(path, bytes)?;
    Ok(parts.len())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Minimal ZIP writer, stored (no compression). Enough for 3MF.
fn zip_store(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        let header = |sig: u32, v: &mut Vec<u8>| {
            v.extend_from_slice(&sig.to_le_bytes());
        };
        header(0x0403_4b50, &mut out);
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0u16.to_le_bytes()); // date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        header(0x0201_4b50, &mut central);
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Build the business card sample: credit-card outline with two 0.5 in
/// and two 0.25 in corner fillets, an embossed name, and a QR code, as
/// separate bodies for multi-colour printing.
pub fn business_card(name: &str, url: &str) -> Document {
    use anvil_feature::features::emboss::{QrFeature, TextFeature};
    use anvil_feature::features::extrude::ExtrudeFeature;
    use anvil_feature::features::sketch::SketchFeature;
    let mut doc = Document::new("Business card");
    doc.set_expression("card_w", "85.6").ok();
    doc.set_expression("card_h", "53.98").ok();
    doc.set_expression("card_t", "0.8").ok();
    doc.set_expression("emboss", "0.6").ok();
    doc.set_expression("r_big", "12.7").ok();
    doc.set_expression("r_small", "6.35").ok();
    let mut sk = SketchFeature::on_datum("XY");
    sk.sketch.add_rounded_rectangle(0.0, 0.0, 85.6, 53.98, [12.7, 6.35, 12.7, 6.35]);
    doc.add_feature(Box::new(sk));
    doc.add_feature(Box::new(ExtrudeFeature { sketch: 0, distance: "card_t".into(), symmetric: false }));
    doc.add_feature(Box::new(TextFeature {
        text: name.into(),
        x: "30".into(),
        y: "40".into(),
        z: "card_t".into(),
        size: "7".into(),
        height: "emboss".into(),
        center: true,
        ..Default::default()
    }));
    doc.add_feature(Box::new(QrFeature {
        data: url.into(),
        x: "55".into(),
        y: "6".into(),
        z: "card_t".into(),
        size: "24".into(),
        height: "emboss".into(),
        ..Default::default()
    }));
    doc.appearance.insert(1, [235, 235, 230]);
    doc.appearance.insert(2, [30, 60, 140]);
    doc.appearance.insert(3, [30, 60, 140]);
    doc
}

#[cfg(test)]
mod card_tests {
    use super::*;

    #[test]
    fn business_card_exports_parts() {
        let doc = business_card("Alex Goldman", "https://www.linkedin.com/");
        for (i, f) in doc.features.iter().enumerate() {
            assert!(f.error.is_none(), "feature {i}: {:?}", f.error);
        }
        let parts = document_parts(&doc);
        assert_eq!(parts.len(), 3, "card, text, qr");
        let dir = std::env::temp_dir().join("anvil_card_test");
        std::fs::create_dir_all(&dir).unwrap();
        let n = write_3mf(&doc, &dir.join("card.3mf")).unwrap();
        assert_eq!(n, 3);
        let files = write_stl_parts(&doc, &dir.join("card.stl")).unwrap();
        assert_eq!(files.len(), 3);
    }
}
