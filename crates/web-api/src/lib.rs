//! Web APIs implementation, DOM JavaScript bindings, and `WebWorker` threads.

pub mod console;
pub mod dom_bindings;
pub mod timers;
pub mod worker;

pub use console::register_console;
pub use dom_bindings::register_dom;
pub use timers::{TimerQueue, register_timers};
pub use worker::WebWorker;

use boa_engine::{Context, JsResult};
use dom::Document;
use std::sync::{Arc, Mutex};

/// Binds all core Web APIs (`console`, `document`, `setTimeout`) into a Boa ECMAScript context.
///
/// # Errors
///
/// Returns a `JsResult` error if any global registration fails.
pub fn bind_web_apis(
    context: &mut Context,
    document: Option<Arc<Mutex<Document>>>,
    captured_logs: Option<Arc<Mutex<Vec<String>>>>,
    timer_queue: Option<TimerQueue>,
) -> JsResult<()> {
    register_console(context, captured_logs)?;

    if let Some(doc) = document {
        register_dom(context, doc)?;
    }

    if let Some(t_queue) = timer_queue {
        register_timers(context, t_queue)?;
    }

    Ok(())
}
