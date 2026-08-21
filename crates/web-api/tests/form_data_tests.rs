//! Integration tests for WHATWG `FormData` API.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_form_data_crud_operations() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const fd = new FormData();
        fd.append("username", "alice");
        fd.append("role", "admin");
        fd.append("role", "editor");

        const hasUser = fd.has("username");
        const user = fd.get("username");
        const roles = fd.getAll("role");

        fd.set("role", "superuser");
        const updatedRole = fd.get("role");

        fd.delete("username");
        const hasDeleted = fd.has("username");

        hasUser === true &&
        user === "alice" &&
        roles.length === 2 &&
        roles[0] === "admin" &&
        roles[1] === "editor" &&
        updatedRole === "superuser" &&
        hasDeleted === false;
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}
