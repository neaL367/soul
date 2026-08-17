//! Built-in, engine-rendered pages owned by the browser shell.

use crate::engine::{RenderOptions, render_html_to_buffer};
use soul_core::NavigationError;
use soul_ui::ViewportFrame;

const NEW_TAB_HTML: &str = r"<!DOCTYPE html>
<html>
<head>
  <title>New Tab</title>
  <style>
    body { background-color: #f6f8fb; color: #1f2937; }
    h1 { color: #111827; }
    p { color: #4b5563; }
  </style>
</head>
<body>
  <h1>New Tab</h1>
  <p>Enter a URL in the address bar to start browsing.</p>
</body>
</html>";

/// Renders the static page shown for a newly-created tab.
///
/// # Errors
///
/// Returns `NavigationError` when the local HTML cannot be rendered.
pub fn render_new_tab_frame(options: RenderOptions) -> Result<ViewportFrame, NavigationError> {
    let (buffer, _, _) = render_html_to_buffer(NEW_TAB_HTML, options)?;
    Ok(ViewportFrame::SoftwareRgba {
        width: buffer.width,
        height: buffer.height,
        pixels: buffer.data,
    })
}
