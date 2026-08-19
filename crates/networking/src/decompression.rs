//! RFC 9110 HTTP payload decompression (`gzip`, `deflate`).

use crate::error::NetworkError;
use flate2::read::{GzDecoder, ZlibDecoder};
use std::io::Read;

/// Ceiling on decompressed payload size.
///
/// Guards against decompression bombs: a small compressed response must not
/// expand into an unbounded allocation. The transfer-side cap in the HTTP
/// client uses the same constant.
pub const MAX_DECOMPRESSED_BYTES: usize = 64 * 1024 * 1024;

/// Decompresses raw HTTP payload bytes according to the `Content-Encoding` header value.
///
/// Decompressed output is capped at [`MAX_DECOMPRESSED_BYTES`]; larger output
/// fails with `NetworkError::DecompressionFailed`.
///
/// # Errors
///
/// Returns `NetworkError::DecompressionFailed` if decoding fails or the
/// decompressed payload exceeds the size cap.
pub fn decompress_payload(
    raw_bytes: &[u8],
    content_encoding: Option<&str>,
) -> Result<Vec<u8>, NetworkError> {
    let Some(encoding) = content_encoding else {
        return Ok(raw_bytes.to_vec());
    };

    let enc = encoding.trim().to_ascii_lowercase();
    match enc.as_str() {
        "gzip" | "x-gzip" => {
            let mut decoder = GzDecoder::new(raw_bytes).take(MAX_DECOMPRESSED_BYTES as u64 + 1);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| NetworkError::DecompressionFailed(format!("gzip error: {e}")))?;
            check_size_limit(&decompressed)?;
            Ok(decompressed)
        }
        "deflate" => {
            let mut decoder = ZlibDecoder::new(raw_bytes).take(MAX_DECOMPRESSED_BYTES as u64 + 1);
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() {
                check_size_limit(&decompressed)?;
                return Ok(decompressed);
            }
            // Some non-compliant servers send raw deflate streams without zlib headers
            let mut raw_decoder = flate2::read::DeflateDecoder::new(raw_bytes)
                .take(MAX_DECOMPRESSED_BYTES as u64 + 1);
            let mut raw_decompressed = Vec::new();
            raw_decoder
                .read_to_end(&mut raw_decompressed)
                .map_err(|e| NetworkError::DecompressionFailed(format!("deflate error: {e}")))?;
            check_size_limit(&raw_decompressed)?;
            Ok(raw_decompressed)
        }
        "identity" | "" => Ok(raw_bytes.to_vec()),
        other => {
            tracing::warn!(
                encoding = other,
                "Unsupported Content-Encoding, keeping raw bytes"
            );
            Ok(raw_bytes.to_vec())
        }
    }
}

/// Rejects payloads that hit or exceeded the decompressed size ceiling.
fn check_size_limit(decompressed: &[u8]) -> Result<(), NetworkError> {
    if decompressed.len() > MAX_DECOMPRESSED_BYTES {
        Err(NetworkError::DecompressionFailed(format!(
            "decompressed size exceeds limit of {MAX_DECOMPRESSED_BYTES} bytes"
        )))
    } else {
        Ok(())
    }
}
