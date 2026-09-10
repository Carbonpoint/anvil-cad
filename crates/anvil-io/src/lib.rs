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
    }
}
