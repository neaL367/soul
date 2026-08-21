//! W3C CSSOM View Module `window.matchMedia()` and `MediaQueryList` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::native_function::NativeFunction;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsResult, JsString, JsValue};

/// Evaluates a CSS media query string against default or specified viewport metrics.
#[must_use]
pub fn evaluate_media_query(query: &str, viewport_w: f32, _viewport_h: f32, is_dark: bool) -> bool {
    let lower = query.to_ascii_lowercase();
    let trimmed = lower.trim();

    if trimmed == "all" || trimmed.is_empty() {
        return true;
    }

    if trimmed.contains("prefers-color-scheme: dark") {
        return is_dark;
    }
    if trimmed.contains("prefers-color-scheme: light") {
        return !is_dark;
    }

    if let Some(idx) = trimmed.find("min-width:") {
        let rest = &trimmed[idx + 10..];
        let num_part = rest.trim().trim_end_matches(')').trim_end_matches("px");
        if let Ok(min_w) = num_part.trim().parse::<f32>() {
            return viewport_w >= min_w;
        }
    }

    if let Some(idx) = trimmed.find("max-width:") {
        let rest = &trimmed[idx + 10..];
        let num_part = rest.trim().trim_end_matches(')').trim_end_matches("px");
        if let Ok(max_w) = num_part.trim().parse::<f32>() {
            return viewport_w <= max_w;
        }
    }

    if trimmed.contains("screen") {
        return true;
    }

    false
}

/// Registers `window.matchMedia` and `globalThis.matchMedia` in the JS context.
pub fn register_match_media(context: &mut Context) {
    let match_media_fn =
        NativeFunction::from_fn_ptr(js_match_media).to_js_function(context.realm());

    let global = context.global_object();
    let _ = global.set(
        JsString::from("matchMedia"),
        match_media_fn.clone(),
        false,
        context,
    );

    if let Ok(window_val) = global.get(JsString::from("window"), context)
        && let Some(window_obj) = window_val.as_object()
    {
        let _ = window_obj.set(JsString::from("matchMedia"), match_media_fn, false, context);
    }
}

/// Native implementation of `window.matchMedia(query)`.
fn js_match_media(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let query_str = args
        .get_or_undefined(0)
        .to_string(context)
        .unwrap_or_else(|_| JsString::from(""))
        .to_std_string_escaped();

    let matches = evaluate_media_query(&query_str, 1024.0, 768.0, false);

    let mql_obj = ObjectInitializer::new(context)
        .property(
            JsString::from("matches"),
            JsValue::from(matches),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            JsString::from("media"),
            JsValue::from(JsString::from(query_str)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(
            NativeFunction::from_fn_ptr(js_noop),
            JsString::from("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_noop),
            JsString::from("removeEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_noop),
            JsString::from("addListener"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_noop),
            JsString::from("removeListener"),
            1,
        )
        .build();

    Ok(JsValue::from(mql_obj))
}

const fn js_noop(_this: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}
