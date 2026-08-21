//! Windows 11 window visual effects, Per-Monitor v2 DPI queries, and DWM attributes.

use crate::dpi::DpiScale;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DwmSetWindowAttribute,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;

/// Windows 11 DWM backdrop material types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackdropType {
    /// Default system background.
    #[default]
    Auto,
    /// Disable backdrop effects.
    None,
    /// Windows 11 Mica backdrop material.
    Mica,
    /// Windows 11 Acrylic translucent material.
    Acrylic,
    /// Windows 11 Mica Alt (tabbed) material.
    MicaAlt,
}

/// Queries the effective DPI and scaling factor for a specific window handle.
#[must_use]
pub fn query_window_dpi(hwnd: isize) -> DpiScale {
    let raw_hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let dpi = unsafe { GetDpiForWindow(raw_hwnd) };
    if dpi == 0 {
        DpiScale::default()
    } else {
        DpiScale::from_dpi_value(dpi)
    }
}

/// Applies Windows 11 immersive dark mode title bar styling to a window.
///
/// Returns `true` if DWM accepted the attribute change.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn set_immersive_dark_mode(hwnd: isize, enable: bool) -> bool {
    let raw_hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let val = i32::from(enable);
    unsafe {
        DwmSetWindowAttribute(
            raw_hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            std::ptr::from_ref(&val).cast(),
            std::mem::size_of::<i32>() as u32,
        )
        .is_ok()
    }
}

/// Configures the Windows 11 DWM backdrop effect (Mica, Acrylic, or Mica Alt).
///
/// Returns `true` if DWM accepted the backdrop attribute.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn set_system_backdrop(hwnd: isize, backdrop: BackdropType) -> bool {
    let raw_hwnd = HWND(hwnd as *mut std::ffi::c_void);
    let dwm_backdrop = match backdrop {
        BackdropType::Auto => DWM_SYSTEMBACKDROP_TYPE(0),
        BackdropType::None => DWM_SYSTEMBACKDROP_TYPE(1),
        BackdropType::Mica => DWM_SYSTEMBACKDROP_TYPE(2),
        BackdropType::Acrylic => DWM_SYSTEMBACKDROP_TYPE(3),
        BackdropType::MicaAlt => DWM_SYSTEMBACKDROP_TYPE(4),
    };

    unsafe {
        DwmSetWindowAttribute(
            raw_hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            std::ptr::from_ref(&dwm_backdrop).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
        .is_ok()
    }
}
