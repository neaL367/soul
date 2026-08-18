//! JavaScript `Response` object binding supporting status, headers, and body readers.

use super::headers::create_headers_object;
use super::types::FetchResponse;
use boa_engine::{
    Context, JsArgs, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{
        ObjectInitializer,
        builtins::{JsArray, JsPromise},
    },
    property::Attribute,
};

/// Internal trace-safe wrapper for `FetchResponse`.
#[derive(Clone, Trace, Finalize)]
pub struct ResponseHolder(#[unsafe_ignore_trace] pub FetchResponse);

/// Creates a JavaScript `Response` object representing a completed fetch.
///
/// # Errors
/// Returns `JsResult` if object initialization fails.
pub fn create_response_object(
    context: &mut Context,
    response: FetchResponse,
) -> JsResult<JsObject> {
    let holder = ResponseHolder(response.clone());

    let text_holder = holder.clone();
    let text_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let body_str = String::from_utf8_lossy(&cap.0.body).into_owned();
            let promise = JsPromise::resolve(JsValue::from(js_string!(body_str)), ctx);
            Ok(JsValue::from(promise))
        },
        text_holder,
    );

    let json_holder = holder.clone();
    let json_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let body_str = String::from_utf8_lossy(&cap.0.body).into_owned();
            let escaped = serde_json::to_string(&body_str).unwrap_or_else(|_| "\"\"".to_string());
            let script = format!("JSON.parse({escaped})");
            match ctx.eval(boa_engine::Source::from_bytes(script.as_bytes())) {
                Ok(val) => {
                    let promise = JsPromise::resolve(val, ctx);
                    Ok(JsValue::from(promise))
                }
                Err(err) => {
                    let promise = JsPromise::reject(err, ctx);
                    Ok(JsValue::from(promise))
                }
            }
        },
        json_holder,
    );

    let bytes_holder = holder.clone();
    let array_buffer_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let array = JsArray::new(ctx);
            for (idx, byte) in cap.0.body.iter().enumerate() {
                let _ = array.set(idx, JsValue::from(*byte), true, ctx);
            }
            let promise = JsPromise::resolve(JsValue::from(array), ctx);
            Ok(JsValue::from(promise))
        },
        bytes_holder,
    );

    let clone_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let cloned_obj = create_response_object(ctx, cap.0.clone())?;
            Ok(JsValue::from(cloned_obj))
        },
        holder,
    );

    let headers_obj = create_headers_object(context, response.headers)?;

    let is_ok = response.status >= 200 && response.status < 300;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("ok"),
            JsValue::from(is_ok),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("status"),
            JsValue::from(response.status),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("statusText"),
            JsValue::from(js_string!(response.status_text)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("url"),
            JsValue::from(js_string!(response.url)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("headers"),
            JsValue::from(headers_obj),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(text_fn, js_string!("text"), 0)
        .function(json_fn, js_string!("json"), 0)
        .function(array_buffer_fn, js_string!("arrayBuffer"), 0)
        .function(clone_fn, js_string!("clone"), 0)
        .build();

    Ok(obj)
}

/// Registers the global `Response` constructor in the Boa `Context`.
///
/// # Errors
/// Returns `JsResult` if registration fails.
pub fn register_response_constructor(context: &mut Context) -> JsResult<()> {
    let ctor = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let body_val = args.get_or_undefined(0);
        let body_bytes = if body_val.is_undefined() || body_val.is_null() {
            Vec::new()
        } else {
            let s = body_val.to_string(ctx)?;
            s.to_std_string_escaped().into_bytes()
        };

        let response = FetchResponse::from_status_and_body(200, body_bytes);
        let res_obj = create_response_object(ctx, response)?;
        Ok(JsValue::from(res_obj))
    });

    context.register_global_callable(js_string!("Response"), 1, ctor)?;
    Ok(())
}
