//! Integration tests for W3C `Blob` and `File` APIs.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_blob_construction_and_slice() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const blob = new Blob(["Hello, ", "World!"], { type: "text/plain" });
        const sliced = blob.slice(0, 5, "text/plain");

        blob.size === 13 &&
        blob.type === "text/plain" &&
        blob.text() === "Hello, World!" &&
        sliced.size === 5 &&
        sliced.text() === "Hello";
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn test_file_metadata_properties() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const file = new File(["payload data"], "test.txt", { type: "text/plain", lastModified: 1700000000000 });
        file.name === "test.txt" &&
        file.size === 12 &&
        file.type === "text/plain" &&
        file.lastModified === 1700000000000 &&
        file.text() === "payload data";
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}
