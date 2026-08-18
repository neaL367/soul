//! Web APIs implementation including DOM, Console, Timers, Web Storage, Fetch, and Workers.

pub mod console;
pub mod dom_bindings;
pub mod fetch_binding;
pub mod indexeddb_binding;
pub mod storage_binding;
pub mod timers;
pub mod window_bindings;
pub mod worker;

pub use console::register_console;
pub use dom_bindings::register_dom;
pub use fetch_binding::{FetchHandler, register_fetch};
pub use indexeddb_binding::register_indexeddb;
pub use storage_binding::{register_local_storage, register_session_storage};
pub use timers::{TimerQueue, register_timers};
pub use window_bindings::register_window;
pub use worker::WebWorker;

use boa_engine::{Context, JsResult};
use dom::Document;
use std::sync::{Arc, Mutex};

/// Registers common Web APIs (`console`, `timers`, `document`) into a Boa `Context`.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn bind_web_apis(
    context: &mut Context,
    document: Option<Arc<Mutex<Document>>>,
    captured_logs: Option<Arc<Mutex<Vec<String>>>>,
    pending_timers: Option<TimerQueue>,
) -> JsResult<()> {
    register_console(context, captured_logs)?;

    if let Some(queue) = pending_timers {
        register_timers(context, queue)?;
    }

    if let Some(doc) = document {
        register_dom(context, doc)?;
    }

    Ok(())
}
