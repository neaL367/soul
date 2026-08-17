//! Concrete `SoulBackend` implementation against `GPUI`.
//!
//! This is the only crate in the workspace permitted to depend on `gpui`.

pub mod backend;
mod state;
mod view;

pub use backend::{GpuiSoulBackend, SoulBackendHandle};
