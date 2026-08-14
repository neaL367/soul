//! Restricted security token creation and privilege stripping for Windows sandboxes.

use crate::error::SandboxError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, LUA_TOKEN, TOKEN_ALL_ACCESS, TOKEN_DUPLICATE,
    TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Wrapper representing a restricted Windows security token with stripped privileges.
#[derive(Debug)]
pub struct RestrictedToken {
    handle: HANDLE,
}

unsafe impl Send for RestrictedToken {}
unsafe impl Sync for RestrictedToken {}

impl RestrictedToken {
    /// Creates a restricted security token based on the current process's primary token,
    /// disabling admin/write privileges and applying LUA (Limited User Account) constraints.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if token query or creation fails.
    pub fn create_for_renderer() -> Result<Self, SandboxError> {
        let mut process_token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ALL_ACCESS,
                &raw mut process_token,
            )?;
        }

        let mut restricted_token = HANDLE::default();
        let res = unsafe {
            CreateRestrictedToken(
                process_token,
                DISABLE_MAX_PRIVILEGE | LUA_TOKEN,
                Some(&[]),
                Some(&[]),
                Some(&[]),
                &raw mut restricted_token,
            )
        };

        unsafe {
            let _ = CloseHandle(process_token);
        }

        res?;
        Ok(Self {
            handle: restricted_token,
        })
    }

    /// Returns the raw Win32 token `HANDLE`.
    #[must_use]
    pub const fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for RestrictedToken {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
