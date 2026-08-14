//! Windows 11 platform integration, DPI calculations, and DPAPI encryption.

#![allow(unsafe_code)]

pub mod dpapi;
pub mod dpi;

pub use dpapi::Dpapi;
pub use dpi::{BASELINE_DPI, DpiScale, MonitorBounds};
