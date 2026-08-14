//! Integration tests for engine performance metrics and benchmark execution.

use benchmarks::benchmark_full_pipeline;

#[test]
fn test_full_pipeline_benchmark_execution() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Benchmark Test</title></head>
        <body>
            <div style="background-color: red; width: 200px; height: 100px;">
                <h1>Heading text</h1>
                <p>Paragraph content to measure full layout performance.</p>
            </div>
        </body>
        </html>
    "#;
    let css = "h1 { color: blue; } p { color: black; }";

    let result = benchmark_full_pipeline(html, css);

    // Assert that every stage recorded non-zero or valid sub-millisecond durations
    assert!(result.html_parse_duration.as_nanos() > 0);
    assert!(result.css_cascade_duration.as_nanos() > 0);
    assert!(result.layout_duration.as_nanos() > 0);
    assert!(result.paint_duration.as_nanos() > 0);
    assert!(result.raster_duration.as_nanos() > 0);
}
