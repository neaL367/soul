//! Performance benchmarking harnesses and metrics collectors for the browser engine.

use css::{CascadeResolver, parse_stylesheet};
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use paint::DisplayListBuilder;
use raster::CpuRasterizer;
use std::time::{Duration, Instant};

/// Benchmark performance summary results for a single run.
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

impl PipelineBenchmarkResult {
    /// Returns the total duration across all 5 engine pipeline stages.
    #[must_use]
    pub fn total_duration(&self) -> Duration {
        self.html_parse_duration
            + self.css_cascade_duration
            + self.layout_duration
            + self.paint_duration
            + self.raster_duration
    }
}

/// Aggregated multi-iteration benchmark statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipelineStats {
    /// Total number of iterations executed.
    pub iterations: usize,
    /// Mean total duration per frame.
    pub mean_total: Duration,
    /// Minimum recorded frame duration.
    pub min_total: Duration,
    /// Maximum recorded frame duration.
    pub max_total: Duration,
    /// Calculated frames per second throughput.
    pub throughput_fps: f64,
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
    let display_list = DisplayListBuilder::build(&box_tree, &std::collections::HashMap::new());
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

/// Runs the pipeline benchmark for `n` iterations and computes aggregated statistical metrics.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn benchmark_pipeline_stats(
    html_source: &str,
    css_source: &str,
    iterations: usize,
) -> PipelineStats {
    let count = iterations.max(1);
    let mut total_duration = Duration::ZERO;
    let mut min_total = Duration::MAX;
    let mut max_total = Duration::ZERO;

    for _ in 0..count {
        let res = benchmark_full_pipeline(html_source, css_source);
        let tot = res.total_duration();
        total_duration += tot;
        if tot < min_total {
            min_total = tot;
        }
        if tot > max_total {
            max_total = tot;
        }
    }

    let mean_total = total_duration / count as u32;
    let mean_secs = mean_total.as_secs_f64();
    let throughput_fps = if mean_secs > 0.0 {
        1.0 / mean_secs
    } else {
        0.0
    };

    PipelineStats {
        iterations: count,
        mean_total,
        min_total,
        max_total,
        throughput_fps,
    }
}
