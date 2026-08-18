//! JavaScript `Request` object binding.

use super::headers::create_headers_object;
use super::types::FetchRequest;
use boa_engine::{
    Context, JsArgs, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{ObjectInitializer, builtins::JsPromise},
    property::Attribute,
};

/// Internal trace-safe wrapper for `FetchRequest`.
#[derive(Clone, Trace, Finalize)]
pub struct RequestHolder(#[unsafe_ignore_trace] pub FetchRequest);

/// Creates a JavaScript `Request` object.
///
/// # Errors
/// Returns `JsResult` if object initialization fails.
pub fn create_request_object(context: &mut Context, request: FetchRequest) -> JsResult<JsObject> {
    let holder = RequestHolder(request.clone());

    let text_holder = holder.clone();
    let text_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let body_str = cap
                .0
                .body
                .as_ref()
                .map_or_else(String::new, |b| String::from_utf8_lossy(b).into_owned());
            let promise = JsPromise::resolve(JsValue::from(js_string!(body_str)), ctx);
            Ok(JsValue::from(promise))
        },
        text_holder,
    );

    let json_holder = holder.clone();
    let json_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let body_str = cap
                .0
                .body
                .as_ref()
                .map_or_else(String::new, |b| String::from_utf8_lossy(b).into_owned());
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

    let clone_holder = holder;
    let clone_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, cap, ctx| {
            let cloned = create_request_object(ctx, cap.0.clone())?;
            Ok(JsValue::from(cloned))
        },
        clone_holder,
    );

    let headers_obj = create_headers_object(context, request.headers)?;

    let obj = ObjectInitializer::new(context)
        .property(
            js_string!("url"),
            JsValue::from(js_string!(request.url)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("method"),
            JsValue::from(js_string!(request.method)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("headers"),
            JsValue::from(headers_obj),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(text_fn, js_string!("text"), 0)
        .function(json_fn, js_string!("json"), 0)
        .function(clone_fn, js_string!("clone"), 0)
        .build();

    Ok(obj)
}

/// Registers the global `Request` constructor into Boa `Context`.
///
/// # Errors
/// Returns `JsResult` if registration fails.
pub fn register_request_constructor(context: &mut Context) -> JsResult<()> {
    let ctor = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let url_val = args.get_or_undefined(0).to_string(ctx)?;
        let url_str = url_val.to_std_string_escaped();
        let mut req = FetchRequest::get(url_str);

        if let Some(opts) = args.get(1)
            && let Some(obj) = opts.as_object()
        {
            if let Ok(method_val) = obj.get(js_string!("method"), ctx)
                && !method_val.is_undefined()
            {
                req.method = method_val.to_string(ctx)?.to_std_string_escaped();
            }
            if let Ok(body_val) = obj.get(js_string!("body"), ctx)
                && !body_val.is_undefined()
            {
                req.body = Some(
                    body_val
                        .to_string(ctx)?
                        .to_std_string_escaped()
                        .into_bytes(),
                );
            }
        }

        let req_obj = create_request_object(ctx, req)?;
        Ok(JsValue::from(req_obj))
    });

    context.register_global_callable(js_string!("Request"), 1, ctor)?;
    Ok(())
}
