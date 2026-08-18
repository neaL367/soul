//! WHATWG HTML5 tokenization and tree construction powered by `html5ever` and `dom`.

pub mod parser;
pub mod sink;

pub use parser::{
    ParsedHtmlDocument, parse_html, parse_html_bytes, parse_html_resources, parse_html_with_styles,
};
pub use sink::HtmlTreeSink;
