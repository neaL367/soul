//! URL encoding and percent-decoding helper algorithms.

/// Decodes application/x-www-form-urlencoded and percent-encoded string.
#[must_use]
pub fn urlencoding_decode(s: &str) -> String {
    let replaced = s.replace('+', " ");
    let mut out = Vec::new();
    let mut bytes = replaced.as_bytes().iter();
    while let Some(&b) = bytes.next() {
        if b == b'%' {
            if let (Some(&h1), Some(&h2)) = (bytes.next(), bytes.next()) {
                if let (Some(v1), Some(v2)) = (hex_val(h1), hex_val(h2)) {
                    out.push((v1 << 4) | v2);
                    continue;
                }
                out.push(b'%');
                out.push(h1);
                out.push(h2);
            } else {
                out.push(b'%');
            }
        } else {
            out.push(b);
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

use std::fmt::Write;

/// Encodes a query component into application/x-www-form-urlencoded format.
#[must_use]
pub fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            byte => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

const fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
