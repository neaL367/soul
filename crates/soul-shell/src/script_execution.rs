//! Script execution for the shell rendering pipeline.

use dom::{Document, NodeId};
use javascript::JsRuntime;
use networking::{CspDirective, CspPolicy, HttpRequest, NetworkClient};
use soul_core::NavigationError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use storage::{LocalStorage, SessionStorage, StorageDatabase};
use url::Url;
use web_api::{
    FetchRequest, FetchResponse, RichFetchHandler, bind_web_apis, register_local_storage,
    register_rich_fetch, register_session_storage,
};

/// Creates a rich fetch callback handler for script execution in the shell.
fn create_shell_fetch_handler(
    client: NetworkClient,
    doc_url: Url,
    csp: Vec<CspPolicy>,
) -> RichFetchHandler {
    Arc::new(move |req: &FetchRequest| {
        let target_url = doc_url.join(&req.url).map_err(|e| e.to_string())?;

        // Enforce Content Security Policy (connect-src) on outgoing fetch requests.
        if csp
            .iter()
            .any(|p| !p.allows(CspDirective::ConnectSrc, &target_url, &doc_url))
        {
            return Err(format!(
                "fetch to {target_url} blocked by Content Security Policy (connect-src)"
            ));
        }

        let method = match req.method.to_ascii_uppercase().as_str() {
            "POST" => networking::types::HttpMethod::Post,
            "HEAD" => networking::types::HttpMethod::Head,
            "PUT" => networking::types::HttpMethod::Put,
            "DELETE" => networking::types::HttpMethod::Delete,
            _ => networking::types::HttpMethod::Get,
        };
        let request = HttpRequest {
            url: target_url,
            method,
            headers: req.headers.clone(),
            body: req.body.clone().map(bytes::Bytes::from),
        };
        let client = client.clone();
        let doc_url = doc_url.clone();

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

        let is_success = response.is_success();
        let status_code = response.status_code;
        let response_url = response.url.to_string();
        let body_bytes = response.body.to_vec();
        let headers_vec: Vec<(String, String)> = response.headers.into_iter().collect();

        Ok(FetchResponse {
            status: status_code,
            status_text: if is_success {
                "OK".to_string()
            } else {
                "Error".to_string()
            },
            headers: headers_vec,
            body: body_bytes,
            url: response_url,
        })
    })
}

/// Executes inline scripts against a parsed document before style and layout.
///
/// # Errors
///
/// Returns `NavigationError` only if the in-process DOM handoff fails.
#[allow(dead_code)]
pub fn execute_inline_scripts(
    document: Document,
    document_url: Option<&Url>,
    client: Option<&NetworkClient>,
) -> Result<Document, NavigationError> {
    execute_scripts(document, document_url, client, None, &[])
}

/// Executes all scripts (both inline and external) in document order.
///
/// # Errors
///
/// Returns `NavigationError` only if the in-process DOM handoff fails.
pub fn execute_scripts(
    document: Document,
    document_url: Option<&Url>,
    client: Option<&NetworkClient>,
    external_scripts: Option<&HashMap<NodeId, String>>,
    csp: &[CspPolicy],
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
                let allowed = if csp.is_empty() {
                    true
                } else {
                    let node = document.get_node(script_id);
                    let nonce = node.and_then(|n| {
                        if let dom::NodeData::Element(e) = &n.data {
                            e.attr("nonce")
                        } else {
                            None
                        }
                    });
                    nonce.map_or_else(
                        || csp.iter().all(|p| p.allows_inline(CspDirective::ScriptSrc)),
                        |nonce_val| {
                            csp.iter()
                                .all(|p| p.allows_nonce(CspDirective::ScriptSrc, nonce_val))
                        },
                    )
                };

                if allowed {
                    scripts.push(inline_text);
                } else {
                    tracing::warn!("Blocked inline script by Content Security Policy (script-src)");
                }
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

    let current_url_str = document_url.map_or("about:blank", Url::as_str);
    let _ = web_api::register_window(&mut runtime.context, current_url_str);

    let origin =
        document_url.map_or_else(|| "null".to_string(), |u| u.origin().ascii_serialization());

    if let Ok(db) = StorageDatabase::open_in_memory() {
        let local_storage = Arc::new(LocalStorage::new(db));
        let _ = register_local_storage(&mut runtime.context, local_storage, &origin);
    }
    let session_storage = Arc::new(SessionStorage::new());
    let _ = register_session_storage(&mut runtime.context, session_storage, &origin);

    if let (Some(client), Some(doc_url)) = (client, document_url) {
        let fetch_handler =
            create_shell_fetch_handler(client.clone(), doc_url.clone(), csp.to_vec());
        let _ = register_rich_fetch(&mut runtime.context, fetch_handler);
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
