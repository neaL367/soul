//! Wired end-to-end pipeline: navigation state machine → HTTP fetch (CORS/mixed
//! content enforced) → HTML parse → CSS cascade → layout → display list → raster.

use crate::hit_testing::build_hit_test_map;
use crate::script_execution::execute_inline_scripts;
use css::{CascadeResolver, Origin, parse_stylesheet};
use dom::{Document, NodeData, NodeId};
use html::parse_html_with_styles;
use image_decode::{DecodedImage, ImageDecoder};
use layout::{Dimensions, IntrinsicSize, Rect, build_box_tree_with_intrinsics, layout_block};
use networking::{HttpClient, HttpRequest};
use paint::DisplayListBuilder;
use raster::{CpuRasterizer, PixelBuffer};
use soul_core::{NavigationController, NavigationError, NavigationId};
use soul_ui::HitTestMap;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use url::Url;

/// Re-exports for callers that inspect the accessibility tree.
pub use crate::diagnostics::{a11y_lines, has_visible_pixels};
pub use layout::{A11yNode, A11yRole};

/// Viewport configuration for the rendering pipeline.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
        }
    }
}

/// Per-stage timings of a full pipeline run.
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineTimings {
    /// Network fetch duration.
    pub fetch: Duration,
    /// HTML parse duration.
    pub parse: Duration,
    /// Subresource image fetch + decode duration.
    pub images: Duration,
    /// Style cascade duration.
    pub style: Duration,
    /// Layout duration.
    pub layout: Duration,
    /// Display-list paint duration.
    pub paint: Duration,
    /// CPU rasterization duration.
    pub raster: Duration,
}

impl PipelineTimings {
    /// Sum of all stages.
    #[must_use]
    pub fn total(&self) -> Duration {
        self.fetch + self.parse + self.images + self.style + self.layout + self.paint + self.raster
    }
}

/// Outcome of a complete navigation + render cycle.
#[derive(Debug)]
pub struct RenderResult {
    /// Navigation id assigned by the state machine.
    pub navigation_id: NavigationId,
    /// Final page URL.
    pub url: Url,
    /// Document title used by browser chrome.
    pub title: String,
    /// HTTP status code of the response.
    pub status_code: u16,
    /// Rasterized page pixels.
    pub pixel_buffer: PixelBuffer,
    /// Full document raster retained for viewport-only scrolling.
    document_buffer: PixelBuffer,
    /// Total laid-out document height in pixels.
    pub document_height: f32,
    /// Current document-space vertical offset.
    pub scroll_y: f32,
    /// Accessibility tree extracted from the laid-out page.
    pub a11y_tree: Option<A11yNode>,
    /// Interactive page regions generated from the laid-out page.
    pub hit_test_map: HitTestMap,
    /// Per-stage timings.
    pub timings: PipelineTimings,
}

impl RenderResult {
    /// Scrolls the retained page raster without refetching or relayout.
    #[allow(clippy::cast_precision_loss)]
    pub fn scroll_by(&mut self, delta_y: f32, viewport_height: u32) {
        self.scroll_y = (self.scroll_y + delta_y).clamp(
            0.0,
            (self.document_height - viewport_height as f32).max(0.0),
        );
        self.pixel_buffer = crop_viewport(&self.document_buffer, viewport_height, self.scroll_y);
    }

    /// Encodes the rasterized page as a PNG byte stream.
    ///
    /// # Errors
    ///
    /// Returns an error if PNG encoding fails.
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        image_decode::encode_png(
            &self.pixel_buffer.data,
            self.pixel_buffer.width,
            self.pixel_buffer.height,
        )
        .map_err(|e| e.to_string())
    }
}

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

    // Stage 1: network fetch (top-level document navigation — CORS is not applied
    // to top-level loads; mixed-content/CORS enforcement for subresources happens
    // in `load_subresource_images` via `fetch_with_security_context`).
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
    let (doc, style_sources) = parse_html_with_styles(&html);
    timings.parse = parse_start.elapsed();
    let doc = execute_inline_scripts(doc)?;
    let title = document_title(&doc, &url);

    // Stage 3: fetch + decode `<img>` subresources (CORS/mixed content enforced).
    let images_start = Instant::now();
    let images = load_subresource_images(&client, &url, &doc).await;
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
        layout_paint_raster(&doc, &styles, &images, options)?;
    timings.layout = stage_timings.layout;
    timings.paint = stage_timings.paint;
    timings.raster = stage_timings.raster;

    if !controller.handle_loaded(navigation_id) {
        return Err(NavigationError::Other(
            "navigation id mismatch during load completion".to_string(),
        ));
    }

    Ok(RenderResult {
        navigation_id,
        url,
        title,
        status_code: response.status_code,
        pixel_buffer,
        document_buffer,
        document_height,
        scroll_y: 0.0,
        a11y_tree,
        hit_test_map,
        timings,
    })
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
    let doc = execute_inline_scripts(doc)?;
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
        layout_paint_raster(&doc, &styles, &HashMap::new(), options)?;
    timings.layout = stage_timings.layout;
    timings.paint = stage_timings.paint;
    timings.raster = stage_timings.raster;
    let _ = controller.handle_loaded(id);

    Ok((pixel_buffer, a11y_tree, timings))
}

