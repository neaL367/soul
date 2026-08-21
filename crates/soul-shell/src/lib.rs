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

pub use crash_reporter::{BreadcrumbTracker, CrashReport, prune_old_reports};
pub use diagnostics::SystemDiagnostics;
pub use updater::{
    UpdateChannel, UpdateError, UpdateManifest, apply_staged_update, check_for_update,
    compute_sha256, is_newer_version, stage_update_payload, verify_manifest_signature,
    verify_payload_checksum,
};
