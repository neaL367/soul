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
    /// 50th percentile (median) duration per frame.
    pub p50_total: Duration,
    /// 95th percentile duration per frame.
    pub p95_total: Duration,
    /// 99th percentile duration per frame.
    pub p99_total: Duration,
    /// Mean HTML parsing duration per iteration.
    pub mean_html_parse: Duration,
    /// Mean CSS cascade duration per iteration.
    pub mean_css_cascade: Duration,
    /// Mean block layout duration per iteration.
    pub mean_layout: Duration,
    /// Mean display-list paint duration per iteration.
    pub mean_paint: Duration,
    /// Mean CPU rasterization duration per iteration.
    pub mean_raster: Duration,
    /// Minimum recorded frame duration.
    pub min_total: Duration,
    /// Maximum recorded frame duration.
    pub max_total: Duration,
    /// Calculated frames per second throughput.
    pub throughput_fps: f64,
}

impl PipelineStats {
    /// Checks if the mean frame rendering duration satisfies a given budget.
    #[must_use]
    pub const fn satisfies_budget(&self, max_allowed_mean: Duration) -> bool {
        self.mean_total.as_nanos() <= max_allowed_mean.as_nanos()
    }
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
    let mut html_parse_duration = Duration::ZERO;
    let mut css_cascade_duration = Duration::ZERO;
    let mut layout_duration = Duration::ZERO;
    let mut paint_duration = Duration::ZERO;
    let mut raster_duration = Duration::ZERO;

    let mut totals = Vec::with_capacity(count);

    for _ in 0..count {
        let res = benchmark_full_pipeline(html_source, css_source);
        let tot = res.total_duration();
        totals.push(tot);
        total_duration += tot;
        html_parse_duration += res.html_parse_duration;
        css_cascade_duration += res.css_cascade_duration;
        layout_duration += res.layout_duration;
        paint_duration += res.paint_duration;
        raster_duration += res.raster_duration;
        if tot < min_total {
            min_total = tot;
        }
        if tot > max_total {
            max_total = tot;
        }
    }

    totals.sort_unstable();
    let p50_total = totals[count / 2];
    let p95_idx = ((count * 95) / 100).min(count - 1);
    let p95_total = totals[p95_idx];
    let p99_idx = ((count * 99) / 100).min(count - 1);
    let p99_total = totals[p99_idx];

    let count_u32 = count as u32;
    let mean_total = total_duration / count_u32;
    let mean_secs = mean_total.as_secs_f64();
    let throughput_fps = if mean_secs > 0.0 {
        1.0 / mean_secs
    } else {
        0.0
    };

    PipelineStats {
        iterations: count,
        mean_total,
        p50_total,
        p95_total,
        p99_total,
        mean_html_parse: html_parse_duration / count_u32,
        mean_css_cascade: css_cascade_duration / count_u32,
        mean_layout: layout_duration / count_u32,
        mean_paint: paint_duration / count_u32,
        mean_raster: raster_duration / count_u32,
        min_total,
        max_total,
        throughput_fps,
    }
}
