//! CSS Color model, parsers (Hex, RGB, RGBA, HSL, HSLA), and standard named colors palette.

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

    /// Parses a CSS color string (Hex, RGB, RGBA, HSL, HSLA, or Named Color).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_ascii_lowercase();
        if let Some(named) = Self::parse_named(&trimmed) {
            return Some(named);
        }
        if let Some(hex) = trimmed.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        if trimmed.starts_with("rgb") {
            return Self::parse_rgb_fn(&trimmed);
        }
        if trimmed.starts_with("hsl") {
            return Self::parse_hsl_fn(&trimmed);
        }
        None
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        // Reject any non-ASCII input before slicing: byte-length checks below
        // assume one byte per hex digit, and slicing a multibyte char boundary
        // would panic on untrusted CSS input.
        if !hex.is_ascii() {
            return None;
        }
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                Some(Self::rgba(r, g, b, a))
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
            if !alpha_f.is_finite() {
                return None;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8;
            Some(Self::rgba(r, g, b, a))
        } else {
            None
        }
    }

    #[allow(
        clippy::many_single_char_names,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::suboptimal_flops
    )]
    fn parse_hsl_fn(s: &str) -> Option<Self> {
        let inside = s.strip_prefix("hsla(").or_else(|| s.strip_prefix("hsl("))?;
        let inside = inside.strip_suffix(')')?;
        let parts: Vec<&str> = inside.split(',').map(str::trim).collect();
        if parts.len() < 3 {
            return None;
        }
        let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
        let s = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        if !h.is_finite() || !s.is_finite() || !l.is_finite() {
            return None;
        }
        // Negative hues wrap around the color wheel per CSS Color 4; NaN is rejected above.
        let h = h.rem_euclid(360.0);
        let a = if parts.len() >= 4 {
            let alpha_f = parts[3].trim_end_matches('%').parse::<f32>().ok()?;
            if !alpha_f.is_finite() {
                return None;
            }
            (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            255
        };

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;

        let (r1, g1, b1) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Some(Self::rgba(
            ((r1 + m) * 255.0).round() as u8,
            ((g1 + m) * 255.0).round() as u8,
            ((b1 + m) * 255.0).round() as u8,
            a,
        ))
    }

    /// Parses a standard CSS named color string (e.g. "red", "cornflowerblue", "transparent").
    #[must_use]
    pub fn parse_named(name: &str) -> Option<Self> {
        match name {
            "transparent" => Some(Self::TRANSPARENT),
            "black" => Some(Self::BLACK),
            "white" => Some(Self::WHITE),
            "red" => Some(Self::rgb(255, 0, 0)),
            "green" => Some(Self::rgb(0, 128, 0)),
            "blue" => Some(Self::rgb(0, 0, 255)),
            "yellow" => Some(Self::rgb(255, 255, 0)),
            "cyan" | "aqua" => Some(Self::rgb(0, 255, 255)),
            "magenta" | "fuchsia" => Some(Self::rgb(255, 0, 255)),
            "gray" | "grey" => Some(Self::rgb(128, 128, 128)),
            "silver" => Some(Self::rgb(192, 192, 192)),
            "maroon" => Some(Self::rgb(128, 0, 0)),
            "olive" => Some(Self::rgb(128, 128, 0)),
            "navy" => Some(Self::rgb(0, 0, 128)),
            "purple" => Some(Self::rgb(128, 0, 128)),
            "teal" => Some(Self::rgb(0, 128, 128)),
            "orange" => Some(Self::rgb(255, 165, 0)),
            "pink" => Some(Self::rgb(255, 192, 203)),
            "gold" => Some(Self::rgb(255, 215, 0)),
            "indigo" => Some(Self::rgb(75, 0, 130)),
            "violet" => Some(Self::rgb(238, 130, 238)),
            "coral" => Some(Self::rgb(255, 127, 80)),
            "brown" => Some(Self::rgb(165, 42, 42)),
            "crimson" => Some(Self::rgb(220, 20, 60)),
            "cornflowerblue" => Some(Self::rgb(100, 149, 237)),
            _ => None,
        }
    }
}
