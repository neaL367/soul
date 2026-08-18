//! Integration tests for CDP command handling, DOM inspection, and network monitoring.

use devtools::{CdpRequest, CdpServer};
use dom::{Document, ElementData, NodeData};
use std::collections::HashMap;

#[test]
fn test_cdp_dom_inspection_request() {
    let mut server = CdpServer::new();

    let mut doc = Document::new();
    let root = doc.root_id();
    let mut attrs = HashMap::new();
    attrs.insert("class".to_string(), "main-view".to_string());

    let body = doc.alloc_node(NodeData::Element(ElementData::new("body", attrs)));
    doc.append_child(root, body);

    let req = CdpRequest {
        id: 1,
        method: "DOM.getDocument".to_string(),
        params: None,
    };

    let resp = server.handle_request(&req, Some(&doc));
    assert_eq!(resp.id, 1);
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());

    let res_str = resp.result.unwrap().to_string();
    assert!(res_str.contains("main-view"));
    assert!(res_str.contains("body"));
}

#[test]
fn test_cdp_network_and_console_queries() {
    let mut server = CdpServer::new();

    // Record network
    server.network.record_request(
        42,
        "https://example.com/api/data".to_string(),
        "GET".to_string(),
    );
    server.network.record_response(42, 200, 1024);

    // Record console
    server
        .console
        .log("info", "Application loaded successfully");

    // Query network via CDP
    let net_req = CdpRequest {
        id: 2,
        method: "Network.getEvents".to_string(),
        params: None,
    };
    let net_resp = server.handle_request(&net_req, None);
    assert_eq!(net_resp.id, 2);
    let net_str = net_resp.result.unwrap().to_string();
    assert!(net_str.contains("https://example.com/api/data"));
    assert!(net_str.contains("200"));

    // Query console via CDP
    let con_req = CdpRequest {
        id: 3,
        method: "Console.getMessages".to_string(),
        params: None,
    };
    let con_resp = server.handle_request(&con_req, None);
    assert_eq!(con_resp.id, 3);
    let con_str = con_resp.result.unwrap().to_string();
    assert!(con_str.contains("Application loaded successfully"));
}

#[test]
fn test_cdp_query_selector_and_clearing() {
    let mut server = CdpServer::new();
    let mut doc = Document::new();
    let root = doc.root_id();
    let mut attrs = HashMap::new();
    attrs.insert("class".to_string(), "target".to_string());
    let p = doc.alloc_node(NodeData::Element(ElementData::new("p", attrs)));
    doc.append_child(root, p);

    let query_req = CdpRequest {
        id: 10,
        method: "DOM.querySelector".to_string(),
        params: Some(serde_json::json!({ "selector": ".target" })),
    };

    let query_resp = server.handle_request(&query_req, Some(&doc));
    assert_eq!(query_resp.id, 10);
    assert!(query_resp.error.is_none());
    let res = query_resp.result.unwrap();
    assert_ne!(res.get("nodeId").unwrap().as_u64().unwrap(), 0);

    // Test clear
    server.console.log("warn", "Test warning");
    assert_eq!(server.console.get_messages().len(), 1);

    let clear_req = CdpRequest {
        id: 11,
        method: "Console.clearMessages".to_string(),
        params: None,
    };
    let clear_resp = server.handle_request(&clear_req, None);
    assert_eq!(clear_resp.id, 11);
    assert_eq!(server.console.get_messages().len(), 0);
}
