//! Viewport options, timing metrics, and raster render outcomes.

use raster::PixelBuffer;
use soul_core::NavigationId;
use soul_ui::HitTestMap;
use std::time::Duration;
use url::Url;

pub use layout::A11yNode;

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
    pub(super) document_buffer: PixelBuffer,
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
    /// Constructs a `RenderResult`.
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        navigation_id: NavigationId,
        url: Url,
        title: String,
        status_code: u16,
        pixel_buffer: PixelBuffer,
        document_buffer: PixelBuffer,
        document_height: f32,
        scroll_y: f32,
        a11y_tree: Option<A11yNode>,
        hit_test_map: HitTestMap,
        timings: PipelineTimings,
    ) -> Self {
        Self {
            navigation_id,
            url,
            title,
            status_code,
            pixel_buffer,
            document_buffer,
            document_height,
            scroll_y,
            a11y_tree,
            hit_test_map,
            timings,
        }
    }

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

/// Copies visible rows from a retained full-document raster.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn crop_viewport(
    document: &PixelBuffer,
    viewport_height: u32,
    scroll_y: f32,
) -> PixelBuffer {
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
