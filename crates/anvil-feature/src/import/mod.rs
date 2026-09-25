//! Readers for neutral files that other CAD tools write: STEP and 3MF.
//!
//! These live in `anvil-feature` so that the Insert Mesh and Import STEP
//! features can use them; `anvil-io` re-exports them as `read_step` and
//! `read_3mf`. They read untrusted files, so they return errors and never
//! panic on bad input.

mod curved;
pub mod step;
mod surface;
pub mod svg;
mod tess;
pub mod threemf;
pub mod xml;
mod zip;

use anvil_kernel::{Solid, TriMesh};

/// Bodies read from a file, with anything the reader could not use.
#[derive(Clone, Debug, Default)]
pub struct Imported {
    pub solids: Vec<Solid>,
    pub meshes: Vec<TriMesh>,
    /// One line per thing skipped, for example
    /// "12 faces on a CYLINDRICAL_SURFACE were skipped".
    pub notes: Vec<String>,
}

impl Imported {
    /// Every triangle of every mesh, for features that build a Body from
    /// triangles.
    pub fn triangles(&self) -> Vec<[anvil_math::DVec3; 3]> {
        let mut out = Vec::new();
        for m in &self.meshes {
            for t in m.indices.as_chunks::<3>().0 {
                let p = |i: u32| m.positions.get(i as usize).copied();
                if let (Some(a), Some(b), Some(c)) = (p(t[0]), p(t[1]), p(t[2])) {
                    out.push([a, b, c]);
                }
            }
        }
        out
    }
}

/// Build a `TriMesh` from positions and triangle indices. Normals are per
/// vertex, the average of the triangles around it.
pub(crate) fn tri_mesh(positions: Vec<anvil_math::DVec3>, indices: Vec<u32>) -> TriMesh {
    let mut normals = vec![anvil_math::DVec3::ZERO; positions.len()];
    for t in indices.as_chunks::<3>().0 {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if let (Some(&pa), Some(&pb), Some(&pc)) = (positions.get(a), positions.get(b), positions.get(c)) {
            let n = (pb - pa).cross(pc - pa);
            for i in [a, b, c] {
                normals[i] += n;
            }
        }
    }
    for n in &mut normals {
        *n = n.normalize_or_zero();
    }
    let face_of_tri = vec![anvil_kernel::FaceId::default(); indices.len() / 3];
    TriMesh { positions, normals, indices, face_of_tri }
}
