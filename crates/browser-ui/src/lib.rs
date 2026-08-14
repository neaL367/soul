//! `ChromeBackend` trait and backend-agnostic view logic for tabs, omnibox, toolbar, menus, settings, and downloads UI.

pub mod backend;
pub mod event;

pub use backend::{ChromeBackend, ChromeConfig, ChromeError, ViewportFrame, WindowId, WindowSpec};
pub use event::ChromeEvent;
