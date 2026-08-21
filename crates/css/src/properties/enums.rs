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

/// CSS `flex-direction` property (CSS Flexbox §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    /// Main axis is horizontal left-to-right.
    #[default]
    Row,
    /// Main axis is horizontal right-to-left.
    RowReverse,
    /// Main axis is vertical top-to-bottom.
    Column,
    /// Main axis is vertical bottom-to-top.
    ColumnReverse,
}

/// CSS `flex-wrap` property (CSS Flexbox §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    /// All flex items on one line.
    #[default]
    NoWrap,
    /// Items wrap to the next line.
    Wrap,
    /// Items wrap in the reverse direction.
    WrapReverse,
}

/// CSS `justify-content` property (CSS Flexbox §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    /// Items packed at the start of the main axis.
    #[default]
    FlexStart,
    /// Items packed at the end of the main axis.
    FlexEnd,
    /// Items centered on the main axis.
    Center,
    /// Space between items, no space at edges.
    SpaceBetween,
    /// Equal space around each item.
    SpaceAround,
    /// Equal space between all items and edges.
    SpaceEvenly,
}

/// CSS `align-items` property (CSS Flexbox §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    /// Items stretched to fill the cross-axis height.
    #[default]
    Stretch,
    /// Items aligned at the start of the cross axis.
    FlexStart,
    /// Items aligned at the end of the cross axis.
    FlexEnd,
    /// Items centered on the cross axis.
    Center,
    /// Items aligned to their baselines.
    Baseline,
}

/// CSS `align-self` property — overrides `align-items` on individual flex items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignSelf {
    /// Inherit parent's `align-items`.
    #[default]
    Auto,
    /// Stretch to fill the cross axis.
    Stretch,
    /// Align at the start of the cross axis.
    FlexStart,
    /// Align at the end of the cross axis.
    FlexEnd,
    /// Center on the cross axis.
    Center,
    /// Align to baseline.
    Baseline,
}

/// CSS Grid track sizing function.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridTrack {
    /// `auto` track.
    #[default]
    Auto,
    /// Fixed pixel length.
    Px(f32),
    /// Fractional `fr` unit.
    Fr(f32),
    /// Percentage of container.
    Percent(f32),
}

impl GridTrack {
    /// Returns the `fr` value if this is an `Fr` track.
    #[must_use]
    pub const fn to_fr(self) -> Option<f32> {
        match self {
            Self::Fr(v) => Some(v),
            _ => None,
        }
    }

    /// Returns the percentage value if this is a `Percent` track.
    #[must_use]
    pub const fn to_percent(self) -> Option<f32> {
        match self {
            Self::Percent(v) => Some(v),
            _ => None,
        }
    }
}
