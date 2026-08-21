//! JavaScript bindings for `window.location` and `window.navigator`.

use boa_engine::{Context, JsResult, js_string, object::ObjectInitializer, property::Attribute};
use url::Url;

/// Registers `location` and `navigator` properties onto the global object.
///
/// # Errors
///
/// Returns a `JsResult` error if property registration fails.
#[allow(clippy::too_many_lines)]
pub fn register_window(context: &mut Context, current_url: &str) -> JsResult<()> {
    let parsed = Url::parse(current_url).ok();
    let href = current_url.to_string();
    let origin = parsed.as_ref().map_or_else(
        || "null".to_string(),
        |u| u.origin().unicode_serialization(),
    );
    let protocol = parsed
        .as_ref()
        .map_or_else(|| "about:".to_string(), |u| format!("{}:", u.scheme()));
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .map_or_else(String::new, |h| {
            parsed
                .as_ref()
                .and_then(Url::port)
                .map_or_else(|| h.to_string(), |port| format!("{h}:{port}"))
        });
    let hostname = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or_default()
        .to_string();
    let port = parsed
        .as_ref()
        .and_then(Url::port)
        .map_or_else(String::new, |p| p.to_string());
    let pathname = parsed.as_ref().map_or("/", Url::path).to_string();
    let search = parsed
        .as_ref()
        .and_then(Url::query)
        .map_or_else(String::new, |q| format!("?{q}"));
    let hash = parsed
        .as_ref()
        .and_then(Url::fragment)
        .map_or_else(String::new, |f| format!("#{f}"));

    let location_obj = ObjectInitializer::new(context)
        .property(js_string!("href"), js_string!(href), Attribute::all())
        .property(
            js_string!("origin"),
            js_string!(origin),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("protocol"),
            js_string!(protocol),
            Attribute::all(),
        )
        .property(js_string!("host"), js_string!(host), Attribute::all())
        .property(
            js_string!("hostname"),
            js_string!(hostname),
            Attribute::all(),
        )
        .property(js_string!("port"), js_string!(port), Attribute::all())
        .property(
            js_string!("pathname"),
            js_string!(pathname),
            Attribute::all(),
        )
        .property(js_string!("search"), js_string!(search), Attribute::all())
        .property(js_string!("hash"), js_string!(hash), Attribute::all())
        .build();

    let clipboard_obj = crate::clipboard_binding::create_clipboard_object(context);

    let navigator_obj = ObjectInitializer::new(context)
        .property(
            js_string!("userAgent"),
            js_string!("Soul/0.1.0 (Windows NT 10.0; Win64; x64)"),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("platform"),
            js_string!("Win32"),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("language"),
            js_string!("en-US"),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("onLine"),
            true,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("cookieEnabled"),
            true,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("clipboard"),
            clipboard_obj,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .build();

    context.register_global_property(
        js_string!("location"),
        location_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
    )?;

    context.register_global_property(
        js_string!("navigator"),
        navigator_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
    )?;

    Ok(())
}
