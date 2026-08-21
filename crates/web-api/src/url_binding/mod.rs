//! WHATWG `URL` and `URLSearchParams` ECMAScript bindings.

pub mod encoding;
pub mod search_params;
pub mod url;

pub use encoding::{urlencoding_decode, urlencoding_encode};
pub use search_params::{
    SearchParamsHolder, create_url_search_params_constructor, create_url_search_params_object,
};
pub use url::{UrlHolder, create_url_constructor, create_url_object};

use boa_engine::property::Attribute;
use boa_engine::{Context, JsResult, js_string};

/// Registers standard `URL` and `URLSearchParams` constructors in the global scope.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn register_url(ctx: &mut Context) -> JsResult<()> {
    let url_fn = create_url_constructor(ctx);
    ctx.register_global_property(
        js_string!("URL"),
        url_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    let params_fn = create_url_search_params_constructor(ctx);
    ctx.register_global_property(
        js_string!("URLSearchParams"),
        params_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
