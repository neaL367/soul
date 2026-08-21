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

#[test]
fn test_cdp_dom_mutations_and_outer_html() {
    let mut server = CdpServer::new();
    let mut doc = Document::new();
    let root = doc.root_id();
    let div = doc.create_element("div");
    doc.set_attribute(div, "id", "box");
    doc.append_child(root, div);

    // Test getOuterHTML
    let html_req = CdpRequest {
        id: 20,
        method: "DOM.getOuterHTML".to_string(),
        params: Some(serde_json::json!({ "nodeId": div.0 })),
    };
    let html_resp = server.handle_request(&html_req, Some(&doc));
    assert!(
        html_resp
            .result
            .unwrap()
            .get("outerHTML")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("<div id=\"box\">")
    );

    // Test setAttributeValue
    let set_attr_req = CdpRequest {
        id: 21,
        method: "DOM.setAttributeValue".to_string(),
        params: Some(serde_json::json!({ "nodeId": div.0, "name": "data-test", "value": "123" })),
    };
    let set_resp = server.handle_request_mut(&set_attr_req, Some(&mut doc));
    assert!(set_resp.error.is_none());
    if let Some(node) = doc.get_node(div)
        && let dom::NodeData::Element(elem) = &node.data
    {
        assert_eq!(elem.attributes.get("data-test"), Some(&"123".to_string()));
    } else {
        panic!("expected element node");
    }

    // Test removeAttribute
    let rem_attr_req = CdpRequest {
        id: 22,
        method: "DOM.removeAttribute".to_string(),
        params: Some(serde_json::json!({ "nodeId": div.0, "name": "data-test" })),
    };
    let rem_resp = server.handle_request_mut(&rem_attr_req, Some(&mut doc));
    assert!(rem_resp.error.is_none());
    if let Some(node) = doc.get_node(div)
        && let dom::NodeData::Element(elem) = &node.data
    {
        assert_eq!(elem.attributes.get("data-test"), None);
    }

    // Test removeNode
    let rem_node_req = CdpRequest {
        id: 23,
        method: "DOM.removeNode".to_string(),
        params: Some(serde_json::json!({ "nodeId": div.0 })),
    };
    let rem_node_resp = server.handle_request_mut(&rem_node_req, Some(&mut doc));
    assert!(rem_node_resp.error.is_none());
    assert_eq!(doc.children(root).len(), 0);
}

#[test]
fn test_cdp_runtime_and_page_domains() {
    let mut server = CdpServer::new();

    // Runtime.evaluate
    let eval_req = CdpRequest {
        id: 30,
        method: "Runtime.evaluate".to_string(),
        params: Some(serde_json::json!({ "expression": "20 + 22" })),
    };
    let eval_resp = server.handle_request(&eval_req, None);
    assert_eq!(eval_resp.id, 30);
    assert!(
        eval_resp
            .result
            .unwrap()
            .get("result")
            .unwrap()
            .get("value")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("42")
    );

    // Page.navigate
    let nav_req = CdpRequest {
        id: 31,
        method: "Page.navigate".to_string(),
        params: Some(serde_json::json!({ "url": "https://example.com" })),
    };
    let nav_resp = server.handle_request(&nav_req, None);
    assert_eq!(nav_resp.id, 31);
    assert_eq!(
        nav_resp
            .result
            .unwrap()
            .get("url")
            .unwrap()
            .as_str()
            .unwrap(),
        "https://example.com"
    );
}
