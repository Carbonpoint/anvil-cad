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
pub mod mesh_loader;
pub mod param;
pub mod registry;

pub use document::{Document, FeatureId, FeatureNode, Material, RegenContext, RegenError, MATERIALS};
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
pub const PLANE_TYPES: &[&str] = &["sketch", "offset_plane", "angle_plane", "plane_3pt", "midplane"];

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

    /// Put this feature on a picked face. `body` is the feature that made
    /// the face. Returns false if the feature has no placement.
    fn place_on_face(&mut self, _plane: anvil_math::Plane, _body: usize) -> bool {
        false
    }

    /// Take the edges picked in the viewport (world end points) and the
    /// feature that owns them. Returns false if the feature has no edges.
    fn set_edges(&mut self, _edges: Vec<[anvil_math::DVec3; 2]>, _body: usize) -> bool {
        false
    }

    /// Rewrite references to other features after the history changes.
    /// `map(old)` returns the new index, or `None` if the target was
    /// deleted. The default walks `params()` for feature references.
    /// Returns the names of references that now point at nothing.
    fn remap_refs(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<&'static str> {
        let mut broken = Vec::new();
        for p in self.params() {
            if let ParamValue::FeatureRef(old) = p.value {
                match map(old) {
                    Some(new) if new != old => {
                        let _ = self.set_param(p.name, ParamValue::FeatureRef(new));
                    }
                    Some(_) => {}
                    None => {
                        broken.push(p.name);
                        let _ = self.set_param(p.name, ParamValue::FeatureRef(usize::MAX));
                    }
                }
            }
        }
        broken
    }
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
