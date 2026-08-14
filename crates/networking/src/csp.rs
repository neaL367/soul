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
                let parsed_src = match src.to_ascii_lowercase().as_str() {
                    "*" => CspSource::Wildcard,
                    "'self'" => CspSource::SelfOrigin,
                    "'none'" => CspSource::None,
                    s if s.ends_with(':') => CspSource::Scheme(s.to_string()),
                    s => CspSource::Host(s.to_string()),
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
}
