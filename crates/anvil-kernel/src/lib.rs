//! Anvil geometry kernel.
//!
//! Scope of this first kernel (see `docs/adr/0001-kernel-strategy.md`):
//! * A **Solid** is a boundary representation (B-rep): faces, edges, vertices.
//! * Every face is a planar polygon with one outer loop and zero or more
//!   inner loops (holes). Curved surfaces are stored as many planar facets
//!   plus a `Surface` tag that remembers the analytic surface they came from.
//! * **Tessellation** turns a Solid into a `TriMesh` for display and export.
//! * **Operations** live in `ops`: `extrude`, `revolve`, and stubs for
//!   `fillet` and `boolean` that return `KernelError::Unsupported` until a
//!   real implementation lands.
//!
//! The `Kernel` trait is the seam for swapping in another backend later
//! (truck, OpenCASCADE via FFI). Everything above the kernel talks to the
//! trait, not to the structs directly.

pub mod mesh;
pub mod ops;
pub mod topology;

pub use mesh::TriMesh;
pub use topology::{Edge, EdgeId, Face, FaceId, Solid, Surface, Vertex, VertexId};

use anvil_math::{Axis, DVec2, Plane};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("operation not supported by this kernel yet: {0}")]
    Unsupported(&'static str),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("profile must have at least 3 points")]
    DegenerateProfile,
    #[error("boolean failed: {0}")]
    BooleanFailed(String),
}

pub type KernelResult<T> = Result<T, KernelError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
}

/// The seam between features and geometry. Implement this to plug in a
/// different modeling backend.
pub trait Kernel {
    fn extrude(&self, plane: &Plane, profile: &[DVec2], distance: f64) -> KernelResult<Solid>;
    fn revolve(&self, plane: &Plane, profile: &[DVec2], axis: &Axis, angle: f64) -> KernelResult<Solid>;
    fn boolean(&self, a: &Solid, b: &Solid, op: BooleanOp) -> KernelResult<Solid>;
    fn fillet(&self, solid: &Solid, edges: &[EdgeId], radius: f64) -> KernelResult<Solid>;
    fn tessellate(&self, solid: &Solid) -> TriMesh;
}

/// The built-in polyhedral kernel.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeKernel;

impl Kernel for NativeKernel {
    fn extrude(&self, plane: &Plane, profile: &[DVec2], distance: f64) -> KernelResult<Solid> {
        ops::extrude(plane, profile, distance)
    }
    fn revolve(&self, plane: &Plane, profile: &[DVec2], axis: &Axis, angle: f64) -> KernelResult<Solid> {
        ops::revolve(plane, profile, axis, angle)
    }
    fn boolean(&self, a: &Solid, b: &Solid, op: BooleanOp) -> KernelResult<Solid> {
        ops::boolean(a, b, op)
    }
    fn fillet(&self, solid: &Solid, edges: &[EdgeId], radius: f64) -> KernelResult<Solid> {
        ops::fillet(solid, edges, radius)
    }
    fn tessellate(&self, solid: &Solid) -> TriMesh {
        mesh::tessellate(solid)
    }
}
