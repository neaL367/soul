//! Integration tests for JavaScript `fetch()` Promise bindings and microtask resolution.

use boa_engine::{
    JsArgs, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
};
use javascript::JsRuntime;
use std::sync::{Arc, Mutex};
use web_api::register_fetch;

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

    // Hook a global callback to capture promise resolution
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

    // Draining microtasks resolves the Promise reactions
    runtime.drain_microtasks().expect("Drain microtasks failed");

    let captured = result_cell.lock().unwrap().clone();
    assert_eq!(captured, r#"{"status": "ok"}"#);
}
