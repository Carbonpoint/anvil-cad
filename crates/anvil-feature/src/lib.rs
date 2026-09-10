//! Feature history and regeneration.
//!
//! Vocabulary:
//! * **Feature**: one step in the part history (Sketch, Extrude, Revolve, ...).
//! * **Document**: the ordered feature list plus the expression table.
//! * **Regenerate**: run every feature in order and rebuild all bodies.
//! * **Body**: one solid produced by the history.
//! * **Descriptor**: static metadata that tells the ribbon how to offer a
//!   feature. Descriptors self-register through `inventory`.
//!
//! To add a feature, copy `features/template.rs`. See
//! `docs/ADDING_A_FEATURE.md`.

pub mod document;
pub mod features;
pub mod param;
pub mod registry;

pub use document::{Document, FeatureId, FeatureNode, RegenContext, RegenError};
pub use param::{ParamSpec, ParamValue};
pub use registry::{descriptor, descriptors, FeatureDescriptor};

use anvil_kernel::Solid;

/// What a feature produced during regeneration.
#[derive(Clone, Debug, Default)]
pub struct FeatureOutput {
    /// Solids added to the document by this feature.
    pub bodies: Vec<Solid>,
    /// Closed profiles produced by a sketch feature (in the sketch plane).
    pub profiles: Vec<anvil_sketch::Profile>,
    /// Open polylines produced by a sketch feature (in the sketch plane).
    pub paths: Vec<Vec<anvil_math::DVec2>>,
    /// The plane a sketch or construction feature defines.
    pub plane: Option<anvil_math::Plane>,
    /// Indices of upstream features whose bodies this feature replaces.
    /// Their bodies are hidden from `Document::bodies()`.
    pub consumes: Vec<usize>,
}

/// Feature type ids that produce bodies. Used by `feature_ref` params.
pub const BODY_TYPES: &[&str] = &[
    "extrude",
    "revolve",
    "sweep",
    "loft",
    "box",
    "cylinder",
    "sphere",
    "torus",
    "mirror",
    "rect_pattern",
    "circ_pattern",
    "move",
    "scale",
    "fillet",
    "chamfer",
    "shell",
    "combine",
    "hole",
];

/// Feature type ids that define a plane.
pub const PLANE_TYPES: &[&str] = &["sketch", "offset_plane", "angle_plane"];

/// The trait every feature implements.
///
/// Features are plain data plus a `regenerate` step. They must be
/// serialisable so documents can be saved; `typetag` handles the trait object.
#[typetag::serde(tag = "type")]
pub trait Feature: Send + Sync + std::fmt::Debug + std::any::Any {
    /// Stable identifier, matches `FeatureDescriptor::id`.
    fn kind(&self) -> &'static str;
    /// Display name, for the part navigator.
    fn name(&self) -> String;
    /// Editable parameters, for the generic property panel.
    fn params(&self) -> Vec<ParamSpec>;
    /// Update one parameter. Return an error message for bad input.
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String>;
    /// Rebuild this feature's output from its parameters and upstream outputs.
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError>;
    /// Needed because `Box<dyn Feature>` cannot derive Clone.
    fn clone_box(&self) -> Box<dyn Feature>;
}

impl Clone for Box<dyn Feature> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl dyn Feature {
    /// Concrete access for editors that need the real type, for example the
    /// sketch editor. Uses trait upcasting to `Any`.
    pub fn downcast_ref<T: Feature>(&self) -> Option<&T> {
        (self as &dyn std::any::Any).downcast_ref::<T>()
    }
    pub fn downcast_mut<T: Feature>(&mut self) -> Option<&mut T> {
        (self as &mut dyn std::any::Any).downcast_mut::<T>()
    }
}

/// Re-export so downstream crates can name the JSON error type without a
/// direct serde_json dependency.
pub mod serde_json_error {
    pub use serde_json::Error;
}
