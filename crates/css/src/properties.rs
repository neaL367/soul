//! CSS property definitions, color models, and resolved `ComputedStyle` structures.

/// 8-bit RGBA color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red channel (0–255).
    pub r: u8,
    /// Green channel (0–255).
    pub g: u8,
    /// Blue channel (0–255).
    pub b: u8,
    /// Alpha channel (0–255, where 255 is fully opaque).
    pub a: u8,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Creates a new `Color` from RGBA components.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a new `Color` from RGB with full opacity (255).
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses a CSS color string (e.g. "#ff0000", "#f00", "rgb(255, 0, 0)", "red", "transparent").
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "transparent" => Some(Self::TRANSPARENT),
            "black" => Some(Self::BLACK),
            "white" => Some(Self::WHITE),
            "red" => Some(Self::rgb(255, 0, 0)),
            "green" => Some(Self::rgb(0, 128, 0)),
            "blue" => Some(Self::rgb(0, 0, 255)),
            "yellow" => Some(Self::rgb(255, 255, 0)),
            "gray" | "grey" => Some(Self::rgb(128, 128, 128)),
            _ if trimmed.starts_with('#') => Self::parse_hex(&trimmed[1..]),
            _ if trimmed.starts_with("rgb") => Self::parse_rgb_fn(&trimmed),
            _ => None,
        }
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn parse_rgb_fn(s: &str) -> Option<Self> {
        let inside = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?;
        let inside = inside.strip_suffix(')')?;
        let parts: Vec<&str> = inside.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            Some(Self::rgb(r, g, b))
        } else if parts.len() == 4 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            let alpha_f = parts[3].parse::<f32>().ok()?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8;
            Some(Self::rgba(r, g, b, a))
        } else {
            None
        }
    }
}

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

/// Fully resolved computed styles for a single DOM element.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    /// Layout display type.
    pub display: Display,
    /// Positioning scheme.
    pub position: Position,
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
    /// Text alignment (inherited).
    pub text_align: TextAlign,
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
    /// Width dimension.
    pub width: Length,
    /// Height dimension.
    pub height: Length,
    /// Stacking order z-index.
    pub z_index: Option<i32>,
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
            color: Color::BLACK,
            background_color: Color::TRANSPARENT,
            opacity: 1.0,
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            font_weight: FontWeight::Normal,
            text_align: TextAlign::Left,
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
            width: Length::Auto,
            height: Length::Auto,
            z_index: None,
        }
    }

    /// Inherits inherited properties (color, font, text-align) from parent computed style.
    pub fn inherit_from(&mut self, parent: &Self) {
        self.color = parent.color;
        self.font_size = parent.font_size;
        self.font_family.clone_from(&parent.font_family);
        self.font_weight = parent.font_weight;
        self.text_align = parent.text_align;
    }
}
