//! Mote core: the domain model and the language-plugin contract.
//!
//! This crate is intentionally small and stable. Language plugins depend only on it; the
//! engine, reporters, and CLI build everything else on top.

pub mod lines;
pub mod model;
pub mod plugin;
pub mod testing;

pub use lines::LineIndex;
pub use model::*;
pub use plugin::*;
