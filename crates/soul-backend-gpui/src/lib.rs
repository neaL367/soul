//! Concrete `SoulBackend` implementation against `GPUI`.
//!
//! This is the only crate in the workspace permitted to depend on `gpui`.

pub mod backend;
pub mod layout;
mod state;
mod toolbar;
mod view;

pub use backend::{GpuiSoulBackend, SoulBackendHandle};
pub use layout::{CHROME_HEIGHT, TAB_STRIP_HEIGHT, TOOLBAR_HEIGHT, page_coordinate};
