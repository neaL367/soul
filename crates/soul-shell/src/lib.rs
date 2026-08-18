//! Browser application library: end-to-end navigation and rendering pipeline driver.
//!
//! This crate's library target exposes the wired engine path — live URL fetch through
//! the networking stack (with CORS/mixed-content enforcement), navigation state
//! machine transitions, and the HTML → CSS → layout → paint → raster pipeline —
//! so it can be exercised by integration tests and the `soul-shell` binary.

pub mod crash_reporter;
pub mod diagnostics;
pub mod engine;
pub(crate) mod hit_testing;
pub mod local_page;
pub mod navigation_driver;
pub(crate) mod script_execution;
pub mod updater;

pub use crash_reporter::CrashReport;
pub use updater::{UpdateChannel, UpdateManifest, check_for_update, is_newer_version};
