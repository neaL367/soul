//! Integration tests for Web API bindings (`console`, `document`, `setTimeout`).

use html::parse_html;
use javascript::JsRuntime;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use web_api::{TimerQueue, bind_web_apis};

#[test]
fn test_console_log_captured() {
    let mut runtime = JsRuntime::new();
    let logs = Arc::new(Mutex::new(Vec::new()));

    bind_web_apis(&mut runtime.context, None, Some(logs.clone()), None)
        .expect("bind_web_apis failed");

    runtime
        .eval("console.log('User logged in:', 42, true);")
        .expect("eval failed");

    let count = logs.lock().unwrap().len();
    assert_eq!(count, 1);
    let msg = logs.lock().unwrap()[0].clone();
    assert_eq!(msg, "User logged in: 42 true");
}

#[test]
fn test_dom_get_element_by_id_and_mutate_text() {
    let html = r#"<html><body>
        <h1 id="title">Initial Title</h1>
    </body></html>"#;
    let doc = Arc::new(Mutex::new(parse_html(html)));

    let mut runtime = JsRuntime::new();
    bind_web_apis(&mut runtime.context, Some(doc.clone()), None, None)
        .expect("bind_web_apis failed");

    let js = r"
        const el = document.getElementById('title');
        el.setTextContent('Mutated Title via JS');
    ";
    runtime.eval(js).expect("eval failed");

    let (text, dirty_layout, dirty_paint) = {
        let doc_guard = doc.lock().unwrap();
        let title_id = doc_guard.get_element_by_id("title").unwrap();
        let text = doc_guard.text_content(title_id);
        let node = doc_guard.get_node(title_id).unwrap();
        let dirty_l = node.dirty_flags.layout;
        let dirty_p = node.dirty_flags.paint;
        drop(doc_guard);
        (text, dirty_l, dirty_p)
    };

    assert_eq!(text, "Mutated Title via JS");
    assert!(dirty_layout);
    assert!(dirty_paint);
}

#[test]
fn test_set_timeout_callback_queued() {
    let mut runtime = JsRuntime::new();
    let timer_queue: TimerQueue = Rc::new(RefCell::new(Vec::new()));

    bind_web_apis(&mut runtime.context, None, None, Some(timer_queue.clone()))
        .expect("bind_web_apis failed");

    runtime
        .eval("setTimeout(() => { return 42; }, 50);")
        .expect("eval failed");

    let count = timer_queue.borrow().len();
    assert_eq!(count, 1);
}

#[test]
#[allow(clippy::significant_drop_tightening)]
fn test_dom_query_selector_and_class_list_manipulation() {
    let html = r#"<html><body>
        <div class="box card" id="card-1">Card Content</div>
    </body></html>"#;
    let doc = Arc::new(Mutex::new(parse_html(html)));

    let mut runtime = JsRuntime::new();
    bind_web_apis(&mut runtime.context, Some(doc.clone()), None, None)
        .expect("bind_web_apis failed");

    let js = r"
        const card = document.querySelector('.card');
        card.classList.add('highlighted');
        card.classList.remove('box');
        const hasHighlight = card.classList.contains('highlighted');
        const hasBox = card.classList.contains('box');
    ";
    runtime.eval(js).expect("eval failed");

    let (has_highlight, has_no_box, has_card) = {
        let doc_guard = doc.lock().unwrap();
        let card_id = doc_guard.get_element_by_id("card-1").unwrap();
        let elem = doc_guard.get_node(card_id).unwrap().as_element().unwrap();
        (
            elem.has_class("highlighted"),
            !elem.has_class("box"),
            elem.has_class("card"),
        )
    };

    assert!(has_highlight);
    assert!(has_no_box);
    assert!(has_card);
}

#[test]
#[allow(clippy::significant_drop_tightening)]
fn test_dom_set_get_remove_attribute() {
    let html = r#"<html><body><a id="link" href="https://example.com">Link</a></body></html>"#;
    let doc = Arc::new(Mutex::new(parse_html(html)));

    let mut runtime = JsRuntime::new();
    bind_web_apis(&mut runtime.context, Some(doc.clone()), None, None)
        .expect("bind_web_apis failed");

    let js = r"
        const link = document.getElementById('link');
        const oldHref = link.getAttribute('href');
        link.setAttribute('target', '_blank');
        link.removeAttribute('href');
    ";
    runtime.eval(js).expect("eval failed");

    let (target_attr, href_attr) = {
        let doc_guard = doc.lock().unwrap();
        let link_id = doc_guard.get_element_by_id("link").unwrap();
        let elem = doc_guard.get_node(link_id).unwrap().as_element().unwrap();
        (
            elem.attr("target").map(String::from),
            elem.attr("href").map(String::from),
        )
    };

    assert_eq!(target_attr.as_deref(), Some("_blank"));
    assert_eq!(href_attr, None);
}

#[test]
fn test_window_location_and_navigator_bindings() {
    let mut runtime = JsRuntime::new();
    web_api::register_window(
        &mut runtime.context,
        "https://docs.rs/hyper/1.0.0/hyper/index.html?search=client#top",
    )
    .expect("register_window failed");

    let href = runtime.eval("location.href").expect("eval href");
    let origin = runtime.eval("location.origin").expect("eval origin");
    let pathname = runtime.eval("location.pathname").expect("eval pathname");
    let search = runtime.eval("location.search").expect("eval search");
    let user_agent = runtime.eval("navigator.userAgent").expect("eval userAgent");

    assert_eq!(
        href.trim_matches('"'),
        "https://docs.rs/hyper/1.0.0/hyper/index.html?search=client#top"
    );
    assert_eq!(origin.trim_matches('"'), "https://docs.rs");
    assert_eq!(pathname.trim_matches('"'), "/hyper/1.0.0/hyper/index.html");
    assert_eq!(search.trim_matches('"'), "?search=client");
    assert!(user_agent.contains("Soul/"));
}
