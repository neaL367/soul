//! Integration tests for WHATWG `btoa` and `atob` bindings.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_btoa_and_atob_roundtrip() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");

    let script = r#"
        let original = "Hello World! 123";
        let encoded = btoa(original);
        let decoded = atob(encoded);
        ({ encoded, decoded, match: decoded === original })
    "#;

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    let obj = res.as_object().unwrap();

    let encoded_str = obj
        .get(boa_engine::js_string!("encoded"), &mut context)
        .unwrap()
        .to_string(&mut context)
        .unwrap()
        .to_std_string_escaped();
    let is_match = obj
        .get(boa_engine::js_string!("match"), &mut context)
        .unwrap()
        .to_boolean();

    assert_eq!(encoded_str, "SGVsbG8gV29ybGQhIDEyMw==");
    assert!(is_match);
}

#[test]
fn test_btoa_rejects_out_of_range_characters() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");

    let script = r#"
        let threw = false;
        try {
            btoa("Hello \u{1F600}"); // Emoji outside Latin1 range
        } catch (e) {
            threw = true;
        }
        threw
    "#;

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    assert!(res.to_boolean());
}
