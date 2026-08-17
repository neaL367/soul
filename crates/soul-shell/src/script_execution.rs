//! Script execution for the shell rendering pipeline.

use dom::{Document, NodeId};
use javascript::JsRuntime;
use networking::{HttpClient, HttpRequest};
use soul_core::NavigationError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use storage::{LocalStorage, SessionStorage, StorageDatabase};
use url::Url;
use web_api::{
    FetchHandler, bind_web_apis, register_fetch, register_local_storage, register_session_storage,
};

/// Executes inline scripts against a parsed document before style and layout.
///
/// # Errors
///
/// Returns `NavigationError` only if the in-process DOM handoff fails.
pub fn execute_inline_scripts(
    document: Document,
    document_url: Option<&Url>,
    client: Option<&HttpClient>,
) -> Result<Document, NavigationError> {
    execute_scripts(document, document_url, client, None)
}

/// Executes all scripts (both inline and external) in document order.
///
/// # Errors
///
/// Returns `NavigationError` only if the in-process DOM handoff fails.
pub fn execute_scripts(
    document: Document,
    document_url: Option<&Url>,
    client: Option<&HttpClient>,
    external_scripts: Option<&HashMap<NodeId, String>>,
) -> Result<Document, NavigationError> {
    let mut scripts: Vec<String> = Vec::new();
    for script_id in document.get_elements_by_tag_name("script") {
        if let Some(ext) = external_scripts.and_then(|map| map.get(&script_id)) {
            if !ext.trim().is_empty() {
                scripts.push(ext.clone());
            }
        } else {
            let inline_text = document.text_content(script_id);
            if !inline_text.trim().is_empty() {
                scripts.push(inline_text);
            }
        }
    }
    if scripts.is_empty() {
        return Ok(document);
    }

    let document = Arc::new(Mutex::new(document));
    let mut runtime = JsRuntime::new();
    bind_web_apis(&mut runtime.context, Some(document.clone()), None, None)
        .map_err(|error| NavigationError::Other(format!("failed to bind Web APIs: {error}")))?;

    let origin =
        document_url.map_or_else(|| "null".to_string(), |u| u.origin().ascii_serialization());

    if let Ok(db) = StorageDatabase::open_in_memory() {
        let local_storage = Arc::new(LocalStorage::new(db));
        let _ = register_local_storage(&mut runtime.context, local_storage, &origin);
    }
    let session_storage = Arc::new(SessionStorage::new());
    let _ = register_session_storage(&mut runtime.context, session_storage, &origin);

    if let (Some(client), Some(doc_url)) = (client, document_url) {
        let client_clone = client.clone();
        let doc_url_clone = doc_url.clone();
        let fetch_handler: FetchHandler = Arc::new(move |req_url_str: &str| {
            let target_url = doc_url_clone.join(req_url_str).map_err(|e| e.to_string())?;
            let request = HttpRequest::get(target_url);
            let client = client_clone.clone();
            let doc_url = doc_url_clone.clone();

            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result = (|| -> Result<networking::HttpResponse, String> {
                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?;
                    let fetch_res = rt.block_on(async move {
                        client
                            .fetch_with_security_context(&request, Some(&doc_url))
                            .await
                            .map_err(|e| e.to_string())
                    });
                    rt.shutdown_background();
                    fetch_res
                })();
                let _ = tx.send(result);
            });

            let response = rx
                .recv()
                .map_err(|_| "fetch worker thread disconnected".to_string())??;

            String::from_utf8(response.body.to_vec()).map_err(|e| e.to_string())
        });
        let _ = register_fetch(&mut runtime.context, fetch_handler);
    }

    for (script_index, source) in scripts.iter().enumerate() {
        if let Err(error) = runtime.eval(source) {
            tracing::warn!(script_index, %error, "Inline script failed");
        }
        for _ in 0..10 {
            if let Err(error) = runtime.drain_microtasks() {
                tracing::warn!(script_index, %error, "Microtask drain failed");
                break;
            }
        }
    }
    drop(runtime);

    let mut doc_guard = document.lock().map_err(|error| {
        NavigationError::Other(format!("script document lock poisoned: {error}"))
    })?;
    Ok(std::mem::take(&mut *doc_guard))
}
