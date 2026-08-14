//! Integration tests for `JsRuntime`, event loop, and microtask scheduling.

use boa_engine::Source;
use javascript::JsRuntime;

#[test]
fn test_runtime_eval_expression() {
    let mut runtime = JsRuntime::new();
    let result = runtime
        .eval("const a = 10; const b = 20; a + b")
        .expect("eval failed");
    assert_eq!(result, "30");
}

#[test]
fn test_runtime_promise_microtask_drain() {
    let mut runtime = JsRuntime::new();
    let code = r"
        globalThis.executionLog = [];
        Promise.resolve('promise_1').then(val => globalThis.executionLog.push(val));
        globalThis.executionLog.push('sync_1');
        Promise.resolve('promise_2').then(val => globalThis.executionLog.push(val));
        globalThis.executionLog.push('sync_2');
    ";
    runtime.eval(code).expect("eval failed");

    let result = runtime
        .eval("globalThis.executionLog.join(',')")
        .expect("eval failed");
    assert_eq!(result, "\"sync_1,sync_2,promise_1,promise_2\"");
}

#[test]
fn test_runtime_macrotask_step() {
    let mut runtime = JsRuntime::new();
    runtime
        .eval("globalThis.counter = 0;")
        .expect("init failed");

    runtime.enqueue_task(|ctx| {
        let _ = ctx.eval(Source::from_bytes(b"globalThis.counter += 10;"));
    });

    assert_eq!(runtime.pending_task_count(), 1);
    assert_eq!(runtime.eval("globalThis.counter").unwrap(), "0");

    let ran = runtime.step().expect("step failed");
    assert!(ran);
    assert_eq!(runtime.pending_task_count(), 0);
    assert_eq!(runtime.eval("globalThis.counter").unwrap(), "10");
}
