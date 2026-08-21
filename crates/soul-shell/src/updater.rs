//! Browser auto-updater checking, version comparison, signature verification, and atomic staging.

pub mod crypto;

pub use crypto::{compute_sha256, verify_manifest_signature, verify_payload_checksum};

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
        if let Some(ref min_os) = manifest.min_os_version {
            let current_os = "10.0.22000";
            if !is_os_compatible(current_os, min_os) {
                return None;
            }
        }
        Some(manifest.clone())
    } else {
        None
    }
}

/// Checks if current OS meets minimum required version.
#[must_use]
pub fn is_os_compatible(current_os: &str, required_min: &str) -> bool {
    let parse_parts =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };

    let current = parse_parts(current_os);
    let required = parse_parts(required_min);

    for (c, req) in current.iter().zip(required.iter()) {
        if c > req {
            return true;
        }
        if c < req {
            return false;
        }
    }

    current.len() >= required.len()
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
        if backup_path.exists() {
            let _ = std::fs::rename(&backup_path, target_binary_path);
        }
        return Err(UpdateError::Io(e));
    }

    let _ = std::fs::remove_file(backup_path);
    Ok(())
}
