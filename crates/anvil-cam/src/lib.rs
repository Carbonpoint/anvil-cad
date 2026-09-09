//! CAM.
//!
//! Architecture (see `docs/research/04-cam-simulation-and-formats.md`):
//! * An **Operation** turns geometry plus a **Tool** into a **Toolpath**.
//! * A Toolpath is a neutral list of moves. It knows nothing about any
//!   controller dialect.
//! * A **Post** (post-processor) turns a Toolpath into G-code text for one
//!   controller family.
//!
//! This first version has one operation, 2.5D contour, and one post,
//! `GenericGcode`, which writes plain RS-274 that LinuxCNC and GRBL accept.

pub mod ops;
pub mod post;
pub mod toolpath;

pub use post::{GenericGcode, Post};
pub use toolpath::{Move, Tool, Toolpath};
