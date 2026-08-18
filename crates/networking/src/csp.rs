//! Content Security Policy (CSP Level 3) header parser and origin evaluator.

use url::Url;

/// CSP directive kinds controlling resource fetch and execution permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CspDirective {
    /// Fallback directive for all fetch categories (`default-src`).
    DefaultSrc,
    /// Script execution and script fetching (`script-src`).
    ScriptSrc,
    /// Stylesheet fetching and inline styles (`style-src`).
    StyleSrc,
    /// Fetch, XHR, and WebSocket connections (`connect-src`).
    ConnectSrc,
    /// Image asset fetching (`img-src`).
    ImgSrc,
}

/// Allowed source expression within a CSP directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CspSource {
    /// Any origin (`*`).
    Wildcard,
    /// Same origin as the active document (`'self'`).
    SelfOrigin,
    /// Explicitly forbidden (`'none'`).
    None,
    /// Specific scheme prefix (e.g. `https:`, `data:`).
    Scheme(String),
    /// Explicit host match (e.g. `https://api.example.com`).
    Host(String),
    /// Cryptographic nonce source (e.g. `'nonce-abc123'`).
    Nonce(String),
    /// Cryptographic hash source (e.g. `'sha256-...'`).
    Hash(String),
}

/// Structured Content Security Policy violation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CspViolationReport {
    /// Document URI where violation occurred.
    pub document_uri: String,
    /// Resource URI that was blocked.
    pub blocked_uri: String,
    /// Violated directive name.
    pub violated_directive: String,
    /// Original raw policy string.
    pub original_policy: String,
}

/// Parsed Content Security Policy enforcing origin restrictions.
#[derive(Debug, Default, Clone)]
pub struct CspPolicy {
    directives: Vec<(CspDirective, Vec<CspSource>)>,
}

impl CspPolicy {
    /// Parses a `Content-Security-Policy` header value into structured directives.
    #[must_use]
    pub fn parse(header: &str) -> Self {
        let mut directives = Vec::new();

        for policy_part in header.split(';') {
            let mut tokens = policy_part.split_whitespace();
            let Some(dir_name) = tokens.next() else {
                continue;
            };

            let directive_kind = match dir_name.to_ascii_lowercase().as_str() {
                "default-src" => CspDirective::DefaultSrc,
                "script-src" => CspDirective::ScriptSrc,
                "style-src" => CspDirective::StyleSrc,
                "connect-src" => CspDirective::ConnectSrc,
                "img-src" => CspDirective::ImgSrc,
                _ => continue,
            };

            let mut sources = Vec::new();
            for src in tokens {
                let trimmed = src.trim_matches('\'');
                let parsed_src = if src.eq_ignore_ascii_case("*") {
                    CspSource::Wildcard
                } else if src.eq_ignore_ascii_case("'self'") {
                    CspSource::SelfOrigin
                } else if src.eq_ignore_ascii_case("'none'") {
                    CspSource::None
                } else if trimmed.starts_with("nonce-") {
                    CspSource::Nonce(trimmed.trim_start_matches("nonce-").to_string())
                } else if trimmed.starts_with("sha256-") || trimmed.starts_with("sha384-") || trimmed.starts_with("sha512-") {
                    CspSource::Hash(trimmed.to_string())
                } else if src.ends_with(':') {
                    CspSource::Scheme(src.to_ascii_lowercase())
                } else {
                    CspSource::Host(src.to_string())
                };
                sources.push(parsed_src);
            }

            directives.push((directive_kind, sources));
        }

        Self { directives }
    }

    /// Evaluates whether an outgoing resource request is permitted by policy.
    #[must_use]
    pub fn allows(&self, directive: CspDirective, target_url: &Url, doc_origin: &Url) -> bool {
        // Find specific directive sources or fallback to default-src
        let sources = self
            .directives
            .iter()
            .find(|(d, _)| *d == directive)
            .or_else(|| {
                self.directives
                    .iter()
                    .find(|(d, _)| *d == CspDirective::DefaultSrc)
            })
            .map(|(_, s)| s);

        let Some(sources) = sources else {
            // No policy restricting this category
            return true;
        };

        if sources.iter().any(|s| matches!(s, CspSource::None)) {
            return false;
        }

        if sources.iter().any(|s| matches!(s, CspSource::Wildcard)) {
            return true;
        }

        for src in sources {
            match src {
                CspSource::SelfOrigin => {
                    if target_url.origin() == doc_origin.origin() {
                        return true;
                    }
                }
                CspSource::Scheme(scheme) => {
                    let required_scheme = scheme.trim_end_matches(':');
                    if target_url.scheme() == required_scheme {
                        return true;
                    }
                }
                CspSource::Host(host) => {
                    if let Some(target_host) = target_url.host_str()
                        && (target_host == host.as_str() || target_url.as_str().starts_with(host))
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }

    /// Evaluates whether a cryptographic nonce is permitted for inline scripts or styles.
    #[must_use]
    pub fn allows_nonce(&self, directive: CspDirective, nonce: &str) -> bool {
        let sources = self
            .directives
            .iter()
            .find(|(d, _)| *d == directive)
            .or_else(|| {
                self.directives
                    .iter()
                    .find(|(d, _)| *d == CspDirective::DefaultSrc)
            })
            .map(|(_, s)| s);

        let Some(sources) = sources else {
            return true;
        };

        sources.iter().any(|s| match s {
            CspSource::Nonce(allowed) => allowed == nonce,
            _ => false,
        })
    }

    /// Creates a structured CSP violation report for a blocked request.
    #[must_use]
    pub fn create_violation_report(
        &self,
        directive: CspDirective,
        target_url: &Url,
        doc_origin: &Url,
        original_policy: &str,
    ) -> CspViolationReport {
        let directive_name = match directive {
            CspDirective::DefaultSrc => "default-src",
            CspDirective::ScriptSrc => "script-src",
            CspDirective::StyleSrc => "style-src",
            CspDirective::ConnectSrc => "connect-src",
            CspDirective::ImgSrc => "img-src",
        };

        CspViolationReport {
            document_uri: doc_origin.to_string(),
            blocked_uri: target_url.to_string(),
            violated_directive: directive_name.to_string(),
            original_policy: original_policy.to_string(),
        }
    }
}
