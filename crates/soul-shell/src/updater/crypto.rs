//! Cryptographic hashing and Ed25519 signature verification for browser updates.

use super::UpdateError;
use super::UpdateManifest;
use signature::Verifier as _;
use std::fmt::Write;

/// Computes SHA-256 hex digest for binary payloads using pure-Rust bitwise operations.
#[allow(
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::cast_possible_truncation,
    clippy::needless_range_loop
)]
#[must_use]
pub fn compute_sha256(bytes: &[u8]) -> String {
    #[rustfmt::skip]
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
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
/// symmetric token-derived SHA-256 hex for backwards compatibility.
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

    if let Ok(verified) = verify_ed25519_manifest(manifest, public_key_token)
        && verified
    {
        return Ok(true);
    }

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

    let message = format!(
        "{}:{}:{}",
        manifest.version, manifest.sha256, manifest.download_url
    );

    Ok(verifying_key.verify(message.as_bytes(), &signature).is_ok())
}
