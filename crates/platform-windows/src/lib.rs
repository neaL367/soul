//! Windows 11 platform integration, DPI calculations, and DPAPI encryption.

#![allow(unsafe_code)]

pub mod a11y;
pub mod crypto;
pub mod dpapi;
pub mod dpi;
pub mod theme;
pub mod window;

pub use a11y::{UiaBridge, UiaControlType, UiaElement};
pub use crypto::CryptoRandom;
pub use dpapi::Dpapi;
pub use dpi::{BASELINE_DPI, DpiScale, MonitorBounds};
pub use theme::{SystemTheme, query_system_theme};
pub use window::{BackdropType, query_window_dpi, set_immersive_dark_mode, set_system_backdrop};
