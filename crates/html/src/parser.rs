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
