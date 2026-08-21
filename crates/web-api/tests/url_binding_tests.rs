//! Integration tests for WHATWG `URL` and `URLSearchParams` bindings.

use boa_engine::{Context, Source};
use web_api::register_url;

#[test]
fn test_url_parsing_and_properties() {
    let mut context = Context::default();
    register_url(&mut context).unwrap();

    let script = r"
        const url = new URL('https://user:pass@example.com:8080/path/to/page?query=123#heading');
        [
            url.href,
            url.origin,
            url.protocol,
            url.host,
            url.hostname,
            url.port,
            url.pathname,
            url.search,
            url.hash
        ];
    ";

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval url properties");

    let arr = res.as_object().expect("array result");
    assert_eq!(
        arr.get(0, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "https://user:pass@example.com:8080/path/to/page?query=123#heading"
    );
    assert_eq!(
        arr.get(1, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "https://example.com:8080"
    );
    assert_eq!(
        arr.get(2, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "https:"
    );
    assert_eq!(
        arr.get(3, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "example.com:8080"
    );
    assert_eq!(
        arr.get(4, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "example.com"
    );
    assert_eq!(
        arr.get(5, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "8080"
    );
    assert_eq!(
        arr.get(6, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "/path/to/page"
    );
    assert_eq!(
        arr.get(7, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "?query=123"
    );
    assert_eq!(
        arr.get(8, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "#heading"
    );
}

#[test]
fn test_url_search_params_crud() {
    let mut context = Context::default();
    register_url(&mut context).unwrap();

    let script = r"
        const params = new URLSearchParams('?q=rust&sort=desc&tag=web&tag=gpu');
        params.append('page', '2');
        params.set('sort', 'asc');
        params.delete('q');

        [
            params.get('sort'),
            params.getAll('tag').join(','),
            params.has('page'),
            params.has('q'),
            params.size
        ];
    ";

    let res = context
        .eval(Source::from_bytes(script.as_bytes()))
        .expect("eval params crud");

    let arr = res.as_object().expect("array result");
    assert_eq!(
        arr.get(0, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "asc"
    );
    assert_eq!(
        arr.get(1, &mut context)
            .unwrap()
            .to_string(&mut context)
            .unwrap()
            .to_std_string_escaped(),
        "web,gpu"
    );
    assert!(arr.get(2, &mut context).unwrap().to_boolean());
    assert!(!arr.get(3, &mut context).unwrap().to_boolean());
    assert_eq!(
        arr.get(4, &mut context)
            .unwrap()
            .to_u32(&mut context)
            .unwrap(),
        4
    );
}
