//! CAM.
//!
//! Architecture (see `docs/research/04-cam-simulation-and-formats.md`):
//! * An **Operation** turns geometry plus a **Tool** into a **Toolpath**.
//! * A Toolpath is a neutral list of moves. It knows nothing about any
//!   controller dialect.
//! * A **Post** (post-processor) turns a Toolpath into G-code text for one
//!   controller family.
//!
//! Operations: 2.5D contour, pocket (contour parallel, with islands) and
//! drilling (canned cycles). Posts: generic RS-274, LinuxCNC, GRBL, Fanuc
//! and Haas, see `post::posts`. Offsets go through `cavalier_contours`.

pub mod ops;
pub mod post;
pub mod region;
pub mod toolpath;

pub use post::{posts, GenericGcode, Post};
pub use toolpath::{Move, Tool, Toolpath};
