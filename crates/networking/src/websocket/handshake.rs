//! RFC 6455 WebSocket client handshake, key generation, and accept verification.

use crate::error::NetworkError;

const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Generates a random 16-byte `Sec-WebSocket-Key` encoded in standard Base64.
#[must_use]
pub fn generate_websocket_key() -> String {
    // Generate 16 pseudorandom bytes using time and monotonic counter.
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut bytes = [0u8; 16];
    let n1 = now.to_le_bytes();
    let n2 = (now.rotate_left(17)).to_be_bytes();
    bytes[..8].copy_from_slice(&n1[..8]);
    bytes[8..].copy_from_slice(&n2[..8]);
    base64_encode(&bytes)
}

/// Computes the expected `Sec-WebSocket-Accept` header value for a given key.
#[must_use]
pub fn compute_accept_key(client_key: &str) -> String {
    let mut data = Vec::with_capacity(client_key.len() + WS_GUID.len());
    data.extend_from_slice(client_key.trim().as_bytes());
    data.extend_from_slice(WS_GUID);
    let hash = sha1(&data);
    base64_encode(&hash)
}

/// Builds the HTTP/1.1 Upgrade request bytes for establishing a WebSocket session.
#[must_use]
pub fn build_handshake_request(host: &str, path: &str, ws_key: &str) -> Vec<u8> {
    let req = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {ws_key}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    req.into_bytes()
}

/// Validates the server's HTTP/1.1 101 response against the client key.
///
/// # Errors
///
/// Returns `NetworkError` if status is not 101 or `Sec-WebSocket-Accept` does not match.
pub fn validate_handshake_response(
    response_bytes: &[u8],
    client_key: &str,
) -> Result<usize, NetworkError> {
    let text = std::str::from_utf8(response_bytes).map_err(|_| {
        NetworkError::WebSocketProtocol("invalid utf-8 in handshake response".into())
    })?;

    let header_end = text
        .find("\r\n\r\n")
        .ok_or_else(|| NetworkError::WebSocketProtocol("incomplete handshake response".into()))?;

    let header_part = &text[..header_end];
    let lines: Vec<&str> = header_part.lines().collect();
    if lines.is_empty() {
        return Err(NetworkError::WebSocketProtocol("empty response".into()));
    }

    let status_line = lines[0];
    if !status_line.contains("101") {
        return Err(NetworkError::WebSocketProtocol(format!(
            "server rejected websocket upgrade: {status_line}"
        )));
    }

    let expected_accept = compute_accept_key(client_key);
    let mut found_accept = false;

    for line in &lines[1..] {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case("sec-websocket-accept")
        {
            if v.trim() != expected_accept {
                return Err(NetworkError::WebSocketProtocol(
                    "mismatched Sec-WebSocket-Accept".into(),
                ));
            }
            found_accept = true;
            break;
        }
    }

    if !found_accept {
        return Err(NetworkError::WebSocketProtocol(
            "missing Sec-WebSocket-Accept header".into(),
        ));
    }

    Ok(header_end + 4)
}

// ──────────────────────────────────────────────────────────────────────────────
// Pure Rust SHA-1 implementation (RFC 3174) for WebSocket accept verification
// ──────────────────────────────────────────────────────────────────────────────

#[allow(clippy::many_single_char_names, clippy::needless_range_loop)]
fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;

    let bit_len = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

// ──────────────────────────────────────────────────────────────────────────────
// Base64 encoding
// ──────────────────────────────────────────────────────────────────────────────

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        out.push(B64_TABLE[(b0 >> 2) as usize] as char);
        out.push(B64_TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            out.push(B64_TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(B64_TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
