//! Fetch Web API implementation (`fetch()`, `Headers`, `Request`, `Response`).

pub mod headers;
pub mod request;
pub mod response;
pub mod types;

pub use headers::{create_headers_object, register_headers_constructor};
pub use request::{create_request_object, register_request_constructor};
pub use response::{create_response_object, register_response_constructor};
pub use types::{FetchHandler, FetchRequest, FetchResponse, RichFetchHandler};

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::builtins::JsPromise,
};
use std::sync::Arc;

#[derive(Clone, Trace, Finalize)]
struct RichFetchHolder(#[unsafe_ignore_trace] RichFetchHandler);

/// Registers the global `window.fetch()` function, `Headers`, `Request`, and `Response` constructors.
///
/// # Errors
/// Returns `JsResult` if registration fails.
pub fn register_rich_fetch(context: &mut Context, fetch_handler: RichFetchHandler) -> JsResult<()> {
    register_headers_constructor(context)?;
    register_request_constructor(context)?;
    register_response_constructor(context)?;

    let holder = RichFetchHolder(fetch_handler);

    let fetch_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let input_val = args.get_or_undefined(0);
            let mut req = if let Some(obj) = input_val.as_object() {
                let url_prop = obj.get(js_string!("url"), ctx).unwrap_or_default();
                let method_prop = obj.get(js_string!("method"), ctx).unwrap_or_default();
                let url_str = if url_prop.is_undefined() {
                    input_val.to_string(ctx)?.to_std_string_escaped()
                } else {
                    url_prop.to_string(ctx)?.to_std_string_escaped()
                };
                let method_str = if method_prop.is_undefined() {
                    "GET".to_string()
                } else {
                    method_prop.to_string(ctx)?.to_std_string_escaped()
                };
                FetchRequest {
                    url: url_str,
                    method: method_str,
                    headers: Vec::new(),
                    body: None,
                }
            } else {
                let url_str = input_val.to_string(ctx)?.to_std_string_escaped();
                FetchRequest::get(url_str)
            };

            // Process optional init parameter
            if let Some(init_arg) = args.get(1)
                && let Some(init_obj) = init_arg.as_object()
            {
                if let Ok(method_val) = init_obj.get(js_string!("method"), ctx)
                    && !method_val.is_undefined()
                {
                    req.method = method_val.to_string(ctx)?.to_std_string_escaped();
                }
                if let Ok(body_val) = init_obj.get(js_string!("body"), ctx)
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

            match (captures.0)(&req) {
                Ok(response_data) => match create_response_object(ctx, response_data) {
                    Ok(resp_obj) => {
                        let promise = JsPromise::resolve(JsValue::from(resp_obj), ctx);
                        Ok(JsValue::from(promise))
                    }
                    Err(err) => {
                        let promise = JsPromise::reject(err, ctx);
                        Ok(JsValue::from(promise))
                    }
                },
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

/// Registers the global `window.fetch()` using a basic string fetch handler for backwards compatibility.
///
/// # Errors
/// Returns `JsResult` if registration fails.
pub fn register_fetch(context: &mut Context, fetch_handler: FetchHandler) -> JsResult<()> {
    let rich_handler: RichFetchHandler = Arc::new(move |req: &FetchRequest| {
        let text_body = (fetch_handler)(&req.url)?;
        Ok(FetchResponse::ok_text(&req.url, text_body))
    });

    register_rich_fetch(context, rich_handler)
}
