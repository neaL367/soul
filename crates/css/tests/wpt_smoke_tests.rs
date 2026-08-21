//! WPT smoke harness — informational, not MVP gate (per endorsed roadmap).
//! Subset of `web-platform-tests/css/selectors` smoke cases for continuous observability.

use css::{CascadeResolver, Color, Origin, parse_stylesheet};
use html::parse_html;

fn style_for_selector(html: &str, css: &str, target_tag: &str) -> Color {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let el = doc.get_elements_by_tag_name(target_tag)[0];
    styles.get(&el).unwrap().color
}

#[test]
fn wpt_attribute_selector_smoke() {
    // From WPT css/selectors/attribute-selectors
    let html =
        r#"<html><body><a href="https://example.com" id="x"></a><a id="y"></a></body></html>"#;
    let css = r"a[href] { color: #ff0000; }";
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let x = doc.get_element_by_id("x").unwrap();
    let y = doc.get_element_by_id("y").unwrap();
    assert_eq!(styles.get(&x).unwrap().color, Color::rgb(255, 0, 0));
    assert_ne!(styles.get(&y).unwrap().color, Color::rgb(255, 0, 0));
}

#[test]
fn wpt_pseudo_class_first_child_smoke() {
    // WPT css/selectors/pseudo-classes/:first-child
    let html = r#"<html><body><ul><li id="a">1</li><li id="b">2</li></ul></body></html>"#;
    let css = r"li:first-child { color: #ff0000; }";
    assert_eq!(style_for_selector(html, css, "li"), Color::rgb(255, 0, 0));
    // Verify second li is not first-child via direct resolver check
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let b = doc.get_element_by_id("b").unwrap();
    // b should remain default (black) since :first-child only matches a
    assert_eq!(styles.get(&b).unwrap().color, Color::BLACK);
}

#[test]
fn wpt_combinator_smoke() {
    // WPT combinators: child vs descendant
    let html =
        r#"<html><body><div><p id="a">A</p><section><p id="b">B</p></section></div></body></html>"#;
    let css = r"div > p { color: #ff0000; }";
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let a = doc.get_element_by_id("a").unwrap();
    let b = doc.get_element_by_id("b").unwrap();
    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(255, 0, 0));
    assert_ne!(styles.get(&b).unwrap().color, Color::rgb(255, 0, 0));
}

#[test]
fn wpt_is_where_pseudo_smoke_informational() {
    // WPT :is() and :where() — currently unsupported (forgiving fallback drops invalid selectors)
    // This test documents current informational status: :is() should be parsed when we enable it,
    // for now it is gracefully ignored and does not panic.
    let html = r#"<html><body><div class="a" id="x"></div></body></html>"#;
    let css = r":is(.a, .b) { color: #ff0000; } div { color: #00ff00; }";
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    // Sheet should still parse (second rule valid) and div should get green, not red, since :is is dropped
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let x = doc.get_element_by_id("x").unwrap();
    // Informational: if :is is unsupported, x will be green from div rule
    assert_eq!(styles.get(&x).unwrap().color, Color::rgb(0, 255, 0));
}

#[test]
fn wpt_not_pseudo_smoke() {
    let html = r#"<html><body><p class="a" id="x"></p><p class="b" id="y"></p></body></html>"#;
    let css = r"p:not(.b) { color: #ff0000; }";
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let x = doc.get_element_by_id("x").unwrap();
    let y = doc.get_element_by_id("y").unwrap();
    assert_eq!(styles.get(&x).unwrap().color, Color::rgb(255, 0, 0));
    assert_ne!(styles.get(&y).unwrap().color, Color::rgb(255, 0, 0));
}
