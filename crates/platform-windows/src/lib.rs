//! Windows 11 platform integration, DPI calculations, and DPAPI encryption.

#![allow(unsafe_code)]

pub mod dpapi;
pub mod dpi;
pub mod theme;

pub use dpapi::Dpapi;
pub use dpi::{BASELINE_DPI, DpiScale, MonitorBounds};
pub use theme::{SystemTheme, query_system_theme};
