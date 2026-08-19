//! Integration tests for Web API bindings (`console`, `document`, `setTimeout`).

use html::parse_html;
use javascript::JsRuntime;
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
    let timer_queue: TimerQueue = boa_engine::gc::Gc::new(boa_engine::gc::GcRefCell::new(
        web_api::TimerState::default(),
    ));

    bind_web_apis(&mut runtime.context, None, None, Some(timer_queue.clone()))
        .expect("bind_web_apis failed");

    runtime
        .eval("setTimeout(() => { return 42; }, 50);")
        .expect("eval failed");

    let count = timer_queue.borrow().len();
    assert_eq!(count, 1);
}

#[test]
fn test_set_timeout_ids_unique_and_clear_timeout_removes() {
    let mut runtime = JsRuntime::new();
    let timer_queue: TimerQueue = boa_engine::gc::Gc::new(boa_engine::gc::GcRefCell::new(
        web_api::TimerState::default(),
    ));

    bind_web_apis(&mut runtime.context, None, None, Some(timer_queue.clone()))
        .expect("bind_web_apis failed");

    runtime
        .eval("const id1 = setTimeout(() => 1, 10); const id2 = setTimeout(() => 2, 20);")
        .expect("eval failed");

    let (id1, id2) = {
        let js_id1 = runtime.eval("id1").expect("eval id1");
        let js_id2 = runtime.eval("id2").expect("eval id2");
        (
            js_id1.trim_matches('"').parse::<u64>().unwrap(),
            js_id2.trim_matches('"').parse::<u64>().unwrap(),
        )
    };
    assert_eq!(id1, 0);
    assert_eq!(id2, 1);

    runtime
        .eval("clearTimeout(id1);")
        .expect("eval clearTimeout failed");

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
    let protocol = runtime.eval("location.protocol").expect("eval protocol");
    let host = runtime.eval("location.host").expect("eval host");
    let hostname = runtime.eval("location.hostname").expect("eval hostname");
    let pathname = runtime.eval("location.pathname").expect("eval pathname");
    let search = runtime.eval("location.search").expect("eval search");
    let user_agent = runtime.eval("navigator.userAgent").expect("eval userAgent");
    let language = runtime.eval("navigator.language").expect("eval language");
    let on_line = runtime.eval("navigator.onLine").expect("eval onLine");

    assert_eq!(
        href.trim_matches('"'),
        "https://docs.rs/hyper/1.0.0/hyper/index.html?search=client#top"
    );
    assert_eq!(origin.trim_matches('"'), "https://docs.rs");
    assert_eq!(protocol.trim_matches('"'), "https:");
    assert_eq!(host.trim_matches('"'), "docs.rs");
    assert_eq!(hostname.trim_matches('"'), "docs.rs");
    assert_eq!(pathname.trim_matches('"'), "/hyper/1.0.0/hyper/index.html");
    assert_eq!(search.trim_matches('"'), "?search=client");
    assert!(user_agent.contains("Soul/"));
    assert_eq!(language.trim_matches('"'), "en-US");
    assert_eq!(on_line, "true");
}
