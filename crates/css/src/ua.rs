//! Standard HTML5 default User-Agent stylesheet.

use crate::parser::parse_stylesheet;
use crate::rule::{Origin, StyleSheet};
use std::sync::OnceLock;

/// Returns the singleton default User-Agent stylesheet.
#[must_use]
pub fn user_agent_stylesheet() -> &'static StyleSheet {
    static UA_SHEET: OnceLock<StyleSheet> = OnceLock::new();
    UA_SHEET.get_or_init(|| {
        let css = r"
html, body, div, p, main, section, article, header, footer,
nav, aside, h1, h2, h3, h4, h5, h6, ul, ol, li, form, table {
    display: block;
}

head, script, style, title, meta, link {
    display: none;
}

body {
    margin: 8px;
    font-size: 16px;
    font-family: sans-serif;
    color: #000000;
    background-color: transparent;
}

h1 {
    font-size: 32px;
    font-weight: bold;
    margin: 21px 0;
}

h2 {
    font-size: 24px;
    font-weight: bold;
    margin: 19px 0;
}

h3 {
    font-size: 18px;
    font-weight: bold;
    margin: 18px 0;
}

p {
    margin: 16px 0;
}

b, strong {
    font-weight: bold;
}
";
        parse_stylesheet(css, Origin::UserAgent)
    })
}
