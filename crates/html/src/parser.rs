//! HTML5 document parsing entry points using `html5ever`.

use crate::sink::HtmlTreeSink;
use dom::Document;
use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, parse_document};

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
