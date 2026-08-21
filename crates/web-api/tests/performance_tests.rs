//! Integration tests for Performance API and Animation Frame bindings.

use boa_engine::{Context, Source};
use web_api::register_performance;

#[test]
fn test_performance_now_monotonic() {
    let mut context = Context::default();
    register_performance(&mut context);

    let script = r"
        const t1 = performance.now();
        let sum = 0;
        for (let i = 0; i < 1000; i++) { sum += i; }
        const t2 = performance.now();
        t2 >= t1 && performance.timeOrigin > 0;
    ";

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval performance.now()");

    assert!(res.to_boolean());
}

#[test]
fn test_request_animation_frame_callback() {
    let mut context = Context::default();
    register_performance(&mut context);

    let script = r"
        let called = false;
        let frameTimestamp = 0;
        const id = requestAnimationFrame((ts) => {
            called = true;
            frameTimestamp = ts;
        });
        called && id > 0 && frameTimestamp >= 0;
    ";

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval requestAnimationFrame");

    assert!(res.to_boolean());
}
