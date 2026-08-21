//! Integration tests for WHATWG `AbortController` and `AbortSignal` bindings.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_abort_controller_signals_and_listener() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");

    let script = r#"
        let controller = new AbortController();
        let signal = controller.signal;
        let listenerCalled = false;

        signal.addEventListener("abort", () => {
            listenerCalled = true;
        });

        let beforeAbort = signal.aborted;
        controller.abort("custom reason");
        let afterAbort = signal.aborted;

        ({ beforeAbort, afterAbort, listenerCalled })
    "#;

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    let obj = res.as_object().unwrap();

    let before = obj
        .get(boa_engine::js_string!("beforeAbort"), &mut context)
        .unwrap()
        .to_boolean();
    let after = obj
        .get(boa_engine::js_string!("afterAbort"), &mut context)
        .unwrap()
        .to_boolean();
    let listener_called = obj
        .get(boa_engine::js_string!("listenerCalled"), &mut context)
        .unwrap()
        .to_boolean();

    assert!(!before);
    assert!(after);
    assert!(listener_called);
}

#[test]
fn test_abort_signal_static_abort_and_throw() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");

    let script = r#"
        let signal = AbortSignal.abort("custom cancel");
        let threw = false;
        try {
            signal.throwIfAborted();
        } catch (e) {
            threw = true;
        }
        ({ aborted: signal.aborted, threw })
    "#;

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    let obj = res.as_object().unwrap();

    let aborted = obj
        .get(boa_engine::js_string!("aborted"), &mut context)
        .unwrap()
        .to_boolean();
    let threw = obj
        .get(boa_engine::js_string!("threw"), &mut context)
        .unwrap()
        .to_boolean();

    assert!(aborted);
    assert!(threw);
}
