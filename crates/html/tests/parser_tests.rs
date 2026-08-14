//! Integration tests for HTML5 parsing into `dom::Document`.

use html::{parse_html, parse_html_bytes};

#[test]
fn test_parse_full_html5_document() {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <title>Soul Test Page</title>
</head>
<body>
    <main id="app">
        <h1 class="title">Soul Browser Engine</h1>
        <p class="desc">A fast browser engine written in <b>Rust</b>.</p>
    </main>
</body>
</html>"#;

    let doc = parse_html(html);

    assert!(doc.doctype_id().is_some());

    let app_elem = doc.get_element_by_id("app").expect("app element not found");
    assert_eq!(
        doc.get_node(app_elem)
            .unwrap()
            .as_element()
            .unwrap()
            .tag_name,
        "main"
    );

    let titles = doc.get_elements_by_tag_name("h1");
    assert_eq!(titles.len(), 1);
    assert_eq!(doc.text_content(titles[0]), "Soul Browser Engine");

    let p_elems = doc.get_elements_by_class_name("desc");
    assert_eq!(p_elems.len(), 1);
    assert_eq!(
        doc.text_content(p_elems[0]),
        "A fast browser engine written in Rust."
    );
}

#[test]
fn test_parse_malformed_html_auto_correction() {
    let malformed = "<p>First paragraph<p>Second paragraph<div>Inside div</div>";
    let doc = parse_html(malformed);

    let paragraphs = doc.get_elements_by_tag_name("p");
    assert_eq!(paragraphs.len(), 2);
    assert_eq!(doc.text_content(paragraphs[0]), "First paragraph");
    assert_eq!(doc.text_content(paragraphs[1]), "Second paragraph");

    let divs = doc.get_elements_by_tag_name("div");
    assert_eq!(divs.len(), 1);
    assert_eq!(doc.text_content(divs[0]), "Inside div");
}

#[test]
fn test_parse_table_auto_tbody_generation() {
    let table_html = "<table><tr><td>Cell A</td><td>Cell B</td></tr></table>";
    let doc = parse_html(table_html);

    // WHATWG tree builder automatically inserts <tbody> inside <table>
    let tbodies = doc.get_elements_by_tag_name("tbody");
    assert_eq!(tbodies.len(), 1);

    let cells = doc.get_elements_by_tag_name("td");
    assert_eq!(cells.len(), 2);
    assert_eq!(doc.text_content(cells[0]), "Cell A");
    assert_eq!(doc.text_content(cells[1]), "Cell B");
}

#[test]
fn test_parse_html_bytes() {
    let bytes = b"<html><body><div id=\"root\">Rendered from bytes</div></body></html>";
    let doc = parse_html_bytes(bytes);

    let root_node = doc.get_element_by_id("root").expect("root node missing");
    assert_eq!(doc.text_content(root_node), "Rendered from bytes");
}
