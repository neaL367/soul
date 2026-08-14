//! Wired end-to-end pipeline: navigation state machine → HTTP fetch (CORS/mixed
//! content enforced) → HTML parse → CSS cascade → layout → display list → raster.

use browser_core::{NavigationController, NavigationError, NavigationId};
use css::CascadeResolver;
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use networking::HttpClient;
use paint::DisplayListBuilder;
use raster::{CpuRasterizer, PixelBuffer};
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
        self.fetch + self.parse + self.style + self.layout + self.paint + self.raster
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
    // to top-level loads; mixed-content/CORS enforcement for subresources lives in
    // `fetch_with_security_context` on `HttpClient`).
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

    let (pixel_buffer, a11y_tree, stage_timings) =
        render_html_document(&html, options, &mut controller, navigation_id)?;
    timings.parse = stage_timings.parse;
    timings.style = stage_timings.style;
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
    let (pixel_buffer, a11y_tree, timings) =
        render_html_document(html, options, &mut controller, id)?;
    let _ = controller.handle_loaded(id);
    Ok((pixel_buffer, a11y_tree, timings))
}

/// Shared parse → style → layout → paint → raster core, used by both the network
/// path and the in-memory path.
#[allow(clippy::cast_precision_loss, clippy::type_complexity)]
fn render_html_document(
    html: &str,
    options: RenderOptions,
    controller: &mut NavigationController,
    navigation_id: NavigationId,
) -> Result<(PixelBuffer, Option<A11yNode>, PipelineTimings), NavigationError> {
    let mut timings = PipelineTimings::default();

    let parse_start = Instant::now();
    let doc = parse_html(html);
    timings.parse = parse_start.elapsed();

    let style_start = Instant::now();
    // UA stylesheet is injected automatically; author `<style>` extraction is not
    // yet implemented in the HTML sink, so inline styles carry author styling.
    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();
    timings.style = style_start.elapsed();

    if !controller.handle_dom_ready(navigation_id) {
        return Err(NavigationError::Other(
            "navigation id mismatch during DOM ready".to_string(),
        ));
    }

    let layout_start = Instant::now();
    let mut layout_box = build_box_tree(&doc, doc.root_id(), &styles).ok_or_else(|| {
        NavigationError::Other("box tree construction failed for document".to_string())
    })?;
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, options.width as f32, options.height as f32),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);
    timings.layout = layout_start.elapsed();

    let paint_start = Instant::now();
    let display_list = DisplayListBuilder::build(&layout_box);
    timings.paint = paint_start.elapsed();

    let raster_start = Instant::now();
    let pixel_buffer = CpuRasterizer::rasterize(&display_list, options.width, options.height)
        .map_err(|e| NavigationError::Other(format!("rasterization failed: {e}")))?;
    timings.raster = raster_start.elapsed();

    let a11y_tree = A11yNode::from_layout_box(&doc, &layout_box);

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
