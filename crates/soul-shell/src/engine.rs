//! Wired end-to-end pipeline: navigation state machine → HTTP fetch (CORS/mixed
//! content enforced) → HTML parse → CSS cascade → layout → display list → raster.

use css::{CascadeResolver, Origin, parse_stylesheet};
use dom::{Document, NodeData, NodeId};
use html::parse_html_with_styles;
use image_decode::{DecodedImage, ImageDecoder};
use layout::{Dimensions, IntrinsicSize, Rect, build_box_tree_with_intrinsics, layout_block};
use networking::{HttpClient, HttpRequest};
use paint::DisplayListBuilder;
use raster::{CpuRasterizer, PixelBuffer};
use soul_core::{NavigationController, NavigationError, NavigationId};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use url::Url;

/// Re-exports for callers that inspect the accessibility tree.
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
    /// HTTP status code of the response.
    pub status_code: u16,
    /// Rasterized page pixels.
    pub pixel_buffer: PixelBuffer,
    /// Accessibility tree extracted from the laid-out page.
    pub a11y_tree: Option<A11yNode>,
    /// Per-stage timings.
    pub timings: PipelineTimings,
}

impl RenderResult {
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

/// Fetches `url` and renders the resulting document through the full pipeline.
///
/// Transitions the `NavigationController` through `Navigating` → `ResponseReceived`
/// → `DomReady` → `Loaded`.
///
/// # Errors
///
/// Returns `NavigationError` if the fetch, parse, layout, or raster stages fail.
pub async fn navigate_and_render(
    url: Url,
    options: RenderOptions,
) -> Result<RenderResult, NavigationError> {
    let mut controller = NavigationController::new();
    let navigation_id = controller.navigate_url(url.clone());
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
    let (pixel_buffer, a11y_tree, stage_timings) =
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
        status_code: response.status_code,
        pixel_buffer,
        a11y_tree,
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
    let (pixel_buffer, a11y_tree, stage_timings) =
        layout_paint_raster(&doc, &styles, &HashMap::new(), options)?;
    timings.layout = stage_timings.layout;
    timings.paint = stage_timings.paint;
    timings.raster = stage_timings.raster;
    let _ = controller.handle_loaded(id);

    Ok((pixel_buffer, a11y_tree, timings))
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
#[allow(clippy::cast_precision_loss, clippy::type_complexity)]
fn layout_paint_raster(
    doc: &Document,
    styles: &HashMap<NodeId, css::ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    options: RenderOptions,
) -> Result<(PixelBuffer, Option<A11yNode>, PipelineTimings), NavigationError> {
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
    let pixel_buffer = CpuRasterizer::rasterize(&display_list, options.width, options.height)
        .map_err(|e| NavigationError::Other(format!("rasterization failed: {e}")))?;
    timings.raster = raster_start.elapsed();

    let a11y_tree = A11yNode::from_layout_box(doc, &layout_box);

    Ok((pixel_buffer, a11y_tree, timings))
}

/// Collects human-readable accessibility tree lines for logging or dump output.
pub fn a11y_lines(tree: &A11yNode, out: &mut Vec<String>) {
    a11y_lines_indented(tree, 0, out);
}

fn a11y_lines_indented(node: &A11yNode, depth: usize, out: &mut Vec<String>) {
    let name = node.name.as_deref().unwrap_or("");
    out.push(format!(
        "{:indent$}- {:?} \"{}\" bounds=({:.0},{:.0} {:.0}x{:.0})",
        "",
        node.role,
        name,
        node.bounds.x,
        node.bounds.y,
        node.bounds.width,
        node.bounds.height,
        indent = depth * 2
    ));
    for child in &node.children {
        a11y_lines_indented(child, depth + 1, out);
    }
}

/// Convenience: returns `true` if the pixel buffer contains any non-transparent pixel.
#[must_use]
pub fn has_visible_pixels(buffer: &PixelBuffer) -> bool {
    buffer.data.chunks_exact(4).any(|px| px[3] != 0)
}
