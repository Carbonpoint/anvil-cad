//! Built-in features. Each file registers itself with `inventory::submit!`.
//!
//! To add one, copy `template.rs`, rename, and add a `pub mod` line here.

pub mod extrude;
pub mod fillet;
pub mod revolve;
pub mod sketch;
pub mod template;
