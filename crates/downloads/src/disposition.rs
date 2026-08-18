//! RFC 6266 `Content-Disposition` header parser and safe filename resolution.

use std::path::{Path, PathBuf};

/// Parses a safe filename from an RFC 6266 `Content-Disposition` header value.
///
/// Supports both standard `filename="name.ext"` and extended `filename*=UTF-8''''name.ext`.
/// Strips directory traversal sequences (`..`, `/`, `\`) and illegal Windows filename characters.
#[must_use]
pub fn parse_content_disposition_filename(header: &str) -> Option<String> {
    let mut raw_name = None;

    for part in header.split(';') {
        let trimmed = part.trim();
        // Priority 1: RFC 5987 / RFC 6266 extended filename*
        if let Some(val) = trimmed.strip_prefix("filename*=") {
            let val = val.trim_matches('"');
            if let Some(rest) = val
                .strip_prefix("UTF-8''")
                .or_else(|| val.strip_prefix("utf-8''"))
                && let Ok(decoded) = urlencoding_decode(rest)
            {
                raw_name = Some(decoded);
                break;
            }
        }
        // Priority 2: Standard filename parameter
        if raw_name.is_none()
            && let Some(val) = trimmed.strip_prefix("filename=")
        {
            let val = val.trim_matches('"').trim_matches('\'');
            raw_name = Some(val.to_string());
        }
    }

    raw_name
        .map(|name| sanitize_filename(&name))
        .filter(|s| !s.is_empty())
}

/// Sanitizes a filename by stripping path separators, `..`, control characters,
/// and Windows reserved characters (`< > : " / \ | ? *`).
#[must_use]
pub fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .map_or("", |os| os.to_str().unwrap_or(""));

    let sanitized: String = base
        .chars()
        .filter(|&c| {
            !c.is_control()
                && c != '<'
                && c != '>'
                && c != ':'
                && c != '"'
                && c != '/'
                && c != '\\'
                && c != '|'
                && c != '?'
                && c != '*'
        })
        .collect();

    let trimmed = sanitized.trim().trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Finds an available (non-colliding) file path in `dir` by appending `(1)`, `(2)`, etc.
#[must_use]
pub fn find_available_path(dir: &Path, file_name: &str) -> PathBuf {
    let sanitized = sanitize_filename(file_name);
    let target = dir.join(&sanitized);
    if !target.exists() {
        return target;
    }

    let p = Path::new(&sanitized);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

    for count in 1..10000 {
        let new_name = if ext.is_empty() {
            format!("{stem} ({count})")
        } else {
            format!("{stem} ({count}).{ext}")
        };
        let candidate = dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    target
}

/// Minimal percent-decoding helper for RFC 5987 / RFC 6266 `filename*`.
fn urlencoding_decode(s: &str) -> Result<String, ()> {
    let mut bytes = Vec::new();
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().ok_or(())?;
            let h2 = chars.next().ok_or(())?;
            let val = hex_to_u8(h1, h2)?;
            bytes.push(val);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).map_err(|_| ())
}

const fn hex_to_u8(h1: u8, h2: u8) -> Result<u8, ()> {
    let d1 = match h1 {
        b'0'..=b'9' => h1 - b'0',
        b'a'..=b'f' => h1 - b'a' + 10,
        b'A'..=b'F' => h1 - b'A' + 10,
        _ => return Err(()),
    };
    let d2 = match h2 {
        b'0'..=b'9' => h2 - b'0',
        b'a'..=b'f' => h2 - b'a' + 10,
        b'A'..=b'F' => h2 - b'A' + 10,
        _ => return Err(()),
    };
    Ok((d1 << 4) | d2)
}
