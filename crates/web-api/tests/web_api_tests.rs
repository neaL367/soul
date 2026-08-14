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
