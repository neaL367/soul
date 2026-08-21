//! Integration tests for `navigator.clipboard`.

use boa_engine::{Context, Source};
use web_api::{bind_web_apis, register_window};

#[test]
fn test_navigator_clipboard_write_and_read() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");
    register_window(&mut context, "https://example.com").expect("register_window failed");

    let script = r#"
        globalThis.readResult = null;
        navigator.clipboard.writeText("Copied Token 987");
        navigator.clipboard.readText().then(val => {
            globalThis.readResult = val;
        });
    "#;

    context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    let _ = context.run_jobs();

    let check_script = "globalThis.readResult";
    let res = context
        .eval(Source::from_bytes(check_script.as_bytes()))
        .expect("check eval failed");
    let res_str = res.to_string(&mut context).unwrap().to_std_string_escaped();
    assert_eq!(res_str, "Copied Token 987");
}
