//! Integration tests for Brotli and Zstandard content decoding.

use networking::decompression::decompress_payload;
use std::io::Write;

#[test]
fn test_brotli_decompression_roundtrip() {
    let original = b"Hello Soul Web Engine from Brotli compressed payload!";
    let mut compressed = Vec::new();
    {
        let mut compressor = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
        compressor.write_all(original).expect("compress");
    }

    let decompressed = decompress_payload(&compressed, Some("br")).expect("decompress br");
    assert_eq!(decompressed, original);
}

#[test]
fn test_zstd_decompression_roundtrip() {
    let original = b"Hello Soul Web Engine from Zstandard compressed payload!";
    let compressed = zstd::encode_all(&original[..], 3).expect("compress zstd");

    let decompressed = decompress_payload(&compressed, Some("zstd")).expect("decompress zstd");
    assert_eq!(decompressed, original);
}
