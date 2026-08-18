//! CSS property definitions, models, and resolved `ComputedStyle` structures.

pub mod color;
pub mod enums;

pub use color::Color;
pub use enums::{
    AlignItems, AlignSelf, BoxSizing, Display, FlexDirection, FlexWrap, FontStyle, FontWeight,
    JustifyContent, Length, Position, TextAlign, TextDecoration,
};

/// Fully resolved computed styles for a single DOM element.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    /// Layout display type.
    pub display: Display,
    /// Positioning scheme.
    pub position: Position,
    /// Box sizing model (`ContentBox` vs `BorderBox`).
    pub box_sizing: BoxSizing,
    /// Text foreground color (inherited).
    pub color: Color,
    /// Background color.
    pub background_color: Color,
    /// Element opacity (0.0 to 1.0).
    pub opacity: f32,
    /// Font size in resolved pixels (inherited).
    pub font_size: f32,
    /// Primary font family name (inherited).
    pub font_family: String,
    /// Font weight (inherited).
    pub font_weight: FontWeight,
    /// Font style: normal or italic (inherited).
    pub font_style: FontStyle,
    /// Text decoration: none, underline, or line-through (inherited).
    pub text_decoration: TextDecoration,
    /// Text alignment (inherited).
    pub text_align: TextAlign,
    /// Line height in resolved pixels or relative multiplier (inherited).
    pub line_height: Option<f32>,
    /// Letter spacing in resolved pixels (inherited).
    pub letter_spacing: f32,
    /// Word spacing in resolved pixels (inherited).
    pub word_spacing: f32,
    /// Margin top in resolved pixels.
    pub margin_top: f32,
    /// Margin right in resolved pixels.
    pub margin_right: f32,
    /// Margin bottom in resolved pixels.
    pub margin_bottom: f32,
    /// Margin left in resolved pixels.
    pub margin_left: f32,
    /// Padding top in resolved pixels.
    pub padding_top: f32,
    /// Padding right in resolved pixels.
    pub padding_right: f32,
    /// Padding bottom in resolved pixels.
    pub padding_bottom: f32,
    /// Padding left in resolved pixels.
    pub padding_left: f32,
    /// Border width top in resolved pixels.
    pub border_top_width: f32,
    /// Border width right in resolved pixels.
    pub border_right_width: f32,
    /// Border width bottom in resolved pixels.
    pub border_bottom_width: f32,
    /// Border width left in resolved pixels.
    pub border_left_width: f32,
    /// Border color top.
    pub border_top_color: Color,
    /// Border color right.
    pub border_right_color: Color,
    /// Border color bottom.
    pub border_bottom_color: Color,
    /// Border color left.
    pub border_left_color: Color,
    /// Border radius top-left in resolved pixels.
    pub border_radius_top_left: f32,
    /// Border radius top-right in resolved pixels.
    pub border_radius_top_right: f32,
    /// Border radius bottom-right in resolved pixels.
    pub border_radius_bottom_right: f32,
    /// Border radius bottom-left in resolved pixels.
    pub border_radius_bottom_left: f32,
    /// Width dimension.
    pub width: Length,
    /// Height dimension.
    pub height: Length,
    /// Stacking order z-index.
    pub z_index: Option<i32>,
    // ── Flex / Grid ──────────────────────────────────────────────────────────
    /// Main-axis direction for flex containers.
    pub flex_direction: FlexDirection,
    /// Wrapping behaviour for flex containers.
    pub flex_wrap: FlexWrap,
    /// Main-axis alignment for flex containers.
    pub justify_content: JustifyContent,
    /// Cross-axis alignment for flex containers (applies to children).
    pub align_items: AlignItems,
    /// Cross-axis self-alignment override for flex items.
    pub align_self: AlignSelf,
    /// Flex grow factor for flex items.
    pub flex_grow: f32,
    /// Flex shrink factor for flex items.
    pub flex_shrink: f32,
    /// Flex basis: the initial main-size before flex adjustment.
    pub flex_basis: Length,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self::initial()
    }
}

impl ComputedStyle {
    /// Returns the standard initial CSS computed style for root or default elements.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            display: Display::Inline,
            position: Position::Static,
            box_sizing: BoxSizing::ContentBox,
            color: Color::BLACK,
            background_color: Color::TRANSPARENT,
            opacity: 1.0,
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            text_decoration: TextDecoration::None,
            text_align: TextAlign::Left,
            line_height: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            border_top_width: 0.0,
            border_right_width: 0.0,
            border_bottom_width: 0.0,
            border_left_width: 0.0,
            border_top_color: Color::BLACK,
            border_right_color: Color::BLACK,
            border_bottom_color: Color::BLACK,
            border_left_color: Color::BLACK,
            border_radius_top_left: 0.0,
            border_radius_top_right: 0.0,
            border_radius_bottom_right: 0.0,
            border_radius_bottom_left: 0.0,
            width: Length::Auto,
            height: Length::Auto,
            z_index: None,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_self: AlignSelf::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
        }
    }

    /// Inherits inherited properties from parent computed style.
    pub fn inherit_from(&mut self, parent: &Self) {
        self.color = parent.color;
        self.font_size = parent.font_size;
        self.font_family.clone_from(&parent.font_family);
        self.font_weight = parent.font_weight;
        self.font_style = parent.font_style;
        self.text_decoration = parent.text_decoration;
        self.text_align = parent.text_align;
        self.line_height = parent.line_height;
        self.letter_spacing = parent.letter_spacing;
        self.word_spacing = parent.word_spacing;
    }
}
