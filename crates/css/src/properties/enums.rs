//! CSS enumerated property types and unit representations.

/// CSS `display` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    /// Block-level box.
    #[default]
    Block,
    /// Inline-level box.
    Inline,
    /// Inline block box.
    InlineBlock,
    /// Flexible box container.
    Flex,
    /// Grid container.
    Grid,
    /// Element and its descendants generate no boxes.
    None,
}

/// CSS `position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    /// Standard in-flow positioning.
    #[default]
    Static,
    /// In-flow box offset relative to normal position.
    Relative,
    /// Out-of-flow box positioned relative to containing block.
    Absolute,
    /// Box positioned relative to the viewport.
    Fixed,
    /// Box toggles between relative and fixed depending on scroll offset.
    Sticky,
}

/// CSS length dimension or keyword.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Automatic sizing.
    #[default]
    Auto,
    /// Absolute pixels.
    Px(f32),
    /// Font-relative em units.
    Em(f32),
    /// Root font-relative rem units.
    Rem(f32),
    /// Percentage relative to containing block.
    Percent(f32),
}

/// CSS `box-sizing` property values (W3C CSS3 UI §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    /// Width and height apply to the content area (default).
    #[default]
    ContentBox,
    /// Width and height include content, padding, and border.
    BorderBox,
}

/// CSS `font-weight` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    /// Normal weight (400).
    #[default]
    Normal,
    /// Bold weight (700).
    Bold,
    /// Numeric font weight (100–900).
    Number(u16),
}

/// CSS `font-style` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    /// Upright text.
    #[default]
    Normal,
    /// Italic text.
    Italic,
    /// Oblique text.
    Oblique,
}

/// CSS `text-decoration` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration {
    /// No text decoration.
    #[default]
    None,
    /// Underline decoration.
    Underline,
    /// Strikethrough decoration.
    LineThrough,
}

/// CSS `text-align` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Align text to the left.
    #[default]
    Left,
    /// Align text to the right.
    Right,
    /// Center text horizontally.
    Center,
    /// Justify text lines.
    Justify,
}
