//! `console` object bindings for ECMAScript context logging.

use boa_engine::{
    Context, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Trace, Finalize)]
struct LogHolder(#[unsafe_ignore_trace] Option<Arc<Mutex<Vec<String>>>>);

/// Registers the global `console` object with `.log`, `.info`, `.warn`, and `.error` methods.
///
/// # Errors
///
/// Returns a `JsResult` error if property registration fails.
pub fn register_console(
    context: &mut Context,
    captured_logs: Option<Arc<Mutex<Vec<String>>>>,
) -> JsResult<()> {
    let log_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, _ctx| {
            let msg = args
                .iter()
                .map(|a| {
                    a.as_string()
                        .map_or_else(|| a.display().to_string(), |s| s.to_std_string_escaped())
                })
                .collect::<Vec<_>>()
                .join(" ");
            tracing::info!(target: "web_console", "{msg}");
            if let Some(ref c) = captures.0
                && let Ok(mut lock) = c.lock()
            {
                lock.push(msg);
            }
            Ok(JsValue::undefined())
        },
        LogHolder(captured_logs),
    );

    let console_obj = ObjectInitializer::new(context)
        .function(log_fn.clone(), js_string!("log"), 0)
        .function(log_fn.clone(), js_string!("info"), 0)
        .function(log_fn.clone(), js_string!("warn"), 0)
        .function(log_fn, js_string!("error"), 0)
        .build();

    context.register_global_property(
        js_string!("console"),
        console_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
