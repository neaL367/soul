//! Chrome `DevTools` Protocol JSON-RPC command router and dispatcher.

use crate::console_monitor::ConsoleMonitor;
use crate::dom_inspector::DomInspector;
use crate::network_monitor::NetworkMonitor;
use crate::protocol::{CdpRequest, CdpResponse};
use dom::{Document, NodeData, NodeId};
use javascript::JsRuntime;
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

    /// Dispatches and processes an incoming read-only CDP request against current page state.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
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
            "DOM.getOuterHTML" => {
                let node_id = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("nodeId"))
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| NodeId(n as usize));

                match (document, node_id) {
                    (Some(doc), Some(nid)) => {
                        let html = DomInspector::get_outer_html(doc, nid);
                        CdpResponse::success(request.id, json!({ "outerHTML": html }))
                    }
                    (None, _) => CdpResponse::error(request.id, "No active document".to_string()),
                    (_, None) => {
                        CdpResponse::error(request.id, "Missing nodeId parameter".to_string())
                    }
                }
            }
            "CSS.getComputedStyleForNode" => {
                let node_id = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("nodeId"))
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| NodeId(n as usize));

                match (document, node_id) {
                    (Some(doc), Some(nid)) => {
                        let styles = get_computed_styles(doc, nid);
                        CdpResponse::success(request.id, json!({ "computedStyle": styles }))
                    }
                    (None, _) => CdpResponse::error(request.id, "No active document".to_string()),
                    (_, None) => {
                        CdpResponse::error(request.id, "Missing nodeId parameter".to_string())
                    }
                }
            }
            "Runtime.evaluate" => {
                let expr = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("expression"))
                    .and_then(serde_json::Value::as_str);

                expr.map_or_else(
                    || CdpResponse::error(request.id, "Missing expression parameter".to_string()),
                    |code| {
                        let mut runtime = JsRuntime::new();
                        match runtime.eval(code) {
                            Ok(val) => CdpResponse::success(
                                request.id,
                                json!({ "result": { "value": val } }),
                            ),
                            Err(err) => CdpResponse::error(request.id, err.to_string()),
                        }
                    },
                )
            }
            "Page.navigate" => {
                let url = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("url"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("about:blank");
                CdpResponse::success(
                    request.id,
                    json!({ "frameId": "main", "loaderId": "1", "url": url }),
                )
            }
            "Page.reload" => CdpResponse::success(request.id, json!({ "reloaded": true })),
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

    /// Dispatches and processes an incoming CDP request that may mutate DOM state.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn handle_request_mut(
        &mut self,
        request: &CdpRequest,
        mut document: Option<&mut Document>,
    ) -> CdpResponse {
        match request.method.as_str() {
            "DOM.setAttributeValue" => {
                let p = request.params.as_ref();
                let node_id = p
                    .and_then(|v| v.get("nodeId"))
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| NodeId(n as usize));
                let name = p
                    .and_then(|v| v.get("name"))
                    .and_then(serde_json::Value::as_str);
                let value = p
                    .and_then(|v| v.get("value"))
                    .and_then(serde_json::Value::as_str);

                match (document.as_deref_mut(), node_id, name, value) {
                    (Some(doc), Some(nid), Some(k), Some(v)) => {
                        doc.set_attribute(nid, k, v);
                        CdpResponse::success(request.id, json!({ "success": true }))
                    }
                    (None, _, _, _) => {
                        CdpResponse::error(request.id, "No active document".to_string())
                    }
                    _ => CdpResponse::error(request.id, "Invalid parameters".to_string()),
                }
            }
            "DOM.removeAttribute" => {
                let p = request.params.as_ref();
                let node_id = p
                    .and_then(|v| v.get("nodeId"))
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| NodeId(n as usize));
                let name = p
                    .and_then(|v| v.get("name"))
                    .and_then(serde_json::Value::as_str);

                match (document.as_deref_mut(), node_id, name) {
                    (Some(doc), Some(nid), Some(k)) => {
                        doc.remove_attribute(nid, k);
                        CdpResponse::success(request.id, json!({ "success": true }))
                    }
                    (None, _, _) => {
                        CdpResponse::error(request.id, "No active document".to_string())
                    }
                    _ => CdpResponse::error(request.id, "Invalid parameters".to_string()),
                }
            }
            "DOM.removeNode" => {
                let node_id = request
                    .params
                    .as_ref()
                    .and_then(|v| v.get("nodeId"))
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| NodeId(n as usize));

                match (document.as_deref_mut(), node_id) {
                    (Some(doc), Some(nid)) => {
                        let parent_opt = doc.get_node(nid).and_then(|n| n.parent);
                        parent_opt.map_or_else(
                            || {
                                CdpResponse::error(
                                    request.id,
                                    "Cannot remove root node".to_string(),
                                )
                            },
                            |parent| {
                                doc.remove_child(parent, nid);
                                CdpResponse::success(request.id, json!({ "success": true }))
                            },
                        )
                    }
                    (None, _) => CdpResponse::error(request.id, "No active document".to_string()),
                    (_, None) => {
                        CdpResponse::error(request.id, "Missing nodeId parameter".to_string())
                    }
                }
            }
            _ => self.handle_request(request, document.as_deref()),
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

fn get_computed_styles(doc: &Document, node_id: NodeId) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if let Some(node) = doc.get_node(node_id)
        && let NodeData::Element(elem) = &node.data
    {
        if let Some(style) = elem.attributes.get("style") {
            for decl in style.split(';') {
                let mut parts = decl.splitn(2, ':');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    out.push(json!({
                        "name": k.trim(),
                        "value": v.trim()
                    }));
                }
            }
        }
        if out.is_empty() {
            out.push(json!({ "name": "display", "value": "block" }));
            out.push(json!({ "name": "box-sizing", "value": "border-box" }));
        }
    }
    out
}
