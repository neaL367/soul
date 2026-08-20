//! Restricted security token creation and privilege stripping for Windows sandboxes.

use crate::error::SandboxError;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::{
    CREATE_RESTRICTED_TOKEN_FLAGS, CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, DuplicateTokenEx,
    LUA_TOKEN, SID_AND_ATTRIBUTES, SecurityImpersonation, SetTokenInformation,
    TOKEN_ADJUST_DEFAULT, TOKEN_ALL_ACCESS, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel, TokenPrimary,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Win32 `SE_GROUP_INTEGRITY` SID attribute flag for mandatory integrity labels.
const SE_GROUP_INTEGRITY: u32 = 0x0000_0020;

/// Wrapper representing a restricted Windows security token with stripped privileges.
#[derive(Debug)]
pub struct RestrictedToken {
    handle: HANDLE,
}

// SAFETY: `HANDLE` wraps a Win32 kernel handle with no per-thread state;
// token handles are usable from any thread and `Drop` closes them once via
// `CloseHandle`.
unsafe impl Send for RestrictedToken {}
unsafe impl Sync for RestrictedToken {}

impl RestrictedToken {
    /// Creates a restricted primary security token based on the current process's primary token,
    /// disabling admin/write privileges, applying LUA (Limited User Account) constraints,
    /// and making it usable with `CreateProcessAsUserW`.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if token query, creation, or duplication fails.
    pub fn create_for_renderer() -> Result<Self, SandboxError> {
        Self::create_with_flags(DISABLE_MAX_PRIVILEGE | LUA_TOKEN, true)
    }

    /// Creates a restricted security token with custom restriction flags and optional low integrity.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if Win32 token operations fail.
    pub fn create_with_flags(
        flags: CREATE_RESTRICTED_TOKEN_FLAGS,
        low_integrity: bool,
    ) -> Result<Self, SandboxError> {
        let mut process_token = HANDLE::default();

        // SAFETY: `process_token` is an initialized out-parameter; the pseudo
        // handle from `GetCurrentProcess` is valid for the duration of the
        // call and the result is checked with `?`.
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_ADJUST_DEFAULT,
                &raw mut process_token,
            )?;
        }

        let mut restricted_token = HANDLE::default();
        // SAFETY: `restricted_token` is an initialized out-parameter; the
        // empty input slices mean no SIDs or privileges are excluded, which
        // Win32 accepts. The call result is checked with `?`.
        let res = unsafe {
            CreateRestrictedToken(
                process_token,
                flags,
                Some(&[]),
                Some(&[]),
                Some(&[]),
                &raw mut restricted_token,
            )
        };

        // SAFETY: `process_token` was opened by the successful `OpenProcessToken`
        // above and is closed exactly once here; it is not used afterwards.
        unsafe {
            let _ = CloseHandle(process_token);
        }

        res?;

        // Duplicate the token to ensure it is a Primary Token that can be passed to CreateProcessAsUserW.
        let mut primary_token = HANDLE::default();
        let dup_res = unsafe {
            DuplicateTokenEx(
                restricted_token,
                TOKEN_ALL_ACCESS,
                None,
                SecurityImpersonation,
                TokenPrimary,
                &raw mut primary_token,
            )
        };

        unsafe {
            let _ = CloseHandle(restricted_token);
        }

        dup_res?;

        let token = Self {
            handle: primary_token,
        };

        if low_integrity {
            token.set_low_integrity()?;
        }

        Ok(token)
    }

    /// Sets the mandatory integrity level of the token to Low (`S-1-16-4096`).
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if converting the SID or setting token information fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_low_integrity(&self) -> Result<(), SandboxError> {
        let mut p_sid = windows::Win32::Security::PSID::default();
        // S-1-16-4096 corresponds to SECURITY_MANDATORY_LOW_RID
        unsafe {
            ConvertStringSidToSidW(windows::core::w!("S-1-16-4096"), &raw mut p_sid)?;
        }

        let tml = TOKEN_MANDATORY_LABEL {
            Label: SID_AND_ATTRIBUTES {
                Sid: p_sid,
                Attributes: SE_GROUP_INTEGRITY,
            },
        };

        let res = unsafe {
            SetTokenInformation(
                self.handle,
                TokenIntegrityLevel,
                std::ptr::from_ref(&tml).cast(),
                size_of::<TOKEN_MANDATORY_LABEL>() as u32,
            )
        };

        unsafe {
            let _ = LocalFree(HLOCAL(p_sid.0));
        }

        res?;
        Ok(())
    }

    /// Returns the raw Win32 token `HANDLE` for use with Win32 APIs.
    ///
    /// The handle is **borrowed** from this `RestrictedToken`: callers must
    /// not close it (it is closed by `Drop`), and must not use it after the
    /// `RestrictedToken` is dropped.
    #[must_use]
    pub const fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for RestrictedToken {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            // SAFETY: the handle is valid, unclosed, and `CloseHandle` runs
            // exactly once because `Drop` runs once.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
