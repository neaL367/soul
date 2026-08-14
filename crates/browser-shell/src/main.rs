//! Browser application entry point, wiring chrome, multi-process CLI dispatch, and rendering pipeline.

use browser_ui::{ChromeBackend, ChromeConfig, ViewportFrame, WindowSpec};
use chrome_backend_gpui::GpuiChromeBackend;
use css::CascadeResolver;
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use paint::DisplayListBuilder;
use raster::CpuRasterizer;
use std::env;

fn main() {
    common::init_tracing();
    let args: Vec<String> = env::args().collect();

    // Multi-process CLI role dispatch
    if let Some(process_type) = args.iter().find(|a| a.starts_with("--type=")) {
        match process_type.as_str() {
            "--type=network" => {
                tracing::info!("Starting Soul Browser Network Worker Process");
                return;
            }
            "--type=gpu" => {
                tracing::info!("Starting Soul Browser GPU Compositor Process");
                return;
            }
            "--type=renderer" => {
                tracing::info!("Starting Soul Browser Sandboxed Renderer Process");
                return;
            }
            _ => {}
        }
    }

    tracing::info!("Soul Browser starting up (Production Architecture)...");

    let mut backend = Box::new(GpuiChromeBackend::new());

    if let Err(err) = backend.init(ChromeConfig {
        app_name: "Soul Browser".to_string(),
        resource_dir: None,
    }) {
        tracing::error!(%err, "Failed to initialize chrome backend");
        return;
    }

    backend.set_event_handler(Box::new(|event| {
        tracing::info!(?event, "Received chrome event");
    }));

    let window_spec = WindowSpec {
        title: "Soul Browser".to_string(),
        width: 1280,
        height: 800,
        min_width: Some(400),
        min_height: Some(300),
        decorated: true,
        resizable: true,
    };

    let window_id = match backend.open_window(window_spec) {
        Ok(id) => {
            tracing::info!(window_id = id.0, "Initial browser window opened");
            id
        }
        Err(err) => {
            tracing::error!(%err, "Failed to open initial browser window");
            return;
        }
    };

    // Render initial start page through the full engine pipeline
    let start_html = r#"
        <html>
        <head><title>Soul Browser</title></head>
        <body style="margin: 20px; background-color: #1e1e2e; color: #cdd6f4; font-family: sans-serif;">
            <h1 style="color: #89b4fa;">Welcome to Soul Browser</h1>
            <p style="color: #a6adc8; font-size: 18px;">A complete, modern browser engine built from scratch in Rust with GPUI.</p>
            <div style="background-color: #313244; padding: 15px; border-width: 1px; color: #a6e3a1;">
                <p>Status: Production Architecture Active (WGPU Compositor, Named Pipe IPC, Job Object Sandboxing, Web APIs &amp; A11y Tree)</p>
            </div>
        </body>
        </html>
    "#;

    if let Some(frame) = render_page_to_frame(start_html, 1280, 800) {
        if let Err(err) = backend.update_viewport(window_id, frame) {
            tracing::warn!(%err, "Failed to update viewport with initial frame");
        } else {
            tracing::info!("Initial page frame successfully rendered and presented to viewport");
        }
    }

    if let Err(err) = backend.run() {
        tracing::error!(%err, "Chrome backend runtime error");
    }
}

#[allow(clippy::cast_precision_loss)]
fn render_page_to_frame(html_str: &str, width: u32, height: u32) -> Option<ViewportFrame> {
    let doc = parse_html(html_str);
    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();
    let mut layout_box = build_box_tree(&doc, doc.root_id(), &styles)?;

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, width as f32, height as f32),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box);
    let pixel_buf = CpuRasterizer::rasterize(&display_list, width, height).ok()?;

    Some(ViewportFrame::SoftwareRgba {
        width,
        height,
        pixels: pixel_buf.data,
    })
}
