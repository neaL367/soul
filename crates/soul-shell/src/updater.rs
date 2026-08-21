//! Browser auto-updater checking, version comparison, signature verification, and atomic staging.

use signature::Verifier as _;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors arising during update manifest verification, download staging, or installation.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// SHA256 checksum mismatch on downloaded binary.
    #[error("Checksum mismatch: expected {expected}, computed {computed}")]
    ChecksumMismatch {
        /// Expected SHA256 hex string.
        expected: String,
        /// Computed SHA256 hex string.
        computed: String,
    },

    /// Manifest digital signature validation failed.
    #[error("Invalid manifest signature for version {version}")]
    InvalidSignature {
        /// Target version whose signature failed.
        version: String,
    },

    /// Minimum OS version constraint not satisfied.
    #[error("Incompatible OS: current {current}, required {required}")]
    IncompatibleOs {
        /// Host operating system version.
        current: String,
        /// Minimum version required.
        required: String,
    },

    /// File I/O failure during update staging or atomic replacement.
    #[error("Update I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Release channels for browser updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    /// Stable release channel.
    Stable,
    /// Beta pre-release channel.
    Beta,
    /// Nightly development build channel.
    Nightly,
}

/// Metadata describing an available browser update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateManifest {
    /// Version string of the update (e.g. "0.2.0").
    pub version: String,
    /// Release channel.
    pub channel: UpdateChannel,
    /// Release notes or changelog summary.
    pub release_notes: String,
    /// Binary download URL.
    pub download_url: String,
    /// SHA256 checksum for binary integrity verification.
    pub sha256: String,
    /// Digital signature over manifest metadata.
    pub signature: String,
    /// Optional minimum Windows OS version required (e.g. "10.0.22000").
    pub min_os_version: Option<String>,
}

/// Checks whether an update is available by comparing semver-like version strings.
#[must_use]
pub fn is_newer_version(current_ver: &str, candidate_ver: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };

    let current = parse_parts(current_ver);
    let candidate = parse_parts(candidate_ver);

    for (c, cand) in current.iter().zip(candidate.iter()) {
        if cand > c {
            return true;
        }
        if cand < c {
            return false;
        }
    }

    candidate.len() > current.len()
}

/// Evaluates whether an update manifest warrants an update notification.
#[must_use]
pub fn check_for_update(
    current_version: &str,
    manifest: &UpdateManifest,
) -> Option<UpdateManifest> {
    if is_newer_version(current_version, &manifest.version) {
        Some(manifest.clone())
    } else {
        None
    }
}

/// Computes the standard NIST SHA-256 digest of input bytes, returned as a lowercase hex string.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines
)]
pub fn compute_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;

    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];

    let bit_len = (bytes.len() as u64) * 8;
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let start = i * 4;
            w[i] = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for val in h {
        let _ = write!(out, "{val:08x}");
    }
    out
}

/// Verifies whether the SHA-256 digest of `payload_bytes` matches `expected_sha256`.
#[must_use]
pub fn verify_payload_checksum(payload_bytes: &[u8], expected_sha256: &str) -> bool {
    let computed = compute_sha256(payload_bytes);
    computed.eq_ignore_ascii_case(expected_sha256.trim())
}

/// Verifies manifest digital signature against a trusted Ed25519 public key.
///
/// Accepts both the new asymmetric Ed25519 base64 signature and the legacy
/// symmetric token-derived SHA-256 hex for backwards compatibility. The Ed25519
/// message is `"{version}:{sha256}:{download_url}"` signed with the private
/// key; `public_key_token` is base64-encoded 32-byte Ed25519 verifying key.
///
/// # Errors
///
/// Returns `UpdateError::InvalidSignature` if verification fails.
pub fn verify_manifest_signature(
    manifest: &UpdateManifest,
    public_key_token: &str,
) -> Result<bool, UpdateError> {
    if manifest.signature.is_empty() || public_key_token.is_empty() {
        return Err(UpdateError::InvalidSignature {
            version: manifest.version.clone(),
        });
    }

    // Attempt Ed25519 verification first (base64 32-byte pubkey, 64-byte sig).
    if let Ok(verified) = verify_ed25519_manifest(manifest, public_key_token)
        && verified
    {
        return Ok(true);
    }

    // Fallback: legacy symmetric token-derived SHA-256 hex (backwards compat).
    let sign_payload = format!(
        "{}:{}:{}:{}",
        manifest.version, manifest.sha256, manifest.download_url, public_key_token
    );
    let expected_sig = compute_sha256(sign_payload.as_bytes());

    if expected_sig.eq_ignore_ascii_case(&manifest.signature) {
        Ok(true)
    } else {
        Err(UpdateError::InvalidSignature {
            version: manifest.version.clone(),
        })
    }
}

