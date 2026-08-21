//! Render diagnostics, system hardware information, and internal about: page helpers.

use layout::A11yNode;
use raster::PixelBuffer;

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

/// Returns `true` if the pixel buffer contains any non-transparent pixel.
#[must_use]
pub fn has_visible_pixels(buffer: &PixelBuffer) -> bool {
    buffer.data.chunks_exact(4).any(|px| px[3] != 0)
}

/// Host system and browser runtime diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDiagnostics {
    /// Browser application version string.
    pub browser_version: String,
    /// Host operating system identifier.
    pub os_name: String,
    /// Processor architecture (e.g. "`x86_64`").
    pub os_arch: String,
    /// Number of logical CPU cores available.
    pub logical_cores: usize,
    /// Compiler edition and toolchain target.
    pub target_triple: String,
}

impl SystemDiagnostics {
    /// Gathers current runtime host system metrics.
    #[must_use]
    pub fn current() -> Self {
        Self {
            browser_version: env!("CARGO_PKG_VERSION").to_string(),
            os_name: std::env::consts::OS.to_string(),
            os_arch: std::env::consts::ARCH.to_string(),
            logical_cores: std::thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get),
            target_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        }
    }

    /// Generates standard HTML markup for the `about:version` diagnostic page.
    #[must_use]
    pub fn render_about_version_html(&self) -> String {
        format!(
            r#"<!DOCTYPE html><html><head><title>About Soul</title><style>body {{ font-family: sans-serif; padding: 24px; }} h1 {{ color: #185fa5; }} table {{ border-collapse: collapse; width: 100%; max-width: 600px; }} td {{ padding: 8px; border-bottom: 1px solid #e2e8f0; }} td.key {{ font-weight: bold; width: 180px; }}</style></head><body><h1>Soul Browser</h1><table><tr><td class="key">Version</td><td>{}</td></tr><tr><td class="key">Operating System</td><td>{} ({})</td></tr><tr><td class="key">Logical Cores</td><td>{}</td></tr><tr><td class="key">Target</td><td>{}</td></tr></table></body></html>"#,
            self.browser_version,
            self.os_name,
            self.os_arch,
            self.logical_cores,
            self.target_triple
        )
    }

    /// Generates standard HTML markup for the `about:gpu` diagnostic page.
    #[must_use]
    pub fn render_about_gpu_html(adapter_summary: &str) -> String {
        format!(
            r"<!DOCTYPE html><html><head><title>Graphics Diagnostics</title><style>body {{ font-family: sans-serif; padding: 24px; }} h1 {{ color: #185fa5; }} pre {{ background: #f8fafc; padding: 12px; border-radius: 6px; border: 1px solid #e2e8f0; }}</style></head><body><h1>Graphics Diagnostics</h1><h2>GPU Adapter & Compositor</h2><pre>{adapter_summary}</pre></body></html>"
        )
    }

    /// Generates standard HTML markup for the `about:crashes` diagnostic page.
    #[must_use]
    pub fn render_about_crashes_html(crash_reports: &[String]) -> String {
        use std::fmt::Write;

        let mut rows = String::new();
        if crash_reports.is_empty() {
            rows.push_str("<p>No crash logs recorded.</p>");
        } else {
            for (idx, report) in crash_reports.iter().enumerate() {
                let count = idx + 1;
                let _ = write!(
                    rows,
                    r#"<div style="border:1px solid #e2e8f0; border-radius:6px; padding:12px; margin-bottom:12px;"><div style="font-weight:bold; color:#e11d48;">Crash #{count}</div><pre style="margin-top:6px; font-size:12px; background:#f8fafc; padding:8px;">{report}</pre></div>"#
                );
            }
        }

        format!(
            r"<!DOCTYPE html><html><head><title>Crash Reports</title><style>body {{ font-family: sans-serif; padding: 24px; }} h1 {{ color: #185fa5; }}</style></head><body><h1>Crash Reports</h1>{rows}</body></html>"
        )
    }
}
