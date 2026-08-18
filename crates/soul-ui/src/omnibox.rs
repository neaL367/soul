//! Omnibox input model, autocompletion engine, and URL/Search suggestion ranking.

use storage::{BookmarkEntry, HistoryEntry};
use url::Url;

/// Source category of an omnibox autocompletion suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmniboxSuggestionType {
    /// Exact or direct web URL entered by the user.
    DirectUrl,
    /// Previously visited page in browser history.
    History,
    /// Saved user bookmark.
    Bookmark,
    /// Web search query template.
    Search,
}

/// A single autocompletion suggestion item in the omnibox popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmniboxSuggestion {
    /// User-friendly label/title.
    pub title: String,
    /// Destination URL.
    pub url: String,
    /// Type of suggestion.
    pub suggestion_type: OmniboxSuggestionType,
    /// Ranking score for sorting relevance.
    pub score: u32,
}

/// State model for the omnibox input field and suggestion dropdown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OmniboxModel {
    /// Current text string in the input field.
    pub text: String,
    /// Whether the omnibox currently has keyboard focus.
    pub is_focused: bool,
    /// Active list of autocompletion suggestions.
    pub suggestions: Vec<OmniboxSuggestion>,
    /// Currently highlighted suggestion index in the popup list.
    pub selected_index: Option<usize>,
}

impl OmniboxModel {
    /// Creates a new empty `OmniboxModel`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text: String::new(),
            is_focused: false,
            suggestions: Vec::new(),
            selected_index: None,
        }
    }

    /// Updates the omnibox text and resets selection.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.selected_index = None;
    }

    /// Sets the suggestions list.
    pub fn set_suggestions(&mut self, suggestions: Vec<OmniboxSuggestion>) {
        self.suggestions = suggestions;
        self.selected_index = None;
    }

    /// Navigates selection downwards in the suggestions list.
    pub const fn select_next(&mut self) {
        if self.suggestions.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = match self.selected_index {
            Some(idx) if idx + 1 < self.suggestions.len() => Some(idx + 1),
            Some(_) | None => Some(0),
        };
    }

    /// Navigates selection upwards in the suggestions list.
    pub const fn select_prev(&mut self) {
        if self.suggestions.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = match self.selected_index {
            Some(0) | None => Some(self.suggestions.len() - 1),
            Some(idx) => Some(idx - 1),
        };
    }

    /// Returns the URL of the selected suggestion if one is active, otherwise the raw text.
    #[must_use]
    pub fn target_url(&self) -> String {
        if let Some(idx) = self.selected_index
            && let Some(suggestion) = self.suggestions.get(idx)
        {
            suggestion.url.clone()
        } else {
            self.text.clone()
        }
    }
}

/// Scoring and suggestion engine generating autocompletions for user omnibox input.
#[derive(Debug, Clone)]
pub struct OmniboxEngine {
    search_template: String,
}

impl Default for OmniboxEngine {
    fn default() -> Self {
        Self::new(None)
    }
}

impl OmniboxEngine {
    /// Creates an `OmniboxEngine` with an optional custom search URL template.
    #[must_use]
    pub fn new(search_template: Option<String>) -> Self {
        Self {
            search_template: search_template
                .unwrap_or_else(|| "https://duckduckgo.com/?q={}".to_string()),
        }
    }

    /// Generates and ranks autocompletion suggestions against history and bookmarks.
    #[must_use]
    pub fn generate_suggestions(
        &self,
        input: &str,
        history: &[HistoryEntry],
        bookmarks: &[BookmarkEntry],
    ) -> Vec<OmniboxSuggestion> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let mut suggestions = Vec::new();

        // 1. Direct URL check
        if let Some(direct_url) = Self::parse_direct_url(trimmed) {
            suggestions.push(OmniboxSuggestion {
                title: format!("Go to {direct_url}"),
                url: direct_url,
                suggestion_type: OmniboxSuggestionType::DirectUrl,
                score: 100,
            });
        }

        // 2. Bookmark matching
        let lower = trimmed.to_ascii_lowercase();
        for b in bookmarks {
            if b.title.to_ascii_lowercase().contains(&lower)
                || b.url.to_ascii_lowercase().contains(&lower)
            {
                suggestions.push(OmniboxSuggestion {
                    title: b.title.clone(),
                    url: b.url.clone(),
                    suggestion_type: OmniboxSuggestionType::Bookmark,
                    score: 80,
                });
            }
        }

        // 3. History matching
        for h in history {
            let matches_title = h
                .title
                .as_ref()
                .is_some_and(|t| t.to_ascii_lowercase().contains(&lower));
            let matches_url = h.url.to_ascii_lowercase().contains(&lower);

            if matches_title || matches_url {
                let score = 50 + h.visit_count.min(30);
                suggestions.push(OmniboxSuggestion {
                    title: h.title.clone().unwrap_or_else(|| h.url.clone()),
                    url: h.url.clone(),
                    suggestion_type: OmniboxSuggestionType::History,
                    score,
                });
            }
        }

        // 4. Web search suggestion
        let search_url = self
            .search_template
            .replace("{}", &encode_query_value(trimmed));
        suggestions.push(OmniboxSuggestion {
            title: format!("Search: {trimmed}"),
            url: search_url,
            suggestion_type: OmniboxSuggestionType::Search,
            score: 20,
        });

        // Deduplicate URLs keeping highest score
        suggestions.sort_by_key(|b| std::cmp::Reverse(b.score));
        let mut deduped: Vec<OmniboxSuggestion> = Vec::new();
        for s in suggestions {
            if !deduped.iter().any(|existing| existing.url == s.url) {
                deduped.push(s);
            }
            if deduped.len() >= 6 {
                break;
            }
        }

        deduped
    }

    fn parse_direct_url(input: &str) -> Option<String> {
        if input.starts_with("http://") || input.starts_with("https://") {
            if let Ok(u) = Url::parse(input) {
                return Some(u.to_string());
            }
        } else if !input.contains(' ') && (input.contains('.') || input.starts_with("localhost")) {
            let candidate = format!("https://{input}");
            if let Ok(u) = Url::parse(&candidate) {
                return Some(u.to_string());
            }
        }
        None
    }
}

/// Percent-encodes a raw user query so it cannot alter the search URL structure.
fn encode_query_value(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
