//! Inline-script execution for the shell rendering pipeline.

use dom::Document;
use javascript::JsRuntime;
use soul_core::NavigationError;
use std::sync::{Arc, Mutex};
use web_api::bind_web_apis;

/// Executes inline scripts against a parsed document before style and layout.
///
/// Script evaluation failures are isolated to the offending script and logged,
/// matching browser behavior where a page can continue after a script error.
///
/// # Errors
///
/// Returns `NavigationError` only if the in-process DOM handoff fails.
pub fn execute_inline_scripts(document: Document) -> Result<Document, NavigationError> {
    let scripts: Vec<String> = document
        .get_elements_by_tag_name("script")
        .into_iter()
        .map(|node_id| document.text_content(node_id))
        .filter(|source| !source.trim().is_empty())
        .collect();
    if scripts.is_empty() {
        return Ok(document);
    }

    let document = Arc::new(Mutex::new(document));
    let mut runtime = JsRuntime::new();
    bind_web_apis(&mut runtime.context, Some(document.clone()), None, None)
        .map_err(|error| NavigationError::Other(format!("failed to bind Web APIs: {error}")))?;

    for (script_index, source) in scripts.iter().enumerate() {
        if let Err(error) = runtime.eval(source) {
            tracing::warn!(script_index, %error, "Inline script failed");
        }
    }
    drop(runtime);

    let mut doc_guard = document.lock().map_err(|error| {
        NavigationError::Other(format!("script document lock poisoned: {error}"))
    })?;
    Ok(std::mem::take(&mut *doc_guard))
}