fn verify_ed25519_manifest(
    manifest: &UpdateManifest,
    public_key_base64: &str,
) -> Result<bool, UpdateError> {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use ed25519_dalek::{Signature, VerifyingKey};

    let pubkey_bytes =
        BASE64
            .decode(public_key_base64.trim())
            .map_err(|_| UpdateError::InvalidSignature {
                version: manifest.version.clone(),
            })?;
    let sig_bytes =
        BASE64
            .decode(manifest.signature.trim())
            .map_err(|_| UpdateError::InvalidSignature {
                version: manifest.version.clone(),
            })?;

    if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 {
        return Err(UpdateError::InvalidSignature {
            version: manifest.version.clone(),
        });
    }

    let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes.try_into().map_err(|_| {
        UpdateError::InvalidSignature {
            version: manifest.version.clone(),
        }
    })?)
    .map_err(|_| UpdateError::InvalidSignature {
        version: manifest.version.clone(),
    })?;

    let signature = Signature::from_bytes(&sig_bytes.try_into().map_err(|_| {
        UpdateError::InvalidSignature {
            version: manifest.version.clone(),
        }
    })?);

    // Message is version:sha256:download_url (without token)
    let message = format!(
        "{}:{}:{}",
        manifest.version, manifest.sha256, manifest.download_url
    );

    Ok(verifying_key.verify(message.as_bytes(), &signature).is_ok())
}

/// Stages an update payload to disk, verifying its SHA-256 hash before final renaming.
///
/// # Errors
///
/// Returns `UpdateError` on hash mismatch or I/O failure.
pub fn stage_update_payload(
    staging_dir: &Path,
    payload: &[u8],
    filename: &str,
    expected_sha256: &str,
) -> Result<PathBuf, UpdateError> {
    let computed = compute_sha256(payload);
    if !computed.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected_sha256.to_string(),
            computed,
        });
    }

    std::fs::create_dir_all(staging_dir)?;
    let temp_file = staging_dir.join(format!("{filename}.staging"));
    let target_file = staging_dir.join(filename);

    std::fs::write(&temp_file, payload)?;
    std::fs::rename(&temp_file, &target_file)?;

    Ok(target_file)
}

/// Atomically replaces the target binary with the staged update file with backup preservation.
///
/// # Errors
///
/// Returns `UpdateError` if replacement fails.
pub fn apply_staged_update(
    staged_file: &Path,
    target_binary_path: &Path,
) -> Result<(), UpdateError> {
    if !staged_file.exists() {
        return Err(UpdateError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Staged update file not found: {}", staged_file.display()),
        )));
    }

    if let Some(parent) = target_binary_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let backup_path = target_binary_path.with_extension("bak");
    if target_binary_path.exists() {
        let _ = std::fs::remove_file(&backup_path);
        std::fs::rename(target_binary_path, &backup_path)?;
    }

    if let Err(e) = std::fs::rename(staged_file, target_binary_path) {
        // Rollback from backup if rename failed
        if backup_path.exists() {
            let _ = std::fs::rename(&backup_path, target_binary_path);
        }
        return Err(UpdateError::Io(e));
    }

    // Clean up backup on success
    let _ = std::fs::remove_file(backup_path);
    Ok(())
}
