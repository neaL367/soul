//! Browser application entry point: CLI dispatch and end-to-end navigation wiring.
//!
//! Usage: `browser-shell [url] [--output=page.png] [--width=N] [--height=N] [--dump-a11y]`
//!
//! With a URL: fetches the page over the network (CORS/mixed-content enforced),
//! renders it through the full engine pipeline, and presents the frame to the
//! chrome backend. Without a URL: renders the built-in start page.

use anyhow::{Context, Result};
use browser_shell::engine::{
    RenderOptions, a11y_lines, has_visible_pixels, navigate_and_render, render_html_to_buffer,
};
use browser_ui::{ChromeBackend, ChromeConfig, ViewportFrame, WindowSpec};
use chrome_backend_gpui::GpuiChromeBackend;
use std::path::PathBuf;
use url::Url;

/// Parsed command-line configuration.
struct Cli {
    url: Option<String>,
    output: Option<PathBuf>,
    width: u32,
    height: u32,
    dump_a11y: bool,
}

fn parse_args(args: &[String]) -> Result<Cli> {
    let mut cli = Cli {
        url: None,
        output: None,
        width: 1280,
        height: 800,
        dump_a11y: false,
    };

    for arg in args {
        if let Some(value) = arg.strip_prefix("--output=") {
            cli.output = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--width=") {
            cli.width = value.parse().context("invalid --width value")?;
        } else if let Some(value) = arg.strip_prefix("--height=") {
            cli.height = value.parse().context("invalid --height value")?;
        } else if arg == "--dump-a11y" {
            cli.dump_a11y = true;
        } else {
            cli.url = Some(arg.clone());
        }
    }
    Ok(cli)
}

fn main() -> Result<()> {
    common::init_tracing();
    let cli = parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;

    // Chrome backend lifecycle (M1 trait contract). The backend is headless in the
    // current implementation; the frame is presented through the same trait API a
    // real window backend will consume.
    let mut backend = Box::new(GpuiChromeBackend::new());
    backend
        .init(ChromeConfig {
            app_name: "Soul Browser".to_string(),
            resource_dir: None,
        })
        .context("failed to initialize chrome backend")?;

    backend.set_event_handler(Box::new(|event| {
        tracing::info!(?event, "Chrome event");
    }));

    let window_id = backend
        .open_window(WindowSpec {
            title: "Soul Browser".to_string(),
            width: cli.width,
            height: cli.height,
            min_width: Some(400),
            min_height: Some(300),
            decorated: true,
            resizable: true,
        })
        .context("failed to open browser window")?;

    let frame = render_selected(&cli)?;

    if let Some(frame) = frame {
        backend
            .update_viewport(window_id, frame)
            .context("failed to present frame to viewport")?;
    }

    backend.run().context("chrome backend runtime error")?;
    Ok(())
}

/// Renders either the requested remote URL or the built-in start page.
fn render_selected(cli: &Cli) -> Result<Option<ViewportFrame>> {
    cli.url
        .as_deref()
        .map_or_else(|| render_start_page(cli), |raw| render_remote_url(raw, cli))
}

/// Fetches and renders a live URL through the full engine pipeline.
fn render_remote_url(raw_url: &str, cli: &Cli) -> Result<Option<ViewportFrame>> {
    let url = normalize_url(raw_url)?;
    let options = RenderOptions {
        width: cli.width,
        height: cli.height,
    };

    tracing::info!(url = %url, width = cli.width, height = cli.height, "Navigating");

    let runtime = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    let result = runtime
        .block_on(navigate_and_render(url, options))
        .context("navigation/render failed")?;

    tracing::info!(
        navigation_id = result.navigation_id.0,
        url = %result.url,
        status = result.status_code,
        fetch_ms = result.timings.fetch.as_millis(),
        parse_ms = result.timings.parse.as_millis(),
        style_ms = result.timings.style.as_millis(),
        layout_ms = result.timings.layout.as_millis(),
        paint_ms = result.timings.paint.as_millis(),
        raster_ms = result.timings.raster.as_millis(),
        total_ms = result.timings.total().as_millis(),
        "Page rendered"
    );

    if cli.dump_a11y
        && let Some(tree) = &result.a11y_tree
    {
        let mut lines = Vec::new();
        a11y_lines(tree, &mut lines);
        tracing::info!(a11y = ?lines, "Accessibility tree");
    }

    if has_visible_pixels(&result.pixel_buffer) {
        tracing::info!("Page produced visible pixels");
    } else {
        tracing::warn!("Page produced an empty (fully transparent) frame");
    }

    if let Some(path) = &cli.output {
        save_png(&result.encode_png().map_err(|e| anyhow::anyhow!(e))?, path)?;
    }

    Ok(Some(ViewportFrame::SoftwareRgba {
        width: cli.width,
        height: cli.height,
        pixels: result.pixel_buffer.data,
    }))
}

/// Renders the built-in start page through the engine pipeline.
fn render_start_page(cli: &Cli) -> Result<Option<ViewportFrame>> {
    let options = RenderOptions {
        width: cli.width,
        height: cli.height,
    };
    let start_html = r#"
        <html>
        <head><title>Soul Browser</title></head>
        <body style="margin: 20px; background-color: #1e1e2e; color: #cdd6f4; font-family: sans-serif;">
            <h1 style="color: #89b4fa;">Welcome to Soul Browser</h1>
            <p style="color: #a6adc8; font-size: 18px;">A complete, modern browser engine built from scratch in Rust with GPUI.</p>
            <div style="background-color: #313244; padding: 15px; border-width: 1px; color: #a6e3a1;">
                <p>Status: End-to-end navigation pipeline active — pass a URL to browse.</p>
            </div>
        </body>
        </html>
    "#;

    let (buffer, _, timings) =
        render_html_to_buffer(start_html, options).context("render failed")?;
    tracing::info!(
        parse_ms = timings.parse.as_millis(),
        style_ms = timings.style.as_millis(),
        layout_ms = timings.layout.as_millis(),
        paint_ms = timings.paint.as_millis(),
        raster_ms = timings.raster.as_millis(),
        "Start page rendered"
    );

    if let Some(path) = &cli.output {
        save_png(
            &image_decode::encode_png(&buffer.data, buffer.width, buffer.height)?,
            path,
        )?;
    }

    Ok(Some(ViewportFrame::SoftwareRgba {
        width: cli.width,
        height: cli.height,
        pixels: buffer.data,
    }))
}

/// Writes encoded PNG bytes to disk.
fn save_png(png: &[u8], path: &PathBuf) -> Result<()> {
    std::fs::write(path, png)
        .with_context(|| format!("failed to write PNG to {}", path.display()))?;
    tracing::info!(path = %path.display(), "Saved rendered page PNG");
    Ok(())
}

/// Normalizes a user-typed address into a parseable `Url`, defaulting to HTTPS.
fn normalize_url(input: &str) -> Result<Url> {
    if input.starts_with("http://") || input.starts_with("https://") || input.starts_with("about:")
    {
        Url::parse(input).context("URL parse failed")
    } else {
        Url::parse(&format!("https://{input}")).context("URL parse failed (try https:// prefix)")
    }
}
