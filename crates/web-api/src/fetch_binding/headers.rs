//! JavaScript `Headers` binding supporting standard HTTP header manipulation.

use boa_engine::{
    Context, JsArgs, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
};
use std::sync::{Arc, Mutex};

/// Internal trace-safe wrapper for header storage.
#[derive(Clone, Trace, Finalize)]
pub struct HeadersHolder(#[unsafe_ignore_trace] pub Arc<Mutex<Vec<(String, String)>>>);

/// Creates a JavaScript `Headers` object initialized with given header entries.
pub fn create_headers_object(
    context: &mut Context,
    headers: Vec<(String, String)>,
) -> JsResult<JsObject> {
    let holder = HeadersHolder(Arc::new(Mutex::new(headers)));

    let get_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_val = args.get_or_undefined(0).to_string(ctx)?;
            let name = name_val.to_std_string_escaped();
            let val = {
                let guard = captures.0.lock().map_err(|_| {
                    boa_engine::JsNativeError::error().with_message("Mutex poisoned")
                })?;
                guard
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&name))
                    .map(|(_, v)| v.clone())
            };

            Ok(val.map_or_else(JsValue::null, |v| JsValue::from(js_string!(v))))
        },
        holder.clone(),
    );

    let has_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_val = args.get_or_undefined(0).to_string(ctx)?;
            let name = name_val.to_std_string_escaped();
            let found = {
                let guard = captures.0.lock().map_err(|_| {
                    boa_engine::JsNativeError::error().with_message("Mutex poisoned")
                })?;
                guard.iter().any(|(k, _)| k.eq_ignore_ascii_case(&name))
            };
            Ok(JsValue::from(found))
        },
        holder.clone(),
    );

    let set_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_val = args.get_or_undefined(0).to_string(ctx)?;
            let value_val = args.get_or_undefined(1).to_string(ctx)?;
            let name = name_val.to_std_string_escaped();
            let value = value_val.to_std_string_escaped();

            {
                let mut guard = captures.0.lock().map_err(|_| {
                    boa_engine::JsNativeError::error().with_message("Mutex poisoned")
                })?;
                guard.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
                guard.push((name, value));
            }
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let append_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_val = args.get_or_undefined(0).to_string(ctx)?;
            let value_val = args.get_or_undefined(1).to_string(ctx)?;
            let name = name_val.to_std_string_escaped();
            let value = value_val.to_std_string_escaped();

            {
                let mut guard = captures.0.lock().map_err(|_| {
                    boa_engine::JsNativeError::error().with_message("Mutex poisoned")
                })?;
                guard.push((name, value));
            }
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let delete_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_val = args.get_or_undefined(0).to_string(ctx)?;
            let name = name_val.to_std_string_escaped();

            {
                let mut guard = captures.0.lock().map_err(|_| {
                    boa_engine::JsNativeError::error().with_message("Mutex poisoned")
                })?;
                guard.retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
            }
            Ok(JsValue::undefined())
        },
        holder,
    );

    let obj = ObjectInitializer::new(context)
        .function(get_fn, js_string!("get"), 1)
        .function(has_fn, js_string!("has"), 1)
        .function(set_fn, js_string!("set"), 2)
        .function(append_fn, js_string!("append"), 2)
        .function(delete_fn, js_string!("delete"), 1)
        .build();

    Ok(obj)
}

/// Registers the global `Headers` constructor into Boa `Context`.
///
/// # Errors
/// Returns `JsResult` if registration fails.
pub fn register_headers_constructor(context: &mut Context) -> JsResult<()> {
    let ctor = NativeFunction::from_copy_closure(|_this, _args, ctx| {
        let headers_obj = create_headers_object(ctx, Vec::new())?;
        Ok(JsValue::from(headers_obj))
    });

    context.register_global_callable(js_string!("Headers"), 0, ctor)?;
    Ok(())
}
