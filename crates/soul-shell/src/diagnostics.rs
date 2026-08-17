//! Render diagnostics and accessibility-tree output helpers.

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
