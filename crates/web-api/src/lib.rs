//! Web APIs implementation including DOM, Console, Timers, Web Storage, Fetch,
//! Workers, `EventTarget`, and `MutationObserver`.

pub mod console;
pub mod dom_bindings;
pub mod event_binding;
pub mod fetch_binding;
pub mod indexeddb_binding;
pub mod mutation_observer;
pub mod storage_binding;
pub mod timers;
pub mod websocket_binding;
pub mod window_bindings;
pub mod worker;

pub use console::register_console;
pub use dom_bindings::register_dom;
pub use event_binding::{CallbackStore, create_event_target, register_custom_event};
pub use fetch_binding::{
    FetchHandler, FetchRequest, FetchResponse, RichFetchHandler, register_fetch,
    register_rich_fetch,
};
pub use indexeddb_binding::register_indexeddb;
pub use mutation_observer::{
    MutationObserverInit, MutationQueue, MutationRecord, MutationRecordType,
    register_mutation_observer,
};
pub use storage_binding::{register_local_storage, register_session_storage};
pub use timers::{TimerQueue, TimerState, register_timers};
pub use websocket_binding::{
    CLOSED, CLOSING, CONNECTING, MockWebSocketSession, OPEN, WebSocketFactory, WebSocketSession,
    register_websocket,
};
pub use window_bindings::register_window;
pub use worker::WebWorker;

use boa_engine::{Context, JsResult};
use dom::Document;
use std::sync::{Arc, Mutex};

/// Registers common Web APIs (`console`, `timers`, `document`, `CustomEvent`,
/// `MutationObserver`, `WebSocket`) into a Boa `Context`.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn bind_web_apis(
    context: &mut Context,
    document: Option<Arc<Mutex<Document>>>,
    captured_logs: Option<Arc<Mutex<Vec<String>>>>,
    pending_timers: Option<TimerQueue>,
    mutation_queue: Option<Arc<Mutex<MutationQueue>>>,
) -> JsResult<()> {
    register_console(context, captured_logs)?;

    if let Some(queue) = pending_timers {
        register_timers(context, queue)?;
    }

    if let Some(doc) = document {
        register_dom(context, doc)?;
    }

    register_custom_event(context)?;

    if let Some(mq) = mutation_queue {
        register_mutation_observer(context, mq)?;
    }

    register_websocket(context, None)?;

    Ok(())
}
