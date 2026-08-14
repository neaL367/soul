//! Asynchronous message routing, callback dispatching, and response correlation.

use crate::error::IpcError;
use crate::messages::{IpcMessage, MessageId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

type MessageCallback = Arc<dyn Fn(&IpcMessage) + Send + Sync>;

/// Message dispatcher managing request correlation and event handlers.
#[derive(Default)]
pub struct IpcDispatcher {
    handlers: Vec<MessageCallback>,
    pending_responses: HashMap<MessageId, oneshot::Sender<IpcMessage>>,
}

impl IpcDispatcher {
    /// Creates a new empty `IpcDispatcher`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            pending_responses: HashMap::new(),
        }
    }

    /// Registers a listener callback triggered on every dispatched incoming message.
    pub fn register_handler<F>(&mut self, handler: F)
    where
        F: Fn(&IpcMessage) + Send + Sync + 'static,
    {
        self.handlers.push(Arc::new(handler));
    }

    /// Registers a pending request and returns a `oneshot::Receiver` awaiting the correlated response.
    pub fn expect_response(&mut self, request_id: MessageId) -> oneshot::Receiver<IpcMessage> {
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id, tx);
        rx
    }

    /// Dispatches an incoming `IpcMessage` to matching pending response waiters or general event handlers.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::InvalidMessage` if a correlated response waiter had already closed unexpectedly.
    pub fn dispatch(&mut self, message: IpcMessage) -> Result<(), IpcError> {
        if let Some(corr_id) = message.correlation_id
            && let Some(sender) = self.pending_responses.remove(&corr_id)
        {
            let _ = sender.send(message);
            return Ok(());
        }

        for handler in &self.handlers {
            handler(&message);
        }

        Ok(())
    }
}
