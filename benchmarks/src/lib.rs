//! Performance benchmarking harnesses and metrics collectors for the browser engine.

use css::{CascadeResolver, parse_stylesheet};
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use paint::DisplayListBuilder;
use raster::CpuRasterizer;
use std::time::{Duration, Instant};

/// Benchmark performance summary results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineBenchmarkResult {
    /// HTML parsing duration.
    pub html_parse_duration: Duration,
    /// CSS cascade calculation duration.
    pub css_cascade_duration: Duration,
    /// Block layout calculation duration.
    pub layout_duration: Duration,
    /// Display list and paint duration.
    pub paint_duration: Duration,
    /// CPU pixel rasterization duration.
    pub raster_duration: Duration,
}

/// Runs a full synthetic HTML-to-pixels pipeline benchmark.
#[must_use]
pub fn benchmark_full_pipeline(html_source: &str, css_source: &str) -> PipelineBenchmarkResult {
    let t0 = Instant::now();
    let doc = parse_html(html_source);
    let html_parse_duration = t0.elapsed();

    let t1 = Instant::now();
    let user_rules = if css_source.is_empty() {
        Vec::new()
    } else {
        vec![parse_stylesheet(css_source, css::Origin::Author)]
    };
    let user_sheets_refs: Vec<&_> = user_rules.iter().collect();
    let resolver = CascadeResolver::new(&doc, &user_sheets_refs);
    let styles = resolver.resolve_all();
    let css_cascade_duration = t1.elapsed();

    let t2 = Instant::now();
    let mut box_tree = build_box_tree(&doc, doc.root_id(), &styles).expect("box tree build failed");
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut box_tree, &viewport);
    let layout_duration = t2.elapsed();

    let t3 = Instant::now();
    let display_list = DisplayListBuilder::build(&box_tree);
    let paint_duration = t3.elapsed();

    let t4 = Instant::now();
    let _ = CpuRasterizer::rasterize(&display_list, 800, 600);
    let raster_duration = t4.elapsed();

    PipelineBenchmarkResult {
        html_parse_duration,
        css_cascade_duration,
        layout_duration,
        paint_duration,
        raster_duration,
    }
}
