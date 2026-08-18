//! Windows 11 system theme detection (`prefers-color-scheme`).
//!
//! Queries the Windows registry key
//! `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW,
    RegQueryValueExW,
};
use windows::core::w;

/// Active Windows system color theme mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemTheme {
    /// Windows light mode (`AppsUseLightTheme` = 1).
    #[default]
    Light,
    /// Windows dark mode (`AppsUseLightTheme` = 0).
    Dark,
}

impl SystemTheme {
    /// Returns `true` if the system theme is dark.
    #[must_use]
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// Queries the current Windows system theme from the registry.
///
/// Returns `SystemTheme::Light` by default if the key is absent or querying fails.
#[must_use]
pub fn query_system_theme() -> SystemTheme {
    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

        let open_res = RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &raw mut hkey);

        if open_res.is_err() {
            return SystemTheme::Light;
        }

        let mut data: u32 = 1;
        let mut data_size: u32 = 4;
        let mut val_type: REG_VALUE_TYPE = REG_DWORD;
        let val_name = w!("AppsUseLightTheme");

        let query_res = RegQueryValueExW(
            hkey,
            val_name,
            None,
            Some(&raw mut val_type),
            Some((&raw mut data).cast::<u8>()),
            Some(&raw mut data_size),
        );

        let _ = RegCloseKey(hkey);

        if query_res.is_ok() && data == 0 {
            SystemTheme::Dark
        } else {
            SystemTheme::Light
        }
    }
}
