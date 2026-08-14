//! WHATWG HTML5 tokenization and tree construction powered by `html5ever` and `dom`.

pub mod parser;
pub mod sink;

pub use parser::{parse_html, parse_html_bytes};
pub use sink::HtmlTreeSink;
