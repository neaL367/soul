//! HTML5 document parsing entry points using `html5ever`.

use crate::sink::HtmlTreeSink;
use dom::Document;
use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, parse_document};

/// Complete extracted document structure and subresource declarations.
#[derive(Debug, Clone)]
pub struct ParsedHtmlDocument {
    /// Arena DOM document.
    pub document: Document,
    /// Author inline `<style>` CSS contents in document order.
    pub inline_styles: Vec<String>,
    /// External stylesheet URLs from `<link rel="stylesheet" href="...">`.
    pub stylesheet_links: Vec<String>,
    /// Inline script bodies in document order.
    pub inline_scripts: Vec<String>,
    /// External script URLs from `<script src="...">`.
    pub external_scripts: Vec<String>,
}

/// Parses an HTML5 string into an arena-allocated `dom::Document`.
#[must_use]
pub fn parse_html(html: &str) -> Document {
    let opts = ParseOpts::default();
    parse_document(HtmlTreeSink::new(), opts).one(html)
}

/// Parses an HTML5 byte buffer into an arena-allocated `dom::Document`.
#[must_use]
pub fn parse_html_bytes(bytes: &[u8]) -> Document {
    let opts = ParseOpts::default();
    parse_document(HtmlTreeSink::new(), opts)
        .from_utf8()
        .one(bytes)
}

/// Parses HTML and additionally extracts the text content of every `<style>`
/// element as author stylesheet sources, in document order.
#[must_use]
pub fn parse_html_with_styles(html: &str) -> (Document, Vec<String>) {
    let doc = parse_html(html);
    let styles = doc
        .get_elements_by_tag_name("style")
        .iter()
        .map(|id| doc.text_content(*id))
        .filter(|css| !css.trim().is_empty())
        .collect();
    (doc, styles)
}

/// Parses an HTML string and extracts the DOM tree along with all `<style>`, `<link rel="stylesheet">`,
/// and `<script>` declarations in document order.
#[must_use]
pub fn parse_html_resources(html: &str) -> ParsedHtmlDocument {
    let doc = parse_html(html);

    let inline_styles = doc
        .get_elements_by_tag_name("style")
        .iter()
        .map(|id| doc.text_content(*id))
        .filter(|css| !css.trim().is_empty())
        .collect();

    let stylesheet_links = doc
        .get_elements_by_tag_name("link")
        .iter()
        .filter_map(|&id| {
            let elem = doc.get_node(id)?.as_element()?;
            let rel = elem.attr("rel")?;
            if rel.eq_ignore_ascii_case("stylesheet") {
                elem.attr("href").map(ToString::to_string)
            } else {
                None
            }
        })
        .collect();

    let mut inline_scripts = Vec::new();
    let mut external_scripts = Vec::new();

    for &id in &doc.get_elements_by_tag_name("script") {
        if let Some(node) = doc.get_node(id)
            && let Some(elem) = node.as_element()
        {
            if let Some(src) = elem.attr("src") {
                external_scripts.push(src.to_string());
            } else {
                let code = doc.text_content(id);
                if !code.trim().is_empty() {
                    inline_scripts.push(code);
                }
            }
        }
    }

    ParsedHtmlDocument {
        document: doc,
        inline_styles,
        stylesheet_links,
        inline_scripts,
        external_scripts,
    }
}
