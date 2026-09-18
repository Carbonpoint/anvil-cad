//! 3MF with the beam lattice extension: beams as nodes and segments with
//! a radius, the compact form printers and slicers that read the
//! extension want for lattices, instead of millions of triangles.

use crate::{xml_escape, zip_store, IoError};
use anvil_math::DVec3;
use std::path::Path;

/// Write `beams` (world segments in mm) with one `radius` as a 3MF beam
/// lattice object named `name`. Returns the node count.
pub fn write_3mf_beams(beams: &[(DVec3, DVec3)], radius: f64, name: &str, path: &Path) -> Result<usize, IoError> {
    let key = |p: DVec3| ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64);
    let mut index: std::collections::HashMap<(i64, i64, i64), usize> = std::collections::HashMap::new();
    let mut nodes: Vec<DVec3> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(beams.len());
    for (a, b) in beams {
        let mut id = |p: DVec3| {
            *index.entry(key(p)).or_insert_with(|| {
                nodes.push(p);
                nodes.len() - 1
            })
        };
        let (ia, ib) = (id(*a), id(*b));
        if ia != ib {
            edges.push((ia, ib));
        }
    }
    let mut model = String::new();
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\" xmlns:b=\"http://schemas.microsoft.com/3dmanufacturing/beamlattice/2017/02\" requiredextensions=\"b\">\n<resources>\n");
    model.push_str(&format!("<object id=\"1\" name=\"{}\" type=\"model\"><mesh><vertices>\n", xml_escape(name)));
    for v in &nodes {
        model.push_str(&format!("<vertex x=\"{:.5}\" y=\"{:.5}\" z=\"{:.5}\"/>\n", v.x, v.y, v.z));
    }
    model.push_str("</vertices><triangles/>\n");
    model.push_str(&format!(
        "<b:beamlattice radius=\"{radius:.5}\" minlength=\"{:.5}\" cap=\"sphere\"><b:beams>\n",
        radius * 0.1
    ));
    for (a, b) in &edges {
        model.push_str(&format!("<b:beam v1=\"{a}\" v2=\"{b}\"/>\n"));
    }
    model.push_str("</b:beams></b:beamlattice></mesh></object>\n</resources>\n<build>\n<item objectid=\"1\"/>\n</build>\n</model>\n");
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    let bytes = zip_store(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ]);
    std::fs::write(path, bytes)?;
    Ok(nodes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beam_file_declares_the_extension() {
        let beams =
            vec![(DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0)), (DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 0.0))];
        let p = std::env::temp_dir().join(format!("anvil_beams_{}.3mf", std::process::id()));
        let n = write_3mf_beams(&beams, 0.5, "test", &p).unwrap();
        assert_eq!(n, 3);
        let bytes = std::fs::read(&p).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("beamlattice/2017/02") && text.contains("requiredextensions=\"b\""));
        assert_eq!(text.matches("<b:beam ").count(), 2);
        std::fs::remove_file(&p).ok();
    }
}
