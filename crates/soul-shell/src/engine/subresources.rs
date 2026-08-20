//! Subresource fetching and decoding: images, external stylesheets, and external scripts.

use dom::{Document, NodeData, NodeId};
use image_decode::{DecodedImage, ImageDecoder};
use networking::{CspDirective, CspPolicy, HttpRequest, NetworkClient};
use std::collections::HashMap;
use url::Url;

/// Fetches and decodes every `<img>` subresource through the security-checked
/// client path (mixed content + CORS + CSP enforced against the document origin).
pub(super) async fn load_subresource_images(
    client: &NetworkClient,
    document_url: &Url,
    doc: &Document,
    csp: Option<&CspPolicy>,
) -> HashMap<NodeId, DecodedImage> {
    let mut images = HashMap::new();

    for img_id in doc.get_elements_by_tag_name("img") {
        let Some(node) = doc.get_node(img_id) else {
            continue;
        };
        let NodeData::Element(element) = &node.data else {
            continue;
        };
        let Some(src) = element.attr("src") else {
            continue;
        };
        let Ok(url) = document_url.join(src) else {
            tracing::warn!(src, "Skipping image with unresolvable src");
            continue;
        };

        if let Some(policy) = csp
            && !policy.allows(CspDirective::ImgSrc, &url, document_url)
        {
            tracing::warn!(url = %url, "Blocked image by Content Security Policy (img-src)");
            continue;
        }

        let request = HttpRequest::get(url.clone());
        match client
            .fetch_with_security_context(&request, Some(document_url))
            .await
        {
            Ok(response) => {
                let decoded = ImageDecoder::decode_auto(&response.body);
                match decoded {
                    Ok(image) => {
                        tracing::debug!(
                            url = %url,
                            width = image.width,
                            height = image.height,
                            "Decoded image"
                        );
                        images.insert(img_id, image);
                    }
                    Err(err) => {
                        tracing::warn!(url = %url, %err, "Skipping undecodable image");
                    }
                }
            }
            Err(err) => {
                tracing::warn!(url = %url, %err, "Blocked image subresource (CORS/mixed content)");
            }
        }
    }

    images
}

/// Fetches external `<link rel="stylesheet">` sheets through the security-checked path.
pub(super) async fn load_subresource_stylesheets(
    client: &NetworkClient,
    document_url: &Url,
    doc: &Document,
    csp: Option<&CspPolicy>,
) -> Vec<String> {
    let mut sheets = Vec::new();

    for link_id in doc.get_elements_by_tag_name("link") {
        let Some(node) = doc.get_node(link_id) else {
            continue;
        };
        let NodeData::Element(element) = &node.data else {
            continue;
        };
        let Some(rel) = element.attr("rel") else {
            continue;
        };
        if !rel
            .split_whitespace()
            .any(|s| s.eq_ignore_ascii_case("stylesheet"))
        {
            continue;
        }
        if element.attr("disabled").is_some() {
            continue;
        }
        let Some(href) = element.attr("href") else {
            continue;
        };
        let Ok(url) = document_url.join(href) else {
            tracing::warn!(href, "Skipping stylesheet with unresolvable href");
            continue;
        };

        if let Some(policy) = csp
            && !policy.allows(CspDirective::StyleSrc, &url, document_url)
        {
            tracing::warn!(url = %url, "Blocked stylesheet by Content Security Policy (style-src)");
            continue;
        }

        let request = HttpRequest::get(url.clone());
        match client
            .fetch_with_security_context(&request, Some(document_url))
            .await
        {
            Ok(response) => match response.text() {
                Ok(css) => {
                    tracing::debug!(url = %url, "Fetched external stylesheet");
                    sheets.push(css);
                }
                Err(err) => {
                    tracing::warn!(url = %url, %err, "Non-UTF8 stylesheet body");
                }
            },
            Err(err) => {
                tracing::warn!(url = %url, %err, "Blocked external stylesheet (CORS/mixed content)");
            }
        }
    }

    sheets
}

/// Fetches external `<script src="...">` scripts through the security-checked path.
pub(super) async fn load_subresource_scripts(
    client: &NetworkClient,
    document_url: &Url,
    doc: &Document,
    csp: Option<&CspPolicy>,
) -> HashMap<NodeId, String> {
    let mut scripts = HashMap::new();

    for script_id in doc.get_elements_by_tag_name("script") {
        let Some(node) = doc.get_node(script_id) else {
            continue;
        };
        let NodeData::Element(element) = &node.data else {
            continue;
        };
        let Some(src) = element.attr("src") else {
            continue;
        };
        let Ok(url) = document_url.join(src) else {
            tracing::warn!(src, "Skipping external script with unresolvable src");
            continue;
        };

        if let Some(policy) = csp
            && !policy.allows(CspDirective::ScriptSrc, &url, document_url)
        {
            tracing::warn!(url = %url, "Blocked external script by Content Security Policy (script-src)");
            continue;
        }

        let request = HttpRequest::get(url.clone());
        match client
            .fetch_with_security_context(&request, Some(document_url))
            .await
        {
            Ok(response) => match response.text() {
                Ok(js) => {
                    tracing::debug!(url = %url, "Fetched external script");
                    scripts.insert(script_id, js);
                }
                Err(err) => {
                    tracing::warn!(url = %url, %err, "Non-UTF8 script body");
                }
            },
            Err(err) => {
                tracing::warn!(url = %url, %err, "Blocked external script (CORS/mixed content)");
            }
        }
    }

    scripts
}
