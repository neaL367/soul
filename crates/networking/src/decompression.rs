//! RFC 9110 HTTP payload decompression (`gzip`, `deflate`).

use crate::error::NetworkError;
use flate2::read::{GzDecoder, ZlibDecoder};
use std::io::Read;

/// Decompresses raw HTTP payload bytes according to the `Content-Encoding` header value.
///
/// # Errors
///
/// Returns `NetworkError::DecompressionFailed` if decoding fails.
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
            let mut decoder = GzDecoder::new(raw_bytes);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| NetworkError::DecompressionFailed(format!("gzip error: {e}")))?;
            Ok(decompressed)
        }
        "deflate" => {
            let mut decoder = ZlibDecoder::new(raw_bytes);
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() {
                return Ok(decompressed);
            }
            // Some non-compliant servers send raw deflate streams without zlib headers
            let mut raw_decoder = flate2::read::DeflateDecoder::new(raw_bytes);
            let mut raw_decompressed = Vec::new();
            raw_decoder
                .read_to_end(&mut raw_decompressed)
                .map_err(|e| NetworkError::DecompressionFailed(format!("deflate error: {e}")))?;
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
