//! Static feature registry.
//!
//! Each feature module submits one `FeatureDescriptor` with
//! `inventory::submit!`. The ribbon reads `descriptors()` at startup, so a new
//! feature appears in the UI without editing any central list.

use crate::Feature;

#[derive(Debug)]
pub struct FeatureDescriptor {
    /// Stable id, for example `"extrude"`.
    pub id: &'static str,
    /// Button label.
    pub label: &'static str,
    /// Ribbon tab, for example `"Home"`.
    pub tab: &'static str,
    /// Group inside the tab, for example `"Feature"`.
    pub group: &'static str,
    /// Short tooltip.
    pub tooltip: &'static str,
    /// Sort key inside the group. Lower comes first.
    pub order: u32,
    /// Construct a feature with default parameters.
    pub create: fn() -> Box<dyn Feature>,
}

inventory::collect!(FeatureDescriptor);

/// Every registered descriptor, sorted by tab, group, order.
pub fn descriptors() -> Vec<&'static FeatureDescriptor> {
    let mut v: Vec<_> = inventory::iter::<FeatureDescriptor>.into_iter().collect();
    v.sort_by_key(|d| (d.tab, d.group, d.order, d.label));
    v
}

pub fn descriptor(id: &str) -> Option<&'static FeatureDescriptor> {
    inventory::iter::<FeatureDescriptor>.into_iter().find(|d| d.id == id)
}
