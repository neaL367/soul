//! CSS relative unit resolution and `calc()` mathematical expression evaluation.

use css::Length;

/// Layout environment metrics required for resolving relative length units and expressions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthContext {
    /// Percentage base dimension in pixels (e.g. containing block width or height).
    pub percentage_basis: f32,
    /// Element font size in pixels (for `em` units).
    pub font_size: f32,
    /// Root `<html>` element font size in pixels (for `rem` units).
    pub root_font_size: f32,
    /// Viewport width in pixels (for `vw` units).
    pub viewport_width: f32,
    /// Viewport height in pixels (for `vh` units).
    pub viewport_height: f32,
}

impl Default for LengthContext {
    fn default() -> Self {
        Self {
            percentage_basis: 0.0,
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 800.0,
            viewport_height: 600.0,
        }
    }
}

impl LengthContext {
    /// Creates a new `LengthContext` with a specific percentage basis and viewport.
    #[must_use]
    pub const fn new(percentage_basis: f32, viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            percentage_basis,
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width,
            viewport_height,
        }
    }

    /// Sets the element and root font sizes.
    #[must_use]
    pub const fn with_font_sizes(mut self, font_size: f32, root_font_size: f32) -> Self {
        self.font_size = font_size;
        self.root_font_size = root_font_size;
        self
    }
}

/// Resolves any CSS `Length` into concrete layout pixels using the provided context.
#[must_use]
pub fn resolve_length(length: &Length, ctx: &LengthContext) -> Option<f32> {
    match length {
        Length::Auto => None,
        Length::Px(px) => Some(*px),
        Length::Em(em) => Some(*em * ctx.font_size),
        Length::Rem(rem) => Some(*rem * ctx.root_font_size),
        Length::Vw(vw) => Some(*vw * ctx.viewport_width / 100.0),
        Length::Vh(vh) => Some(*vh * ctx.viewport_height / 100.0),
        Length::Percent(pct) => Some(*pct * ctx.percentage_basis / 100.0),
        Length::Calc(expr) => evaluate_calc(expr, ctx),
    }
}

/// Evaluates a CSS `calc(...)` mathematical expression into resolved pixels.
#[must_use]
pub fn evaluate_calc(expr: &str, ctx: &LengthContext) -> Option<f32> {
    let tokens = tokenize_calc(expr, ctx)?;
    let mut parser = CalcParser::new(&tokens);
    parser.parse_expression()
}

#[derive(Debug, Clone, PartialEq)]
enum CalcToken {
    Number(f32),
    Plus,
    Minus,
    Multiply,
    Divide,
    OpenParen,
    CloseParen,
}

fn tokenize_calc(expr: &str, ctx: &LengthContext) -> Option<Vec<CalcToken>> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        match ch {
            '+' => {
                tokens.push(CalcToken::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(CalcToken::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(CalcToken::Multiply);
                chars.next();
            }
            '/' => {
                tokens.push(CalcToken::Divide);
                chars.next();
            }
            '(' => {
                tokens.push(CalcToken::OpenParen);
                chars.next();
            }
            ')' => {
                tokens.push(CalcToken::CloseParen);
                chars.next();
            }
            _ => {
                let mut token_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '+' | '-' | '*' | '/' | '(' | ')') {
                        break;
                    }
                    token_str.push(c);
                    chars.next();
                }

                if token_str.is_empty() {
                    return None;
                }

                let num_val = parse_token_value(&token_str, ctx)?;
                tokens.push(num_val);
            }
        }
    }

    Some(tokens)
}

fn parse_token_value(s: &str, ctx: &LengthContext) -> Option<CalcToken> {
    let trimmed = s.trim();
    if let Some(num) = trimmed.strip_suffix("rem") {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n * ctx.root_font_size));
    }
    if let Some(num) = trimmed.strip_suffix("em") {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n * ctx.font_size));
    }
    if let Some(num) = trimmed.strip_suffix("vw") {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n * ctx.viewport_width / 100.0));
    }
    if let Some(num) = trimmed.strip_suffix("vh") {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n * ctx.viewport_height / 100.0));
    }
    if let Some(num) = trimmed.strip_suffix('%') {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n * ctx.percentage_basis / 100.0));
    }
    if let Some(num) = trimmed.strip_suffix("px") {
        let n = num.trim().parse::<f32>().ok()?;
        return Some(CalcToken::Number(n));
    }
    if let Ok(n) = trimmed.parse::<f32>() {
        return Some(CalcToken::Number(n));
    }
    None
}

struct CalcParser<'a> {
    tokens: &'a [CalcToken],
    pos: usize,
}

impl<'a> CalcParser<'a> {
    const fn new(tokens: &'a [CalcToken]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&CalcToken> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&CalcToken> {
        let tok = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(tok)
    }

    fn parse_expression(&mut self) -> Option<f32> {
        let mut left = self.parse_term()?;

        while let Some(tok) = self.peek() {
            match tok {
                CalcToken::Plus => {
                    self.next();
                    let right = self.parse_term()?;
                    left += right;
                }
                CalcToken::Minus => {
                    self.next();
                    let right = self.parse_term()?;
                    left -= right;
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_term(&mut self) -> Option<f32> {
        let mut left = self.parse_factor()?;

        while let Some(tok) = self.peek() {
            match tok {
                CalcToken::Multiply => {
                    self.next();
                    let right = self.parse_factor()?;
                    left *= right;
                }
                CalcToken::Divide => {
                    self.next();
                    let right = self.parse_factor()?;
                    if right == 0.0 {
                        return None;
                    }
                    left /= right;
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_factor(&mut self) -> Option<f32> {
        match self.next()? {
            CalcToken::Number(n) => Some(*n),
            CalcToken::OpenParen => {
                let val = self.parse_expression()?;
                if self.next() != Some(&CalcToken::CloseParen) {
                    return None;
                }
                Some(val)
            }
            _ => None,
        }
    }
}
