//! Display list items, drawing commands, clipping, and opacity primitives.

use css::Color;
use layout::{EdgeSizes, Rect};

/// Atomic display list drawing and state commands emitted by the paint phase.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItem {
    /// Fills a solid rectangle with an RGBA color.
    DrawRect {
        /// Bounding box rectangle to fill.
        rect: Rect,
        /// Fill color.
        color: Color,
    },
    /// Draws four-sided box borders with specified widths and color.
    DrawBorder {
        /// Outer border box rectangle.
        rect: Rect,
        /// Widths of top, right, bottom, left border edges.
        widths: EdgeSizes,
        /// Border line color.
        color: Color,
    },
    /// Renders shaped text content within a bounding box.
    DrawText {
        /// Text bounding box.
        rect: Rect,
        /// UTF-8 string content.
        text: String,
        /// Text fill color.
        color: Color,
        /// Font size in pixels.
        font_size: f32,
        /// Font family name.
        font_family: String,
        /// `true` if text uses bold weight.
        is_bold: bool,
    },
    /// Pushes a rectangular clipping boundary onto the clip stack.
    PushClip {
        /// Clipping rectangle in layout pixels.
        rect: Rect,
    },
    /// Pops the most recently pushed clipping boundary from the clip stack.
    PopClip,
    /// Pushes an alpha opacity multiplier (0.0 to 1.0) onto the opacity stack.
    PushOpacity {
        /// Opacity factor between 0.0 (transparent) and 1.0 (opaque).
        opacity: f32,
    },
    /// Pops the most recently pushed opacity factor.
    PopOpacity,
}

/// Ordered list of display items ready for CPU/GPU rasterization.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayList {
    /// Ordered display commands.
    pub items: Vec<DisplayItem>,
    /// Bounding rectangle of all content in this display list.
    pub bounds: Rect,
}

impl DisplayList {
    /// Creates a new empty `DisplayList`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Appends a `DisplayItem` to the display list.
    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Returns the number of items in the display list.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the display list is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
