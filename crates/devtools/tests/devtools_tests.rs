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
