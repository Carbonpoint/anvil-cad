//! Export to open formats, one entry point for the Export dialog and the
//! command line. Every writer here is self contained: no compression
//! libraries, no external tools.
//!
//! Mesh formats take the tessellated bodies: STL, 3MF, OBJ, PLY, OFF,
//! AMF, glTF. STEP takes the B-rep itself as a faceted B-rep (planar
//! faces with polygon loops), which every STEP reader accepts and which
//! is exact for this kernel because its faces are planar.

use crate::{document_parts, IoError, Part};
use anvil_feature::Document;
use anvil_kernel::TriMesh;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

/// An export format the dialog offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Binary STL, all bodies in one file.
    Stl,
    /// One binary STL per feature next to the chosen file.
    StlParts,
    /// 3MF with one object per feature.
    ThreeMf,
    /// 3MF with one object made of one part per feature. Slicers print
    /// it as one object and merge the overlapping parts.
    ThreeMfGroup,
    /// Wavefront OBJ, one group per feature.
    Obj,
    /// Binary PLY, all bodies in one mesh.
    Ply,
    /// OFF, all bodies in one mesh.
    Off,
    /// AMF, one object with one volume per feature.
    Amf,
    /// glTF 2.0 with the buffer embedded, one mesh per feature.
    Gltf,
    /// STEP AP214 faceted B-rep, one solid per body.
    Step,
    /// The native document.
    Anvil,
}

impl Format {
    pub const ALL: [Format; 11] = [
        Format::Stl,
        Format::StlParts,
        Format::ThreeMf,
        Format::ThreeMfGroup,
        Format::Obj,
        Format::Ply,
        Format::Off,
        Format::Amf,
        Format::Gltf,
        Format::Step,
        Format::Anvil,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Format::Stl => "STL (one file)",
            Format::StlParts => "STL (one file per feature)",
            Format::ThreeMf => "3MF (one object per feature)",
            Format::ThreeMfGroup => "3MF (one object, parts merged by the slicer)",
            Format::Obj => "OBJ",
            Format::Ply => "PLY (binary)",
            Format::Off => "OFF",
            Format::Amf => "AMF",
            Format::Gltf => "glTF 2.0",
            Format::Step => "STEP AP214 (faceted B-rep)",
            Format::Anvil => "Anvil document",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Stl | Format::StlParts => "stl",
            Format::ThreeMf | Format::ThreeMfGroup => "3mf",
            Format::Obj => "obj",
            Format::Ply => "ply",
            Format::Off => "off",
            Format::Amf => "amf",
            Format::Gltf => "gltf",
            Format::Step => "step",
            Format::Anvil => "anvil",
        }
    }

    /// What the format is good for, shown under the format list.
    pub fn hint(self) -> &'static str {
        match self {
            Format::Stl => "Any slicer or mesh tool. No names, no colours.",
            Format::StlParts => "One file per feature for multi-material or assembly prints.",
            Format::ThreeMf => "Slicers load each feature as a separate object.",
            Format::ThreeMfGroup => "Slicers load one object; overlapping features print as one part.",
            Format::Obj => "Blender, Unity, most viewers. Groups keep the feature names.",
            Format::Ply => "Point cloud and mesh tools such as MeshLab.",
            Format::Off => "Geometry research tools such as CGAL.",
            Format::Amf => "Older slicers; one volume per feature.",
            Format::Gltf => "Web viewers and game engines.",
            Format::Step => "FreeCAD, Fusion, SolidWorks, NX. Faces are planar facets.",
            Format::Anvil => "This document with its full history.",
        }
    }

    /// The format whose extension matches `path`, if any.
    pub fn from_path(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "stl" => Some(Format::Stl),
            "3mf" => Some(Format::ThreeMf),
            "obj" => Some(Format::Obj),
            "ply" => Some(Format::Ply),
            "off" => Some(Format::Off),
            "amf" => Some(Format::Amf),
            "gltf" => Some(Format::Gltf),
            "step" | "stp" => Some(Format::Step),
            "anvil" => Some(Format::Anvil),
            _ => None,
        }
    }
}

