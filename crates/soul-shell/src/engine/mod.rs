//! Wired end-to-end pipeline: navigation state machine → HTTP fetch (CORS/mixed
//! content enforced) → HTML parse → CSS cascade → layout → display list → raster.

mod pipeline_types;
mod stages;
mod subresources;

pub use pipeline_types::{PipelineTimings, RenderOptions, RenderResult};

pub use crate::diagnostics::{a11y_lines, has_visible_pixels};
pub use layout::{A11yNode, A11yRole};

use crate::script_execution::{execute_inline_scripts, execute_scripts};
use css::{CascadeResolver, Origin, parse_stylesheet};
use html::parse_html_with_styles;
use networking::HttpClient;
use raster::PixelBuffer;
use soul_core::{NavigationController, NavigationError};
use stages::document_title;
use std::collections::HashMap;
use std::time::Instant;
use subresources::{
    load_subresource_images, load_subresource_scripts, load_subresource_stylesheets,
};
use url::Url;

/// Fetches `url` with an isolated navigation controller and renders the document.
///
/// # Errors
///
/// Returns `NavigationError` if the fetch, parse, layout, or raster stages fail.
pub async fn navigate_and_render(
    url: Url,
    options: RenderOptions,
) -> Result<RenderResult, NavigationError> {
    let mut controller = NavigationController::new();
    navigate_and_render_with_controller(&mut controller, url, options).await
}

/// Fetches `url` using caller-owned navigation state.
///
/// Caller-owned state is required for browser actions such as Back, Forward, and
/// Reload to share one `NavigationController` and one session history.
///
/// # Errors
///
/// Returns `NavigationError` if the fetch, parse, layout, or raster stages fail.
pub async fn navigate_and_render_with_controller(
    controller: &mut NavigationController,
    url: Url,
    options: RenderOptions,
) -> Result<RenderResult, NavigationError> {
    controller.navigate_url(url);
    render_active_navigation(controller, options).await
}

/// Renders the navigation already active in `controller` without creating a
/// second navigation id. Used by Back, Forward, and Reload.
///
/// # Errors
///
/// Returns `NavigationError` if no navigation is active or rendering fails.
pub async fn render_active_navigation(
    controller: &mut NavigationController,
    options: RenderOptions,
) -> Result<RenderResult, NavigationError> {
    let navigation_id = controller
        .state()
        .navigation_id()
        .ok_or_else(|| NavigationError::Other("no active navigation".to_string()))?;
    let url = controller
        .state()
        .current_url()
        .cloned()
        .ok_or_else(|| NavigationError::Other("active navigation has no URL".to_string()))?;
    let mut timings = PipelineTimings::default();

    // Stage 1: network fetch (top-level document navigation).
    let fetch_start = Instant::now();
    let client = HttpClient::default();
    let response = client
        .fetch(&url)
        .await
        .map_err(|e| NavigationError::Other(format!("fetch failed: {e}")))?;
    timings.fetch = fetch_start.elapsed();

    if !controller.handle_response(
        navigation_id,
        response.status_code,
        response.mime_type.clone(),
    ) {
        return Err(NavigationError::Other(
            "navigation id mismatch during response handling".to_string(),
        ));
    }

    if !response.is_success() {
        controller.handle_error(
            navigation_id,
            format!("HTTP error status {}", response.status_code),
        );
        return Err(NavigationError::Other(format!(
            "HTTP {} from {}",
            response.status_code, url
        )));
    }

    let html = response
        .text()
        .map_err(|e| NavigationError::Other(format!("non-UTF8 response body: {e}")))?;

    // Stage 2: parse document and extract author `<style>` sheets.
    let parse_start = Instant::now();
    let (doc, mut style_sources) = parse_html_with_styles(&html);
    timings.parse = parse_start.elapsed();

    // Stage 2.5: fetch external scripts and execute all scripts in document order.
    let external_scripts = load_subresource_scripts(&client, &url, &doc).await;
    let doc = execute_scripts(doc, Some(&url), Some(&client), Some(&external_scripts))?;
    let title = document_title(&doc, &url);

    // Stage 3: fetch + decode `<img>` and `<link rel="stylesheet">` subresources.
    let images_start = Instant::now();
    let images = load_subresource_images(&client, &url, &doc).await;
    let external_stylesheets = load_subresource_stylesheets(&client, &url, &doc).await;
    style_sources.extend(external_stylesheets);
    timings.images = images_start.elapsed();

    // Stage 4: author stylesheet parse + cascade.
    let style_start = Instant::now();
    let author_sheets: Vec<_> = style_sources
        .iter()
        .map(|css| parse_stylesheet(css, Origin::Author))
        .collect();
    let author_refs: Vec<&css::StyleSheet> = author_sheets.iter().collect();
    let resolver = CascadeResolver::new(&doc, &author_refs);
    let styles = resolver.resolve_all();
    timings.style = style_start.elapsed();

    if !controller.handle_dom_ready(navigation_id) {
        return Err(NavigationError::Other(
            "navigation id mismatch during DOM ready".to_string(),
        ));
    }

    // Stage 5: layout, paint, raster + accessibility tree.
    let (pixel_buffer, document_buffer, document_height, a11y_tree, hit_test_map, stage_timings) =
        stages::layout_paint_raster(&doc, &styles, &images, options)?;
    timings.layout = stage_timings.layout;
    timings.paint = stage_timings.paint;
    timings.raster = stage_timings.raster;

    if !controller.handle_loaded(navigation_id) {
        return Err(NavigationError::Other(
            "navigation id mismatch during load completion".to_string(),
        ));
    }

    Ok(RenderResult::new(
        navigation_id,
        url,
        title,
        response.status_code,
        pixel_buffer,
        document_buffer,
        document_height,
        0.0,
        a11y_tree,
        hit_test_map,
        timings,
    ))
}

/// Parses and renders an in-memory HTML string through the full pipeline without
/// network or navigation state. Used for the built-in start page and fixtures.
///
/// # Errors
///
/// Returns `NavigationError` if parsing, layout, or rasterization fails.
pub fn render_html_to_buffer(
    html: &str,
    options: RenderOptions,
) -> Result<(PixelBuffer, Option<A11yNode>, PipelineTimings), NavigationError> {
    let mut controller = NavigationController::new();
    // Local rendering has no navigation id; use 0 and skip controller transitions.
    let id =
        controller.navigate_url(Url::parse("about:start").expect("about:start is a valid URL"));
    controller.handle_response(id, 200, "text/html".to_string());

    let parse_start = Instant::now();
    let (doc, style_sources) = parse_html_with_styles(html);
    let doc = execute_inline_scripts(doc, None, None)?;
    let mut timings = PipelineTimings {
        parse: parse_start.elapsed(),
        ..Default::default()
    };

    let style_start = Instant::now();
    let author_sheets: Vec<_> = style_sources
        .iter()
        .map(|css| parse_stylesheet(css, Origin::Author))
        .collect();
    let author_refs: Vec<&css::StyleSheet> = author_sheets.iter().collect();
    let resolver = CascadeResolver::new(&doc, &author_refs);
    let styles = resolver.resolve_all();
    timings.style = style_start.elapsed();

    let _ = controller.handle_dom_ready(id);
    let (pixel_buffer, _document_buffer, _document_height, a11y_tree, _hit_test_map, stage_timings) =
        stages::layout_paint_raster(&doc, &styles, &HashMap::new(), options)?;
    timings.layout = stage_timings.layout;
    timings.paint = stage_timings.paint;
    timings.raster = stage_timings.raster;
    let _ = controller.handle_loaded(id);

    Ok((pixel_buffer, a11y_tree, timings))
}
