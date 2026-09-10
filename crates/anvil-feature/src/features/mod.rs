//! Built-in features. Each file registers itself with `inventory::submit!`.
//!
//! To add one, copy `template.rs`, rename, and add a `pub mod` line here.

pub mod construct;
pub mod emboss;
pub mod extrude;
pub mod fillet;
pub mod loft;
pub mod pending;
pub mod primitives;
pub mod revolve;
pub mod sketch;
pub mod solid_extra;
pub mod sweep;
pub mod template;
pub mod transform;

/// Parse "0, 2, 5" into feature indices.
pub(crate) fn parse_index_list(s: &str) -> Vec<usize> {
    s.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.trim().parse().ok()).collect()
}

/// Map a datum plane name to a plane.
pub(crate) fn datum_plane(name: &str) -> anvil_math::Plane {
    match name {
        "XZ" => anvil_math::Plane::XZ,
        "YZ" => anvil_math::Plane::YZ,
        _ => anvil_math::Plane::XY,
    }
}

pub(crate) fn datum_axis(name: &str) -> anvil_math::DVec3 {
    match name {
        "X" => anvil_math::DVec3::X,
        "Y" => anvil_math::DVec3::Y,
        _ => anvil_math::DVec3::Z,
    }
}
