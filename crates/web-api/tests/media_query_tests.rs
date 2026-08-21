//! Integration tests for W3C CSSOM View `window.matchMedia` and `MediaQueryList`.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_match_media_matches_and_media_properties() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const mql = window.matchMedia("(min-width: 600px)");
        mql.media === "(min-width: 600px)" && mql.matches === true;
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn test_match_media_dark_mode_query() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const mqlLight = matchMedia("(prefers-color-scheme: light)");
        const mqlDark = matchMedia("(prefers-color-scheme: dark)");
        mqlLight.matches === true && mqlDark.matches === false;
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}
