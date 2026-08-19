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
    Context, JsArgs, JsError, JsNativeError, JsResult, JsValue,
    gc::{Finalize, Trace},
    job::{Job, NativeAsyncJob},
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

            // Run the (potentially blocking) handler on a worker thread and
            // settle the returned promise from an async job once it completes,
            // so a slow network response never blocks the JavaScript thread.
            let (promise, resolvers) = JsPromise::new_pending(ctx);
            let (tx, rx) = tokio::sync::oneshot::channel();
            let handler = captures.0.clone();
            std::thread::spawn(move || {
                let result = (handler)(&req);
                let _ = tx.send(result);
            });

            let job = NativeAsyncJob::new(async move |job_ctx| {
                let outcome = rx.await;
                let mut ctx = job_ctx.borrow_mut();
                let settled = match outcome {
                    Ok(Ok(response_data)) => {
                        let resp_obj = create_response_object(&mut ctx, response_data)?;
                        resolvers
                            .resolve
                            .call(&JsValue::undefined(), &[resp_obj.into()], &mut ctx)
                    }
                    Ok(Err(err_msg)) => {
                        let native_err = JsNativeError::error().with_message(err_msg);
                        let opaque = JsError::from_native(native_err).to_opaque(&mut ctx);
                        resolvers
                            .reject
                            .call(&JsValue::undefined(), &[opaque], &mut ctx)
                    }
                    Err(_) => {
                        let native_err =
                            JsNativeError::error().with_message("fetch worker thread failed");
                        let opaque = JsError::from_native(native_err).to_opaque(&mut ctx);
                        resolvers
                            .reject
                            .call(&JsValue::undefined(), &[opaque], &mut ctx)
                    }
                };
                settled.map(|_| JsValue::undefined())
            });
            ctx.enqueue_job(Job::from(job));

            Ok(JsValue::from(promise))
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
