//! CSS box model geometric primitives: rectangles, edge sizes, and dimensions.

/// 2D axis-aligned rectangle with origin and size in layout pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Horizontal origin coordinate in pixels.
    pub x: f32,
    /// Vertical origin coordinate in pixels.
    pub y: f32,
    /// Width dimension in pixels.
    pub width: f32,
    /// Height dimension in pixels.
    pub height: f32,
}

impl Rect {
    /// Creates a new `Rect`.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns `true` if this rectangle contains the given point.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// Sizing applied to the four edges of a box (e.g., margins, paddings, borders).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeSizes {
    /// Top edge in pixels.
    pub top: f32,
    /// Right edge in pixels.
    pub right: f32,
    /// Bottom edge in pixels.
    pub bottom: f32,
    /// Left edge in pixels.
    pub left: f32,
}

impl EdgeSizes {
    /// Creates a new `EdgeSizes` structure.
    #[must_use]
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Returns the sum of left and right edges.
    #[must_use]
    pub const fn horizontal_total(&self) -> f32 {
        self.left + self.right
    }

    /// Returns the sum of top and bottom edges.
    #[must_use]
    pub const fn vertical_total(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Complete CSS box model dimensions comprising content, padding, border, and margin.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dimensions {
    /// Inner content area rectangle.
    pub content: Rect,
    /// Padding edge sizes.
    pub padding: EdgeSizes,
    /// Border edge sizes.
    pub border: EdgeSizes,
    /// Margin edge sizes.
    pub margin: EdgeSizes,
}

impl Dimensions {
    /// Returns the rectangle encompassing content and padding.
    #[must_use]
    pub fn padding_box(&self) -> Rect {
        Rect::new(
            self.content.x - self.padding.left,
            self.content.y - self.padding.top,
            self.content.width + self.padding.horizontal_total(),
            self.content.height + self.padding.vertical_total(),
        )
    }

    /// Returns the rectangle encompassing content, padding, and borders.
    #[must_use]
    pub fn border_box(&self) -> Rect {
        let p_box = self.padding_box();
        Rect::new(
            p_box.x - self.border.left,
            p_box.y - self.border.top,
            p_box.width + self.border.horizontal_total(),
            p_box.height + self.border.vertical_total(),
        )
    }

    /// Returns the rectangle encompassing content, padding, borders, and margins.
    #[must_use]
    pub fn margin_box(&self) -> Rect {
        let b_box = self.border_box();
        Rect::new(
            b_box.x - self.margin.left,
            b_box.y - self.margin.top,
            b_box.width + self.margin.horizontal_total(),
            b_box.height + self.margin.vertical_total(),
        )
    }
}