fn document_title(doc: &Document, url: &Url) -> String {
    let title = doc
        .get_elements_by_tag_name("title")
        .first()
        .map(|id| doc.text_content(*id).trim().to_string())
        .filter(|title| !title.is_empty());
    title
        .or_else(|| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "New Tab".to_string())
}

/// Fetches and decodes every `<img>` subresource through the security-checked
/// client path (mixed content + CORS enforced against the document origin).
/// Individual failures are non-fatal: the image is skipped and logged.
async fn load_subresource_images(
    client: &HttpClient,
    document_url: &Url,
    doc: &Document,
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

        let request = HttpRequest::get(url.clone());
        match client
            .fetch_with_security_context(&request, Some(document_url))
            .await
        {
            Ok(response) => {
                let decoded = if response.mime_type.contains("svg") {
                    ImageDecoder::decode_svg(&response.body, 0, 0)
                } else {
                    ImageDecoder::decode_raster(&response.body)
                };
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

/// Shared layout → paint → raster core with accessibility extraction.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::type_complexity
)]
fn layout_paint_raster(
    doc: &Document,
    styles: &HashMap<NodeId, css::ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    options: RenderOptions,
) -> Result<
    (
        PixelBuffer,
        PixelBuffer,
        f32,
        Option<A11yNode>,
        HitTestMap,
        PipelineTimings,
    ),
    NavigationError,
> {
    let mut timings = PipelineTimings::default();

    let layout_start = Instant::now();
    let intrinsics: HashMap<NodeId, IntrinsicSize> = images
        .iter()
        .map(|(id, img)| (*id, IntrinsicSize::new(img.width, img.height)))
        .collect();
    let mut layout_box = build_box_tree_with_intrinsics(doc, doc.root_id(), styles, &intrinsics)
        .ok_or_else(|| {
            NavigationError::Other("box tree construction failed for document".to_string())
        })?;
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, options.width as f32, options.height as f32),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);
    timings.layout = layout_start.elapsed();

    let paint_start = Instant::now();
    let display_list = DisplayListBuilder::build(&layout_box, images);
    timings.paint = paint_start.elapsed();

    let raster_start = Instant::now();
    let document_height = (layout_box.dimensions.content.y + layout_box.dimensions.content.height)
        .ceil()
        .max(options.height as f32);
    let document_buffer =
        CpuRasterizer::rasterize(&display_list, options.width, document_height as u32)
            .map_err(|e| NavigationError::Other(format!("rasterization failed: {e}")))?;
    let pixel_buffer = crop_viewport(&document_buffer, options.height, 0.0);
    timings.raster = raster_start.elapsed();

    let a11y_tree = A11yNode::from_layout_box(doc, &layout_box);
    let hit_test_map = build_hit_test_map(doc, &layout_box);

    Ok((
        pixel_buffer,
        document_buffer,
        document_height,
        a11y_tree,
        hit_test_map,
        timings,
    ))
}

/// Copies visible rows from a retained full-document raster.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn crop_viewport(document: &PixelBuffer, viewport_height: u32, scroll_y: f32) -> PixelBuffer {
    let max_scroll = document.height.saturating_sub(viewport_height);
    let offset = (scroll_y.max(0.0) as u32).min(max_scroll);
    let row_bytes = document.width as usize * 4;
    let mut data = vec![0; row_bytes * viewport_height as usize];
    for row in 0..viewport_height {
        let source_row = offset + row;
        if source_row >= document.height {
            break;
        }
        let source_start = source_row as usize * row_bytes;
        let target_start = row as usize * row_bytes;
        data[target_start..target_start + row_bytes]
            .copy_from_slice(&document.data[source_start..source_start + row_bytes]);
    }
    PixelBuffer::from_raw(document.width, viewport_height, data)
}
