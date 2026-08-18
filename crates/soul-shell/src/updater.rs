//! Browser auto-updater checking, version comparison, and release channel management.

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
    /// SHA256 checksum for verification.
    pub sha256: String,
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
pub fn check_for_update(current_version: &str, manifest: &UpdateManifest) -> Option<UpdateManifest> {
    if is_newer_version(current_version, &manifest.version) {
        Some(manifest.clone())
    } else {
        None
    }
}
