//! CSS Media Queries Level 5 (`@media`) and user preference types.

/// User color scheme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    /// Standard light color scheme.
    #[default]
    Light,
    /// Dark mode color scheme.
    Dark,
    /// No specific preference expressed by the user.
    NoPreference,
}

/// Evaluated media condition attached to a CSS `@media` rule block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaCondition {
    /// Matches user's preferred color scheme (`dark` / `light`).
    PrefersColorScheme(ColorScheme),
    /// Condition that always evaluates to true.
    Always,
}

impl MediaCondition {
    /// Evaluates this media condition against the active environment settings.
    #[must_use]
    pub fn matches(&self, active_scheme: ColorScheme) -> bool {
        match self {
            Self::Always => true,
            Self::PrefersColorScheme(target_scheme) => *target_scheme == active_scheme,
        }
    }
}

/// Parses a media query header string (e.g., `(prefers-color-scheme: dark)`).
#[must_use]
pub fn parse_media_condition(query: &str) -> Option<MediaCondition> {
    let lower = query.trim().to_ascii_lowercase();
    let inner = lower.strip_prefix("@media")?.trim();

    if inner.contains("prefers-color-scheme") {
        if inner.contains("dark") {
            return Some(MediaCondition::PrefersColorScheme(ColorScheme::Dark));
        } else if inner.contains("light") {
            return Some(MediaCondition::PrefersColorScheme(ColorScheme::Light));
        } else if inner.contains("no-preference") {
            return Some(MediaCondition::PrefersColorScheme(
                ColorScheme::NoPreference,
            ));
        }
    }

    None
}
