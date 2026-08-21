//! Integration tests for W3C Web Cryptography API (`window.crypto`).

use boa_engine::{Context, Source};
use web_api::register_crypto;

#[test]
fn test_crypto_random_uuid_format() {
    let mut context = Context::default();
    register_crypto(&mut context);

    let script = "crypto.randomUUID()";
    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval randomUUID");

    let uuid_str = res.to_string(&mut context).unwrap().to_std_string_escaped();
    assert_eq!(uuid_str.len(), 36);
    assert_eq!(uuid_str.chars().filter(|&c| c == '-').count(), 4);

    // Verify two generated UUIDs are distinct
    let script2 = "crypto.randomUUID() !== crypto.randomUUID()";
    let res2 = context
        .eval(Source::from_bytes(script2.as_bytes()))
        .expect("eval distinct");
    assert!(res2.to_boolean());
}

#[test]
fn test_crypto_get_random_values() {
    let mut context = Context::default();
    register_crypto(&mut context);

    let script = r"
        const buf = new Uint8Array(16);
        crypto.getRandomValues(buf);
        let sum = 0;
        for (let i = 0; i < buf.length; i++) {
            sum += buf[i];
        }
        sum > 0;
    ";
    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval getRandomValues");
    assert!(res.to_boolean());
}
