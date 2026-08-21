//! W3C High Resolution Time (`performance.now()`) and Animation Frame bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::native_function::NativeFunction;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsResult, JsString, JsValue};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static NEXT_RAF_ID: AtomicU32 = AtomicU32::new(1);

/// Registers `window.performance`, `requestAnimationFrame`, and `cancelAnimationFrame`.
pub fn register_performance(context: &mut Context) {
    let _ = PROCESS_START.get_or_init(Instant::now);

    let time_origin_fn =
        NativeFunction::from_fn_ptr(js_get_time_origin).to_js_function(context.realm());

    let performance_obj = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(js_performance_now),
            JsString::from("now"),
            0,
        )
        .accessor(
            JsString::from("timeOrigin"),
            Some(time_origin_fn),
            None,
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(js_performance_mark),
            JsString::from("mark"),
            1,
        )
        .build();

    let global = context.global_object();

    let _ = global.set(
        JsString::from("performance"),
        performance_obj.clone(),
        false,
        context,
    );

    let _ = global.set(
        JsString::from("requestAnimationFrame"),
        NativeFunction::from_fn_ptr(js_request_animation_frame).to_js_function(context.realm()),
        false,
        context,
    );

    let _ = global.set(
        JsString::from("cancelAnimationFrame"),
        NativeFunction::from_fn_ptr(js_cancel_animation_frame).to_js_function(context.realm()),
        false,
        context,
    );

    if let Ok(window_val) = global.get(JsString::from("window"), context)
        && let Some(window_obj) = window_val.as_object()
    {
        let _ = window_obj.set(
            JsString::from("performance"),
            performance_obj,
            false,
            context,
        );
        let _ = window_obj.set(
            JsString::from("requestAnimationFrame"),
            NativeFunction::from_fn_ptr(js_request_animation_frame).to_js_function(context.realm()),
            false,
            context,
        );
        let _ = window_obj.set(
            JsString::from("cancelAnimationFrame"),
            NativeFunction::from_fn_ptr(js_cancel_animation_frame).to_js_function(context.realm()),
            false,
            context,
        );
    }
}

/// Native implementation of `performance.now()`.
fn js_performance_now(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let start = PROCESS_START.get_or_init(Instant::now);
    let elapsed = start.elapsed();
    let millis = (elapsed.as_secs_f64()) * 1000.0;
    Ok(JsValue::from(millis))
}

/// Native accessor for `performance.timeOrigin`.
fn js_get_time_origin(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let origin_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    Ok(JsValue::from(origin_ms))
}

/// Native implementation of `performance.mark(name)`.
const fn js_performance_mark(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

/// Native implementation of `requestAnimationFrame(callback)`.
fn js_request_animation_frame(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let id = NEXT_RAF_ID.fetch_add(1, Ordering::Relaxed);
    let callback = args.get_or_undefined(0);

    // If a valid callable callback was provided, invoke it with the current timestamp
    if let Some(func) = callback.as_callable() {
        let start = PROCESS_START.get_or_init(Instant::now);
        let timestamp = JsValue::from((start.elapsed().as_secs_f64()) * 1000.0);
        let _ = func.call(&JsValue::undefined(), &[timestamp], context);
    }

    Ok(JsValue::from(id))
}

/// Native implementation of `cancelAnimationFrame(id)`.
const fn js_cancel_animation_frame(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}
