//! Integration tests for `window.history` pushState and replaceState.

use boa_engine::{Context, Source};
use web_api::{bind_web_apis, register_window};

#[test]
fn test_history_push_state_and_replace_state() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");
    register_window(&mut context, "https://example.com/page1").expect("register_window failed");
    web_api::history_binding::register_history(&mut context, "https://example.com/page1");

    let script = r#"
        let initLen = history.length;
        history.pushState({ page: 2 }, "Page 2", "/page2?tab=info#section");
        let pushedLen = history.length;
        let pushedState = history.state.page;
        let newHref = location.href;
        let newSearch = location.search;

        history.replaceState({ page: 3 }, "Page 3", "/page3");
        let replacedLen = history.length;
        let replacedState = history.state.page;

        ({ initLen, pushedLen, pushedState, newHref, newSearch, replacedLen, replacedState })
    "#;

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval failed");
    let obj = res.as_object().unwrap();

    let init_len = obj
        .get(boa_engine::js_string!("initLen"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap();
    let pushed_len = obj
        .get(boa_engine::js_string!("pushedLen"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap();
    let pushed_state = obj
        .get(boa_engine::js_string!("pushedState"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap();
    let new_href = obj
        .get(boa_engine::js_string!("newHref"), &mut context)
        .unwrap()
        .to_string(&mut context)
        .unwrap()
        .to_std_string_escaped();
    let new_search = obj
        .get(boa_engine::js_string!("newSearch"), &mut context)
        .unwrap()
        .to_string(&mut context)
        .unwrap()
        .to_std_string_escaped();
    let replaced_len = obj
        .get(boa_engine::js_string!("replacedLen"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap();
    let replaced_state = obj
        .get(boa_engine::js_string!("replacedState"), &mut context)
        .unwrap()
        .to_u32(&mut context)
        .unwrap();

    assert_eq!(init_len, 1);
    assert_eq!(pushed_len, 2);
    assert_eq!(pushed_state, 2);
    assert_eq!(new_href, "https://example.com/page2?tab=info#section");
    assert_eq!(new_search, "?tab=info");
    assert_eq!(replaced_len, 2);
    assert_eq!(replaced_state, 3);
}

#[test]
fn test_history_cross_origin_push_fails() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind_web_apis failed");
    register_window(&mut context, "https://example.com/app").expect("register_window failed");
    web_api::history_binding::register_history(&mut context, "https://example.com/app");

    let script = r#"
        let threw = false;
        try {
            history.pushState(null, "", "https://attacker.com/evil");
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
