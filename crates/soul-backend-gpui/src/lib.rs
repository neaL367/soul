//! Concrete `SoulBackend` implementation against `GPUI`.
//!
//! This is the only crate in the workspace permitted to depend on `gpui`.

pub mod backend;

pub use backend::GpuiSoulBackend;
