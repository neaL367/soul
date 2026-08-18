//! Integration tests for JavaScript `fetch()`, `Headers`, `Request`, `Response` Promise bindings.

use boa_engine::{
    JsArgs, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
};
use javascript::JsRuntime;
use std::sync::{Arc, Mutex};
use web_api::{FetchRequest, FetchResponse, register_fetch, register_rich_fetch};

#[derive(Clone, Trace, Finalize)]
struct CallbackHolder(#[unsafe_ignore_trace] Arc<Mutex<String>>);

#[test]
fn test_js_fetch_promise_resolution() {
    let mut runtime = JsRuntime::new();
    let handler = Arc::new(|url: &str| {
        if url == "https://api.example.com/data" {
            Ok(r#"{"status": "ok"}"#.to_string())
        } else {
            Err("404 Not Found".to_string())
        }
    });

    register_fetch(&mut runtime.context, handler).expect("Register fetch failed");

    let result_cell = Arc::new(Mutex::new(String::new()));
    let result_holder = CallbackHolder(result_cell.clone());

    let cb = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let val = args.get_or_undefined(0).to_string(ctx)?;
            if let Ok(mut lock) = captures.0.lock() {
                *lock = val.to_std_string_escaped();
            }
            Ok(JsValue::undefined())
        },
        result_holder,
    );
    runtime
        .context
        .register_global_callable(js_string!("onFetched"), 1, cb)
        .unwrap();

    let js = r#"
        fetch("https://api.example.com/data")
            .then(res => res.text())
            .then(text => onFetched(text));
    "#;

    runtime.eval(js).expect("Eval failed");
    runtime.drain_microtasks().expect("Drain microtasks failed");

    let captured = result_cell.lock().unwrap().clone();
    assert_eq!(captured, r#"{"status": "ok"}"#);
}

#[test]
fn test_js_rich_fetch_json_and_headers() {
    let mut runtime = JsRuntime::new();
    let handler = Arc::new(|req: &FetchRequest| {
        if req.url == "https://api.example.com/user" {
            Ok(FetchResponse {
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![
                    ("content-type".to_string(), "application/json".to_string()),
                    ("x-custom-header".to_string(), "soul-engine".to_string()),
                ],
                body: br#"{"id": 42, "name": "Soul"}"#.to_vec(),
                url: req.url.clone(),
            })
        } else {
            Err("Not Found".to_string())
        }
    });

    register_rich_fetch(&mut runtime.context, handler).expect("Register rich fetch failed");

    let result_cell = Arc::new(Mutex::new(String::new()));
    let result_holder = CallbackHolder(result_cell.clone());

    let cb = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let val = args.get_or_undefined(0).to_string(ctx)?;
            if let Ok(mut lock) = captures.0.lock() {
                *lock = val.to_std_string_escaped();
            }
            Ok(JsValue::undefined())
        },
        result_holder,
    );
    runtime
        .context
        .register_global_callable(js_string!("onReport"), 1, cb)
        .unwrap();

    let js = r#"
        fetch("https://api.example.com/user")
            .then(res => {
                let ct = res.headers.get("Content-Type");
                let status = res.status;
                let ok = res.ok;
                return res.json().then(data => {
                    onReport(`${status}|${ok}|${ct}|${data.name}|${data.id}`);
                });
            });
    "#;

    runtime.eval(js).expect("Eval failed");
    runtime.drain_microtasks().expect("Drain microtasks failed");

    let captured = result_cell.lock().unwrap().clone();
    assert_eq!(captured, "200|true|application/json|Soul|42");
}

#[test]
fn test_js_headers_object_crud() {
    let mut runtime = JsRuntime::new();
    let handler = Arc::new(|req: &FetchRequest| Ok(FetchResponse::ok_text(&req.url, "ok")));
    register_rich_fetch(&mut runtime.context, handler).expect("Register rich fetch failed");

    let result_cell = Arc::new(Mutex::new(String::new()));
    let result_holder = CallbackHolder(result_cell.clone());

    let cb = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let val = args.get_or_undefined(0).to_string(ctx)?;
            if let Ok(mut lock) = captures.0.lock() {
                *lock = val.to_std_string_escaped();
            }
            Ok(JsValue::undefined())
        },
        result_holder,
    );
    runtime
        .context
        .register_global_callable(js_string!("onHeaders"), 1, cb)
        .unwrap();

    let js = r#"
        let h = new Headers();
        h.set("Accept", "text/html");
        h.set("Content-Type", "application/json");
        let hasAccept = h.has("accept");
        let getCt = h.get("content-type");
        h.delete("accept");
        let hasDeleted = h.has("accept");
        onHeaders(`${hasAccept}|${getCt}|${hasDeleted}`);
    "#;

    runtime.eval(js).expect("Eval failed");
    let captured = result_cell.lock().unwrap().clone();
    assert_eq!(captured, "true|application/json|false");
}
