//! Browser application library: end-to-end navigation and rendering pipeline driver.
//!
//! This crate's library target exposes the wired engine path — live URL fetch through
//! the networking stack (with CORS/mixed-content enforcement), navigation state
//! machine transitions, and the HTML → CSS → layout → paint → raster pipeline —
//! so it can be exercised by integration tests and the `soul-shell` binary.

pub mod diagnostics;
pub mod engine;
pub mod navigation_driver;
