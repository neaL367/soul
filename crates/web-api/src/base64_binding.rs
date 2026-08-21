//! WHATWG HTML specification `btoa` and `atob` ECMAScript bindings.

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsString, JsValue};

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Registers `btoa` and `atob` globally in the JS context.
pub fn register_base64(context: &mut Context) {
    let global = context.global_object();

    let _ = global.set(
        js_string!("btoa"),
        NativeFunction::from_fn_ptr(js_btoa).to_js_function(context.realm()),
        false,
        context,
    );

    let _ = global.set(
        js_string!("atob"),
        NativeFunction::from_fn_ptr(js_atob).to_js_function(context.realm()),
        false,
        context,
    );
}

/// WHATWG `btoa()`: Encodes a binary Latin-1 string into Base64.
fn js_btoa(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let input = args.get_or_undefined(0).to_string(context)?;
    let input_str = input.to_std_string_escaped();

    let mut bytes = Vec::with_capacity(input_str.len());
    for c in input_str.chars() {
        if c as u32 > 0xFF {
            return Err(JsError::from(JsNativeError::typ().with_message(
                "InvalidCharacterError: The string contains characters outside of the Latin1 range.",
            )));
        }
        #[allow(clippy::cast_possible_truncation)]
        bytes.push(c as u8);
    }

    let encoded = encode_base64(&bytes);
    Ok(JsValue::from(JsString::from(encoded)))
}

/// WHATWG `atob()`: Decodes a Base64-encoded string into a binary Latin-1 string.
fn js_atob(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let input = args.get_or_undefined(0).to_string(context)?;
    let input_str = input.to_std_string_escaped();

    // Strip ASCII whitespace
    let cleaned: String = input_str
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C'))
        .collect();

    let decoded_bytes = decode_base64(&cleaned).map_err(|msg| {
        JsError::from(JsNativeError::typ().with_message(format!("InvalidCharacterError: {msg}")))
    })?;

    let result_str: String = decoded_bytes.into_iter().map(|b| b as char).collect();
    Ok(JsValue::from(JsString::from(result_str)))
}

/// Encodes bytes into standard Base64 string with padding.
#[must_use]
pub fn encode_base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;

    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);

        out.push(BASE64_ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(BASE64_ALPHABET[((n >> 12) & 63) as usize] as char);

        if i + 1 < data.len() {
            out.push(BASE64_ALPHABET[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }

        if i + 2 < data.len() {
            out.push(BASE64_ALPHABET[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }

    out
}

/// Decodes standard Base64 string into bytes.
///
/// # Errors
/// Returns error description if padding or characters are invalid.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, &'static str> {
    let input = input.trim_matches(|c: char| c.is_ascii_whitespace());
    if input.is_empty() {
        return Ok(Vec::new());
    }

    if !input.len().is_multiple_of(4) {
        return Err("Base64 string length must be a multiple of 4");
    }

    let decode_char = |c: u8| -> Result<u32, &'static str> {
        match c {
            b'A'..=b'Z' => Ok(u32::from(c - b'A')),
            b'a'..=b'z' => Ok(u32::from(c - b'a' + 26)),
            b'0'..=b'9' => Ok(u32::from(c - b'0' + 52)),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("Invalid base64 character"),
        }
    };

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(input.len() / 4 * 3);

    for chunk in bytes.chunks(4) {
        let c0 = decode_char(chunk[0])?;
        let c1 = decode_char(chunk[1])?;

        let (c2, p2) = if chunk[2] == b'=' {
            (0, true)
        } else {
            (decode_char(chunk[2])?, false)
        };

        let (c3, p3) = if chunk[3] == b'=' {
            (0, true)
        } else {
            (decode_char(chunk[3])?, false)
        };

        if p2 && !p3 {
            return Err("Illegal padding sequence");
        }

        let n = (c0 << 18) | (c1 << 12) | (c2 << 6) | c3;

        #[allow(clippy::cast_possible_truncation)]
        out.push(((n >> 16) & 0xFF) as u8);

        if !p2 {
            #[allow(clippy::cast_possible_truncation)]
            out.push(((n >> 8) & 0xFF) as u8);
        }

        if !p3 {
            #[allow(clippy::cast_possible_truncation)]
            out.push((n & 0xFF) as u8);
        }
    }

    Ok(out)
}
