//! Integration tests for `JsRuntime`, event loop, and microtask scheduling.

use boa_engine::Source;
use javascript::{JsRuntime, MAX_SCRIPT_BYTES};

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

#[test]
fn test_runtime_run_until_idle() {
    let mut runtime = JsRuntime::new();
    runtime.eval("globalThis.acc = 0;").unwrap();
    runtime.enqueue_task(|ctx| {
        let _ = ctx.eval(Source::from_bytes(b"globalThis.acc += 1;"));
    });
    runtime.enqueue_task(|ctx| {
        let _ = ctx.eval(Source::from_bytes(b"globalThis.acc += 2;"));
    });
    assert_eq!(runtime.pending_task_count(), 2);
    runtime.run_until_idle().unwrap();
    assert_eq!(runtime.pending_task_count(), 0);
    assert_eq!(runtime.eval("globalThis.acc").unwrap(), "3");
}

#[test]
fn test_microtask_budget_bounds_runaway_promise_chains() {
    // A promise reaction that enqueues itself would never terminate under
    // Boa's default unbounded executor; the bounded executor must fail the
    // drain (here reached through `eval`'s internal microtask drain) once the
    // per-drain budget is consumed.
    let mut runtime = JsRuntime::new();
    let err = runtime
        .eval(
            "
            globalThis.ticks = 0;
            function loop() { globalThis.ticks += 1; Promise.resolve().then(loop); }
            loop();
        ",
        )
        .expect_err("runaway microtask chain must be stopped by the budget");
    assert!(
        err.to_string().contains("microtask budget exceeded"),
        "unexpected error: {err}"
    );

    // The runtimes is still usable after the interrupted drain.
    assert_eq!(runtime.eval("1 + 1").unwrap(), "2");
}

#[test]
fn test_oversized_scripts_are_rejected_before_parsing() {
    let mut runtime = JsRuntime::new();

    let oversized = "var padding = 1;".repeat((MAX_SCRIPT_BYTES / 15) + 1);
    assert!(oversized.len() > MAX_SCRIPT_BYTES);

    let err = runtime
        .eval(&oversized)
        .expect_err("oversized script must be rejected");
    assert!(
        err.to_string().contains("exceeds the maximum"),
        "unexpected error: {err}"
    );

    // The runtime remains usable afterwards.
    assert_eq!(runtime.eval("1 + 1").unwrap(), "2");
}