/// Write the document in `format` to `path`. Returns a one line note for
/// the status bar.
pub fn write(doc: &Document, format: Format, path: &Path) -> Result<String, IoError> {
    let parts = document_parts(doc);
    let tris: usize = parts.iter().map(|p| p.mesh.triangle_count()).sum();
    let note = |what: &str| format!("Wrote {} ({what}, {} features, {tris} triangles)", path.display(), parts.len());
    match format {
        Format::Stl => {
            let mut all = TriMesh::default();
            for p in &parts {
                all.append(p.mesh.clone());
            }
            crate::write_stl(&all, path)?;
            Ok(note("STL"))
        }
        Format::StlParts => {
            let files = crate::write_stl_parts(doc, path)?;
            Ok(format!("Wrote {} STL files next to {}", files.len(), path.display()))
        }
        Format::ThreeMf => {
            crate::write_3mf(doc, path)?;
            Ok(note("3MF, one object per feature"))
        }
        Format::ThreeMfGroup => {
            crate::write_3mf_group(doc, path)?;
            Ok(note("3MF, one object"))
        }
        Format::Obj => {
            std::fs::write(path, obj(&parts))?;
            Ok(note("OBJ"))
        }
        Format::Ply => {
            std::fs::write(path, ply(&parts))?;
            Ok(note("PLY"))
        }
        Format::Off => {
            std::fs::write(path, off(&parts))?;
            Ok(note("OFF"))
        }
        Format::Amf => {
            std::fs::write(path, amf(&parts))?;
            Ok(note("AMF"))
        }
        Format::Gltf => {
            std::fs::write(path, gltf(&parts))?;
            Ok(note("glTF"))
        }
        Format::Step => {
            let n = step(doc, path)?;
            Ok(format!("Wrote {} (STEP, {n} faceted solids)", path.display()))
        }
        Format::Anvil => {
            crate::save_document(doc, path)?;
            Ok(format!("Saved {}", path.display()))
        }
    }
}

fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn obj(parts: &[Part]) -> String {
    let mut s = String::from("# Anvil CAD, millimetres\n");
    let mut base = 1usize;
    for p in parts {
        let _ = writeln!(s, "o {}_{}", p.feature, safe(&p.name));
        for v in &p.mesh.positions {
            let _ = writeln!(s, "v {:.6} {:.6} {:.6}", v.x, v.y, v.z);
        }
        for t in p.mesh.indices.as_chunks::<3>().0 {
            let _ = writeln!(s, "f {} {} {}", base + t[0] as usize, base + t[1] as usize, base + t[2] as usize);
        }
        base += p.mesh.positions.len();
    }
    s
}

fn ply(parts: &[Part]) -> Vec<u8> {
    let nv: usize = parts.iter().map(|p| p.mesh.positions.len()).sum();
    let nf: usize = parts.iter().map(|p| p.mesh.triangle_count()).sum();
    let mut out = format!(
        "ply\nformat binary_little_endian 1.0\ncomment Anvil CAD, millimetres\nelement vertex {nv}\nproperty float x\nproperty float y\nproperty float z\nelement face {nf}\nproperty list uchar int vertex_indices\nend_header\n"
    )
    .into_bytes();
    for p in parts {
        for v in &p.mesh.positions {
            for x in [v.x, v.y, v.z] {
                out.extend_from_slice(&(x as f32).to_le_bytes());
            }
        }
    }
    let mut base = 0u32;
    for p in parts {
        for t in p.mesh.indices.as_chunks::<3>().0 {
            out.push(3);
            for &i in t {
                out.extend_from_slice(&((base + i) as i32).to_le_bytes());
            }
        }
        base += p.mesh.positions.len() as u32;
    }
    out
}

fn off(parts: &[Part]) -> String {
    let nv: usize = parts.iter().map(|p| p.mesh.positions.len()).sum();
    let nf: usize = parts.iter().map(|p| p.mesh.triangle_count()).sum();
    let mut s = format!("OFF\n{nv} {nf} 0\n");
    for p in parts {
        for v in &p.mesh.positions {
            let _ = writeln!(s, "{:.6} {:.6} {:.6}", v.x, v.y, v.z);
        }
    }
    let mut base = 0usize;
    for p in parts {
        for t in p.mesh.indices.as_chunks::<3>().0 {
            let _ = writeln!(s, "3 {} {} {}", base + t[0] as usize, base + t[1] as usize, base + t[2] as usize);
        }
        base += p.mesh.positions.len();
    }
    s
}

