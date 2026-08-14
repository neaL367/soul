//! `fetch()` global function binding returning a JavaScript Promise.

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{ObjectInitializer, builtins::JsPromise},
    property::Attribute,
};
use std::sync::Arc;

/// Fetch response payload provider callback type.
pub type FetchHandler = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

#[derive(Clone, Trace, Finalize)]
struct FetchHolder(#[unsafe_ignore_trace] FetchHandler);

/// Registers the global `window.fetch()` function into the Boa `Context`.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn register_fetch(context: &mut Context, fetch_handler: FetchHandler) -> JsResult<()> {
    let holder = FetchHolder(fetch_handler);

    let fetch_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let url_arg = args.get_or_undefined(0).to_string(ctx)?;
            let url_str = url_arg.to_std_string_escaped();

            match (captures.0)(&url_str) {
                Ok(body_text) => {
                    let text_str = body_text;
                    let text_fn = NativeFunction::from_copy_closure_with_captures(
                        |_this, _args, text_cap, text_ctx| {
                            let promise = JsPromise::resolve(
                                JsValue::from(js_string!(text_cap.clone())),
                                text_ctx,
                            );
                            Ok(JsValue::from(promise))
                        },
                        text_str,
                    );

                    let response_obj = ObjectInitializer::new(ctx)
                        .property(
                            js_string!("ok"),
                            JsValue::from(true),
                            Attribute::READONLY | Attribute::ENUMERABLE,
                        )
                        .property(
                            js_string!("status"),
                            JsValue::from(200),
                            Attribute::READONLY | Attribute::ENUMERABLE,
                        )
                        .function(text_fn, js_string!("text"), 0)
                        .build();

                    let promise = JsPromise::resolve(JsValue::from(response_obj), ctx);
                    Ok(JsValue::from(promise))
                }
                Err(err_msg) => {
                    let native_err = JsNativeError::error().with_message(err_msg);
                    let promise = JsPromise::reject(native_err, ctx);
                    Ok(JsValue::from(promise))
                }
            }
        },
        holder,
    );

    context.register_global_callable(js_string!("fetch"), 1, fetch_fn)?;
    Ok(())
}
