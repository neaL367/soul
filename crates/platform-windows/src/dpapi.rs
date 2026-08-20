//! Windows Data Protection API (DPAPI) for user-level credential and cookie encryption.

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData,
};

/// High-level wrapper for Windows DPAPI encryption and decryption.
pub struct Dpapi;

impl Dpapi {
    /// Encrypts plaintext bytes using current Windows user credentials.
    ///
    /// # Errors
    ///
    /// Returns an error string if `CryptProtectData` fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn protect(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr().cast_mut(),
        };

        let mut output_blob = CRYPT_INTEGER_BLOB::default();

        // SAFETY: `input_blob` borrows `plaintext`, which outlives the call.
        // `output_blob` is an initialized out-parameter. The `windows` crate
        // wraps the Win32 return code, so success means the call returned
        // `ERROR_SUCCESS` and `output_blob` describes a valid LocalAlloc
        // buffer owned by the caller.
        unsafe {
            CryptProtectData(
                &raw const input_blob,
                None,
                None,
                None,
                None,
                0,
                &raw mut output_blob,
            )
            .map_err(|e| format!("DPAPI CryptProtectData failed: {e}"))?;
        }

        // SAFETY: after a successful `CryptProtectData`, `output_blob.pbData`
        // points to `cbData` valid bytes in a LocalAlloc buffer that only this
        // function may free. The slice is read into an owned `Vec` before
        // `LocalFree` releases the buffer exactly once.
        let result = unsafe {
            let slice = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
            let vec = slice.to_vec();
            let _ = LocalFree(HLOCAL(output_blob.pbData.cast()));
            vec
        };

        Ok(result)
    }

    /// Decrypts ciphertext bytes encrypted by the current Windows user.
    ///
    /// # Errors
    ///
    /// Returns an error string if `CryptUnprotectData` fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr().cast_mut(),
        };

        let mut output_blob = CRYPT_INTEGER_BLOB::default();

        // SAFETY: `input_blob` borrows `ciphertext`, which outlives the call.
        // `output_blob` is an initialized out-parameter and the Win32 result is
        // checked, so on success it describes a valid LocalAlloc buffer owned
        // by the caller.
        unsafe {
            CryptUnprotectData(
                &raw const input_blob,
                None,
                None,
                None,
                None,
                0,
                &raw mut output_blob,
            )
            .map_err(|e| format!("DPAPI CryptUnprotectData failed: {e}"))?;
        }

        // SAFETY: after a successful `CryptUnprotectData`, `output_blob.pbData`
        // points to `cbData` valid bytes in a LocalAlloc buffer freed exactly
        // once by `LocalFree` after the owned copy is taken.
        let result = unsafe {
            let slice = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
            let vec = slice.to_vec();
            let _ = LocalFree(HLOCAL(output_blob.pbData.cast()));
            vec
        };

        Ok(result)
    }
}