fn amf(parts: &[Part]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<amf unit=\"millimeter\" version=\"1.1\">\n<object id=\"1\"><mesh>\n<vertices>\n");
    for p in parts {
        for v in &p.mesh.positions {
            let _ = writeln!(
                s,
                "<vertex><coordinates><x>{:.6}</x><y>{:.6}</y><z>{:.6}</z></coordinates></vertex>",
                v.x, v.y, v.z
            );
        }
    }
    s.push_str("</vertices>\n");
    let mut base = 0usize;
    for p in parts {
        let _ = writeln!(s, "<volume><metadata type=\"name\">{}</metadata>", crate::xml_escape(&p.name));
        for t in p.mesh.indices.as_chunks::<3>().0 {
            let _ = writeln!(
                s,
                "<triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>",
                base + t[0] as usize,
                base + t[1] as usize,
                base + t[2] as usize
            );
        }
        s.push_str("</volume>\n");
        base += p.mesh.positions.len();
    }
    s.push_str("</mesh></object>\n</amf>\n");
    s
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

/// glTF 2.0 in one file: JSON with the binary buffer as a data URI. One
/// mesh and one node per feature; positions float32, indices uint32.
/// glTF is metres and Y up, so the scene node scales by 0.001 and turns
/// Z up into Y up.
fn gltf(parts: &[Part]) -> String {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = String::new();
    let mut accessors = String::new();
    let mut meshes = String::new();
    let mut nodes = String::new();
    let mut acc = 0usize;
    let mut view = 0usize;
    for (k, p) in parts.iter().enumerate() {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let pos_off = bin.len();
        for v in &p.mesh.positions {
            for (i, x) in [v.x as f32, v.y as f32, v.z as f32].into_iter().enumerate() {
                lo[i] = lo[i].min(x);
                hi[i] = hi[i].max(x);
                bin.extend_from_slice(&x.to_le_bytes());
            }
        }
        let pos_len = bin.len() - pos_off;
        let idx_off = bin.len();
        for &i in &p.mesh.indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        let idx_len = bin.len() - idx_off;
        let _ = write!(
            views,
            "{{\"buffer\":0,\"byteOffset\":{pos_off},\"byteLength\":{pos_len},\"target\":34962}},{{\"buffer\":0,\"byteOffset\":{idx_off},\"byteLength\":{idx_len},\"target\":34963}},"
        );
        let _ = write!(
            accessors,
            "{{\"bufferView\":{view},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}},{{\"bufferView\":{},\"componentType\":5125,\"count\":{},\"type\":\"SCALAR\"}},",
            p.mesh.positions.len(),
            lo[0],
            lo[1],
            lo[2],
            hi[0],
            hi[1],
            hi[2],
            view + 1,
            p.mesh.indices.len()
        );
        let _ = write!(
            meshes,
            "{{\"name\":\"{}\",\"primitives\":[{{\"attributes\":{{\"POSITION\":{acc}}},\"indices\":{}}}]}},",
            json_escape(&p.name),
            acc + 1
        );
        let _ = write!(nodes, "{{\"mesh\":{k},\"name\":\"{}\"}},", json_escape(&p.name));
        acc += 2;
        view += 2;
    }
    let children: Vec<String> = (1..=parts.len()).map(|i| i.to_string()).collect();
    let root = format!(
        "{{\"name\":\"Anvil (mm, Z up)\",\"children\":[{}],\"scale\":[0.001,0.001,0.001],\"rotation\":[-0.7071068,0,0,0.7071068]}}",
        children.join(",")
    );
    format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"Anvil CAD\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{root},{}],\"meshes\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{},\"uri\":\"data:application/octet-stream;base64,{}\"}}]}}\n",
        nodes.trim_end_matches(','),
        meshes.trim_end_matches(','),
        accessors.trim_end_matches(','),
        views.trim_end_matches(','),
        bin.len(),
        base64(&bin)
    )
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// STEP AP214, faceted B-rep: every face of every visible body becomes a
/// FACE_SURFACE on a PLANE bounded by POLY_LOOPs, so the file is exact
/// for the planar kernel. Returns the number of solids written.
pub fn step(doc: &Document, path: &Path) -> Result<usize, IoError> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("part.step");
    writeln!(f, "ISO-10303-21;")?;
    writeln!(f, "HEADER;")?;
    writeln!(f, "FILE_DESCRIPTION(('Anvil CAD faceted B-rep export'),'2;1');")?;
    writeln!(
        f,
        "FILE_NAME('{}','2026-01-01T00:00:00',('Anvil'),(''),'Anvil CAD','Anvil CAD','');",
        name.replace('\'', "")
    )?;
    writeln!(f, "FILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));")?;
    writeln!(f, "ENDSEC;")?;
    writeln!(f, "DATA;")?;
    writeln!(f, "#1=APPLICATION_CONTEXT('automotive design');")?;
    writeln!(f, "#2=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#1);")?;
    writeln!(f, "#3=PRODUCT_CONTEXT('',#1,'mechanical');")?;
    writeln!(f, "#4=PRODUCT('{0}','{0}','',(#3));", safe(&doc.name))?;
    writeln!(f, "#5=PRODUCT_DEFINITION_FORMATION('','',#4);")?;
    writeln!(f, "#6=PRODUCT_DEFINITION_CONTEXT('part definition',#1,'design');")?;
    writeln!(f, "#7=PRODUCT_DEFINITION('design','',#5,#6);")?;
    writeln!(f, "#8=PRODUCT_DEFINITION_SHAPE('','',#7);")?;
    writeln!(f, "#9=( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );")?;
    writeln!(f, "#10=( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) );")?;
    writeln!(f, "#11=( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() );")?;
    writeln!(f, "#12=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#9,'distance_accuracy_value','');")?;
    writeln!(f, "#13=( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#12)) GLOBAL_UNIT_ASSIGNED_CONTEXT((#9,#10,#11)) REPRESENTATION_CONTEXT('','') );")?;
    writeln!(f, "#14=CARTESIAN_POINT('',(0.,0.,0.));")?;
    writeln!(f, "#15=DIRECTION('',(0.,0.,1.));")?;
    writeln!(f, "#16=DIRECTION('',(1.,0.,0.));")?;
    writeln!(f, "#17=AXIS2_PLACEMENT_3D('',#14,#15,#16);")?;
    writeln!(f, "#18=PRODUCT_RELATED_PRODUCT_CATEGORY('part','',(#4));")?;
    let mut id = 100usize;
    let mut next = || {
        id += 1;
        id
    };
    let mut breps: Vec<usize> = Vec::new();
    let num = |x: f64| {
        let s = format!("{x:.6}");
        let s = s.trim_end_matches('0').to_string();
        if s.ends_with('.') {
            s + "0"
        } else {
            s
        }
    };
    for (_, body) in doc.visible_bodies() {
        // One CARTESIAN_POINT per vertex.
        let mut point_of: std::collections::HashMap<anvil_kernel::VertexId, usize> = std::collections::HashMap::new();
        for (vid, v) in &body.vertices {
            let n = next();
            writeln!(f, "#{n}=CARTESIAN_POINT('',({},{},{}));", num(v.pos.x), num(v.pos.y), num(v.pos.z))?;
            point_of.insert(vid, n);
        }
        let mut faces: Vec<usize> = Vec::new();
        for face in body.faces.values() {
            if face.outer.len() < 3 {
                continue;
            }
            let n = body.face_normal(face);
            if n.length_squared() < 1e-24 {
                continue;
            }
            let origin = body.vertices[face.outer[0]].pos;
            let helper = if n.z.abs() < 0.9 { anvil_math::DVec3::Z } else { anvil_math::DVec3::X };
            let xdir = helper.cross(n).normalize();
            let p = next();
            writeln!(f, "#{p}=CARTESIAN_POINT('',({},{},{}));", num(origin.x), num(origin.y), num(origin.z))?;
            let d = next();
            writeln!(f, "#{d}=DIRECTION('',({},{},{}));", num(n.x), num(n.y), num(n.z))?;
            let x = next();
            writeln!(f, "#{x}=DIRECTION('',({},{},{}));", num(xdir.x), num(xdir.y), num(xdir.z))?;
            let ax = next();
            writeln!(f, "#{ax}=AXIS2_PLACEMENT_3D('',#{p},#{d},#{x});")?;
            let plane = next();
            writeln!(f, "#{plane}=PLANE('',#{ax});")?;
            let mut bounds: Vec<String> = Vec::new();
            for (k, lp) in std::iter::once(&face.outer).chain(face.inner.iter()).enumerate() {
                let pts: Vec<String> = lp.iter().filter_map(|v| point_of.get(v)).map(|n| format!("#{n}")).collect();
                if pts.len() < 3 {
                    continue;
                }
                let poly = next();
                writeln!(f, "#{poly}=POLY_LOOP('',({}));", pts.join(","))?;
                let b = next();
                if k == 0 {
                    writeln!(f, "#{b}=FACE_OUTER_BOUND('',#{poly},.T.);")?;
                } else {
                    writeln!(f, "#{b}=FACE_BOUND('',#{poly},.T.);")?;
                }
                bounds.push(format!("#{b}"));
            }
            let fs = next();
            writeln!(f, "#{fs}=FACE_SURFACE('',({}),#{plane},.T.);", bounds.join(","))?;
            faces.push(fs);
        }
        if faces.is_empty() {
            continue;
        }
        let shell = next();
        let list: Vec<String> = faces.iter().map(|n| format!("#{n}")).collect();
        writeln!(f, "#{shell}=CLOSED_SHELL('',({}));", list.join(","))?;
        let brep = next();
        writeln!(f, "#{brep}=FACETED_BREP('',#{shell});")?;
        breps.push(brep);
    }
    let items: Vec<String> = std::iter::once("#17".to_string()).chain(breps.iter().map(|n| format!("#{n}"))).collect();
    let rep = next();
    writeln!(f, "#{rep}=SHAPE_REPRESENTATION('',({}),#13);", items.join(","))?;
    let sdr = next();
    writeln!(f, "#{sdr}=SHAPE_DEFINITION_REPRESENTATION(#8,#{rep});")?;
    writeln!(f, "ENDSEC;")?;
    writeln!(f, "END-ISO-10303-21;")?;
    f.flush()?;
    Ok(breps.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_feature::features::primitives::BoxFeature;

    fn doc() -> Document {
        let mut d = Document::new("t");
        d.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "10".into(),
            depth: "20".into(),
            height: "5".into(),
        }));
        d
    }

    #[test]
    fn every_format_writes_a_file() {
        let d = doc();
        let dir = std::env::temp_dir().join(format!("anvil_export_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in Format::ALL {
            let p = dir.join(format!("box.{}", f.extension()));
            let note = write(&d, f, &p).unwrap();
            assert!(p.exists(), "{f:?} wrote nothing");
            assert!(std::fs::metadata(&p).unwrap().len() > 50, "{f:?} is empty");
            assert!(note.contains("box") || note.contains("STL files"), "{note}");
            assert_eq!(Format::from_path(&p).map(|g| g.extension()), Some(f.extension()));
        }
        let step_text = std::fs::read_to_string(dir.join("box.step")).unwrap();
        assert!(step_text.contains("FACETED_BREP") && step_text.contains("END-ISO-10303-21;"));
        // A box has 6 faces, each with an outer bound.
        assert_eq!(step_text.matches("FACE_OUTER_BOUND").count(), 6);
        let obj_text = std::fs::read_to_string(dir.join("box.obj")).unwrap();
        assert_eq!(obj_text.matches("\nf ").count(), 12);
        let gltf_text = std::fs::read_to_string(dir.join("box.gltf")).unwrap();
        assert!(gltf_text.starts_with("{\"asset\"") && gltf_text.contains("base64,"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
