//! Chrome `DevTools` Protocol JSON-RPC command router and dispatcher.

use crate::console_monitor::ConsoleMonitor;
use crate::dom_inspector::DomInspector;
use crate::network_monitor::NetworkMonitor;
use crate::protocol::{CdpRequest, CdpResponse};
use dom::Document;
use serde_json::json;

/// `DevTools` CDP server handling protocol commands from connected inspector frontends.
#[derive(Default)]
pub struct CdpServer {
    /// Active network traffic monitor.
    pub network: NetworkMonitor,
    /// Active console message monitor.
    pub console: ConsoleMonitor,
}

impl CdpServer {
    /// Creates a new `CdpServer`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            network: NetworkMonitor::new(),
            console: ConsoleMonitor::new(),
        }
    }

    /// Dispatches and processes an incoming CDP request against current page state.
    #[must_use]
    pub fn handle_request(
        &mut self,
        request: &CdpRequest,
        document: Option<&Document>,
    ) -> CdpResponse {
        match request.method.as_str() {
            "DOM.getDocument" => document.map_or_else(
                || CdpResponse::error(request.id, "No active document".to_string()),
                |doc| {
                    let root_id = doc.root_id();
                    let tree = DomInspector::serialize_subtree(doc, root_id);
                    CdpResponse::success(request.id, json!({ "root": tree }))
                },
            ),
            "Network.getEvents" => {
                let events = self.network.get_events();
                CdpResponse::success(request.id, json!({ "events": events }))
            }
            "Console.getMessages" => {
                let messages = self.console.get_messages();
                CdpResponse::success(request.id, json!({ "messages": messages }))
            }
            unknown => CdpResponse::error(request.id, format!("Unsupported CDP method: {unknown}")),
        }
    }
}
