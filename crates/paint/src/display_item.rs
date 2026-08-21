//! Display list items, drawing commands, clipping, and opacity primitives.

use css::{BoxShadow, Color};
use layout::{EdgeSizes, Rect};

/// Atomic display list drawing and state commands emitted by the paint phase.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItem {
    /// Draws one or more CSS box shadow layers around a rectangle.
    DrawBoxShadow {
        /// Outer border box rectangle.
        rect: Rect,
        /// Box shadow layer definitions.
        shadows: Vec<BoxShadow>,
    },
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
    /// Draws a decoded RGBA bitmap image within a bounding box.
    DrawImage {
        /// Destination bounding box in layout pixels.
        rect: Rect,
        /// Natural image width in pixels.
        width: u32,
        /// Natural image height in pixels.
        height: u32,
        /// RGBA8 pixel bytes (`width * height * 4`).
        pixels: Vec<u8>,
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

impl DisplayItem {
    /// Returns the spatial bounding rectangle of this display item if it is a visual drawing item.
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Self::DrawRect { rect, .. }
            | Self::DrawBorder { rect, .. }
            | Self::DrawText { rect, .. }
            | Self::DrawImage { rect, .. }
            | Self::PushClip { rect } => Some(*rect),
            Self::DrawBoxShadow { rect, shadows } => {
                let mut b = *rect;
                for s in shadows {
                    if !s.inset {
                        let expand = s.spread_radius + s.blur_radius;
                        let s_rect = Rect::new(
                            rect.x + s.offset_x - expand,
                            rect.y + s.offset_y - expand,
                            expand.mul_add(2.0, rect.width),
                            expand.mul_add(2.0, rect.height),
                        );
                        b = b.union(&s_rect);
                    }
                }
                Some(b)
            }
            Self::PopClip | Self::PushOpacity { .. } | Self::PopOpacity => None,
        }
    }
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

    /// Appends a `DisplayItem` to the display list and expands the total bounds.
    pub fn push(&mut self, item: DisplayItem) {
        if let Some(item_bounds) = item.bounds() {
            if self.items.is_empty() {
                self.bounds = item_bounds;
            } else {
                self.bounds = self.bounds.union(&item_bounds);
            }
        }
        self.items.push(item);
    }

    /// Returns a new `DisplayList` containing only items that intersect the given viewport.
    #[must_use]
    pub fn cull_to_viewport(&self, viewport: Rect) -> Self {
        let mut culled = Self::new();
        culled.bounds = self.bounds;

        for item in &self.items {
            match item.bounds() {
                Some(item_bounds) if !item_bounds.intersects(&viewport) => {
                    // Item is outside viewport - cull visual items
                    if matches!(item, DisplayItem::PushClip { .. }) {
                        culled.items.push(item.clone());
                    }
                }
                _ => {
                    culled.items.push(item.clone());
                }
            }
        }

        culled
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
