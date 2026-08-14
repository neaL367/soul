//! Timer JavaScript bindings (`setTimeout`, `clearTimeout`).

use boa_engine::{
    Context, JsArgs, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Global timer queue storing pending callbacks for the JavaScript execution context.
pub type TimerQueue = Rc<RefCell<Vec<boa_engine::JsObject>>>;

#[derive(Clone, Trace, Finalize)]
struct TimerHolder(#[unsafe_ignore_trace] TimerQueue);

/// Registers `setTimeout` and `clearTimeout` global functions.
///
/// # Errors
///
/// Returns a `JsResult` error if global property registration fails.
pub fn register_timers(context: &mut Context, pending_callbacks: TimerQueue) -> JsResult<()> {
    let set_timeout_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, _ctx| {
            let callback = args.get_or_undefined(0);
            if let Some(obj) = callback.as_object() {
                captures.0.borrow_mut().push(obj);
            }
            let id = 1;
            Ok(JsValue::from(id))
        },
        TimerHolder(pending_callbacks),
    );

    let clear_timeout_fn =
        NativeFunction::from_copy_closure(|_this, _args, _ctx| Ok(JsValue::undefined()));

    context.register_global_callable(js_string!("setTimeout"), 2, set_timeout_fn)?;
    context.register_global_callable(js_string!("clearTimeout"), 1, clear_timeout_fn)?;

    Ok(())
}
