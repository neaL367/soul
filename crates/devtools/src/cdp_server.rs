//! Chrome `DevTools` Protocol JSON-RPC command router and dispatcher.

use crate::console_monitor::ConsoleMonitor;
use crate::dom_inspector::DomInspector;
use crate::network_monitor::NetworkMonitor;
use crate::protocol::{CdpRequest, CdpResponse};
use dom::{Document, NodeId};
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
            "DOM.querySelector" => {
                let selector = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("selector"))
                    .and_then(serde_json::Value::as_str);

                match (document, selector) {
                    (Some(doc), Some(sel)) => {
                        let node_id = find_node_by_selector(doc, sel);
                        node_id.map_or_else(
                            || CdpResponse::success(request.id, json!({ "nodeId": 0 })),
                            |nid| {
                                let node_obj = DomInspector::serialize_subtree(doc, nid);
                                CdpResponse::success(
                                    request.id,
                                    json!({ "nodeId": nid.0, "node": node_obj }),
                                )
                            },
                        )
                    }
                    (None, _) => CdpResponse::error(request.id, "No active document".to_string()),
                    (_, None) => {
                        CdpResponse::error(request.id, "Missing selector parameter".to_string())
                    }
                }
            }
            "Network.getEvents" => {
                let events = self.network.get_events();
                CdpResponse::success(request.id, json!({ "events": events }))
            }
            "Network.clear" => {
                self.network.clear();
                CdpResponse::success(request.id, json!({ "cleared": true }))
            }
            "Console.getMessages" => {
                let messages = self.console.get_messages();
                CdpResponse::success(request.id, json!({ "messages": messages }))
            }
            "Console.clearMessages" => {
                self.console.clear();
                CdpResponse::success(request.id, json!({ "cleared": true }))
            }
            unknown => CdpResponse::error(request.id, format!("Unsupported CDP method: {unknown}")),
        }
    }
}

fn find_node_by_selector(doc: &Document, sel: &str) -> Option<NodeId> {
    sel.strip_prefix('#').map_or_else(
        || {
            sel.strip_prefix('.').map_or_else(
                || doc.get_elements_by_tag_name(sel).into_iter().next(),
                |class_name| {
                    doc.get_elements_by_class_name(class_name)
                        .into_iter()
                        .next()
                },
            )
        },
        |id_name| doc.get_element_by_id(id_name),
    )
}
