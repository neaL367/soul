//! W3C Web Cryptography API (`window.crypto`) ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::native_function::NativeFunction;
use boa_engine::object::ObjectInitializer;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue};
use platform_windows::CryptoRandom;

/// Registers the standard `window.crypto` and `globalThis.crypto` objects in the JS realm.
pub fn register_crypto(context: &mut Context) {
    let crypto_obj = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(js_random_uuid),
            JsString::from("randomUUID"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_get_random_values),
            JsString::from("getRandomValues"),
            1,
        )
        .build();

    let global = context.global_object();
    let _ = global.set(JsString::from("crypto"), crypto_obj.clone(), false, context);

    if let Ok(window_val) = global.get(JsString::from("window"), context)
        && let Some(window_obj) = window_val.as_object()
    {
        let _ = window_obj.set(JsString::from("crypto"), crypto_obj, false, context);
    }
}

/// Native implementation of `crypto.randomUUID()`.
fn js_random_uuid(_this: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
    let uuid = CryptoRandom::random_uuid_v4();
    Ok(JsValue::from(JsString::from(uuid)))
}

/// Native implementation of `crypto.getRandomValues(array)`.
fn js_get_random_values(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let target = args.get_or_undefined(0);
    let Some(obj) = target.as_object() else {
        return Err(JsError::from(JsNativeError::typ().with_message(
            "getRandomValues argument must be an Object/TypedArray",
        )));
    };

    // Fill typed array or array-like object buffer
    fill_typed_array_random_values(&obj, context)?;

    Ok(target.clone())
}

/// Fills elements of a `TypedArray` with cryptographically secure random bytes.
fn fill_typed_array_random_values(obj: &JsObject, context: &mut Context) -> JsResult<()> {
    let length_val = obj.get(JsString::from("length"), context)?;
    let length = length_val.to_u32(context).unwrap_or(0) as usize;

    if length == 0 {
        return Ok(());
    }

    // Limit maximum byte request size to 64 KiB as specified by W3C WebCrypto spec
    if length > 65536 {
        return Err(JsError::from(
            JsNativeError::range().with_message("getRandomValues exceeds 65536 byte limit"),
        ));
    }

    let mut raw_bytes = vec![0u8; length];
    CryptoRandom::fill_random_bytes(&mut raw_bytes)
        .map_err(|e| JsError::from(JsNativeError::typ().with_message(e)))?;

    for (idx, byte) in raw_bytes.into_iter().enumerate() {
        let _ = obj.set(idx, JsValue::from(u32::from(byte)), false, context);
    }

    Ok(())
}
