//! WHATWG WebSocket API bindings (HTML Living Standard §9.3).
//!
//! Provides the global `WebSocket` constructor, state properties (`CONNECTING`,
//! `OPEN`, `CLOSING`, `CLOSED`), and standard methods (`send`, `close`).

use boa_engine::{
    Context, JsArgs, JsError, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
};
use std::sync::{Arc, Mutex};

/// WebSocket connection ready states (WHATWG §9.3).
pub const CONNECTING: u16 = 0;
/// The connection is open and ready to communicate.
pub const OPEN: u16 = 1;
/// The connection is in the process of closing.
pub const CLOSING: u16 = 2;
/// The connection is closed or couldn't be opened.
pub const CLOSED: u16 = 3;

/// Abstraction for an active WebSocket communication channel.
pub trait WebSocketSession: Send + Sync {
    /// Sends a text message across the WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns an error string if transmission fails.
    fn send(&self, message: &str) -> Result<(), String>;

    /// Closes the WebSocket connection with a status code and reason string.
    ///
    /// # Errors
    ///
    /// Returns an error string if closure fails.
    fn close(&self, code: u16, reason: &str) -> Result<(), String>;

    /// Returns the current ready state integer (0..=3).
    fn ready_state(&self) -> u16;
}

/// Simple in-memory mock WebSocket session for testing and headless execution.
#[derive(Default)]
pub struct MockWebSocketSession {
    state: Mutex<u16>,
    sent_messages: Mutex<Vec<String>>,
}

impl MockWebSocketSession {
    /// Creates a new open mock session.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(OPEN),
            sent_messages: Mutex::new(Vec::new()),
        }
    }

    /// Returns all messages sent through this mock session.
    #[must_use]
    pub fn sent_messages(&self) -> Vec<String> {
        self.sent_messages
            .lock()
            .map_or_else(|_| Vec::new(), |m| m.clone())
    }
}

impl WebSocketSession for MockWebSocketSession {
    fn send(&self, message: &str) -> Result<(), String> {
        if let Ok(mut sent) = self.sent_messages.lock() {
            sent.push(message.to_string());
        }
        Ok(())
    }

    fn close(&self, _code: u16, _reason: &str) -> Result<(), String> {
        if let Ok(mut s) = self.state.lock() {
            *s = CLOSED;
        }
        Ok(())
    }

    fn ready_state(&self) -> u16 {
        self.state.lock().map_or(CLOSED, |s| *s)
    }
}

/// Factory closure creating active [`WebSocketSession`] instances given a URL and protocols.
pub type WebSocketFactory =
    Arc<dyn Fn(&str, &[String]) -> Result<Arc<dyn WebSocketSession>, String> + Send + Sync>;

#[derive(Clone, Trace, Finalize)]
struct WsFactoryCapture {
    #[unsafe_ignore_trace]
    factory: Option<WebSocketFactory>,
}

#[derive(Clone, Trace, Finalize)]
struct WsCaptures {
    #[unsafe_ignore_trace]
    session: Arc<dyn WebSocketSession>,
}

/// Registers the global `WebSocket` constructor and constants into a Boa execution context.
///
/// # Errors
///
/// Returns `JsResult` error if global property registration fails.
pub fn register_websocket(ctx: &mut Context, factory: Option<WebSocketFactory>) -> JsResult<()> {
    let ws_constructor = NativeFunction::from_copy_closure_with_captures(
        move |_this, args, fact_cap, ctx| {
            let url_str = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            let session: Arc<dyn WebSocketSession> = if let Some(ref f) = fact_cap.factory {
                f(&url_str, &[]).map_err(|e| JsError::from_opaque(JsValue::from(js_string!(e))))?
            } else {
                Arc::new(MockWebSocketSession::new())
            };

            let caps = WsCaptures { session };

            let caps_send = caps.clone();
            let send_fn = NativeFunction::from_copy_closure_with_captures(
                move |_this, args, caps, ctx| {
                    let msg = args
                        .get_or_undefined(0)
                        .to_string(ctx)?
                        .to_std_string_escaped();
                    caps.session
                        .send(&msg)
                        .map_err(|e| JsError::from_opaque(JsValue::from(js_string!(e))))?;
                    Ok(JsValue::undefined())
                },
                caps_send,
            );

            let caps_close = caps.clone();
            let close_fn = NativeFunction::from_copy_closure_with_captures(
                move |_this, args, caps, ctx| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let code = args
                        .first()
                        .and_then(JsValue::as_number)
                        .map_or(1000, |n| n as u16);
                    let reason = args.get(1).map_or_else(
                        || Ok(String::new()),
                        |v| v.to_string(ctx).map(|s| s.to_std_string_escaped()),
                    )?;
                    caps.session
                        .close(code, &reason)
                        .map_err(|e| JsError::from_opaque(JsValue::from(js_string!(e))))?;
                    Ok(JsValue::undefined())
                },
                caps_close,
            );

            let obj = ObjectInitializer::new(ctx)
                .property(js_string!("url"), js_string!(url_str), Attribute::READONLY)
                .property(
                    js_string!("readyState"),
                    f64::from(caps.session.ready_state()),
                    Attribute::all(),
                )
                .property(js_string!("bufferedAmount"), 0.0, Attribute::READONLY)
                .property(js_string!("protocol"), js_string!(""), Attribute::READONLY)
                .property(
                    js_string!("extensions"),
                    js_string!(""),
                    Attribute::READONLY,
                )
                .property(
                    js_string!("binaryType"),
                    js_string!("blob"),
                    Attribute::all(),
                )
                .property(js_string!("onopen"), JsValue::null(), Attribute::all())
                .property(js_string!("onmessage"), JsValue::null(), Attribute::all())
                .property(js_string!("onerror"), JsValue::null(), Attribute::all())
                .property(js_string!("onclose"), JsValue::null(), Attribute::all())
                .function(send_fn, js_string!("send"), 1)
                .function(close_fn, js_string!("close"), 2)
                .build();

            Ok(JsValue::from(obj))
        },
        WsFactoryCapture { factory },
    );

    let js_fn = FunctionObjectBuilder::new(ctx.realm(), ws_constructor)
        .constructor(true)
        .name(js_string!("WebSocket"))
        .length(1)
        .build();

    js_fn.set(js_string!("CONNECTING"), f64::from(CONNECTING), false, ctx)?;
    js_fn.set(js_string!("OPEN"), f64::from(OPEN), false, ctx)?;
    js_fn.set(js_string!("CLOSING"), f64::from(CLOSING), false, ctx)?;
    js_fn.set(js_string!("CLOSED"), f64::from(CLOSED), false, ctx)?;

    ctx.register_global_property(
        js_string!("WebSocket"),
        js_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
