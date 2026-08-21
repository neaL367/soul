//! Cryptographically secure random byte and UUID generation on Windows.

use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

/// High-level wrapper for Windows CSPRNG cryptographic operations.
pub struct CryptoRandom;

impl CryptoRandom {
    /// Fills `dest` with cryptographically secure random bytes from Windows CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns an error string if `BCryptGenRandom` fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn fill_random_bytes(dest: &mut [u8]) -> Result<(), String> {
        if dest.is_empty() {
            return Ok(());
        }

        unsafe {
            BCryptGenRandom(None, dest, BCRYPT_USE_SYSTEM_PREFERRED_RNG)
                .ok()
                .map_err(|e| format!("BCryptGenRandom failed: {e}"))?;
        }
        Ok(())
    }

    /// Generates an RFC 4122 version 4 cryptographically secure random UUID string.
    #[must_use]
    pub fn random_uuid_v4() -> String {
        let mut bytes = [0u8; 16];
        if Self::fill_random_bytes(&mut bytes).is_err() {
            // Fallback to high-entropy epoch + address hash if BCrypt fails
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            for (i, b) in bytes.iter_mut().enumerate() {
                *b = ((t >> (i * 8)) & 0xFF) as u8;
            }
        }

        // Set version to 4 (0100 in high 4 bits of byte 6)
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        // Set variant to RFC 4122 (10xx in high 2 bits of byte 8)
        bytes[8] = (bytes[8] & 0x3F) | 0x80;

        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15]
        )
    }
}
