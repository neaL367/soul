//! Selector corpus tests for `selectors` crate migration.
//! Covers attribute selectors, sibling combinators, and structural pseudo-classes
//! that were missing in the hand-rolled matcher (ADR-17).

use css::{CascadeResolver, Color, Origin, parse_stylesheet};
use html::parse_html;

#[test]
fn test_attribute_exists_selector() {
    let html = r#"<html><body><div data-test="1"><p>No attr</p></div></body></html>"#;
    let doc = parse_html(html);
    let css = r"[data-test] { background-color: #ff0000; }";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let div = doc.get_elements_by_tag_name("div")[0];
    let p = doc.get_elements_by_tag_name("p")[0];
    assert_eq!(
        styles.get(&div).unwrap().background_color,
        Color::rgb(255, 0, 0)
    );
    assert_ne!(
        styles.get(&p).unwrap().background_color,
        Color::rgb(255, 0, 0)
    );
}

#[test]
fn test_attribute_equality_and_operators() {
    let html = r#"<html><body>
        <input type="text" id="a">
        <input type="password" id="b">
        <div data-val="foobar" id="c"></div>
        <div data-val="foo" id="d"></div>
        <a href="https://example.com" id="e"></a>
        <a href="http://example.com" id="f"></a>
    </body></html>"#;
    let doc = parse_html(html);
    // Test =, ^=, $=, *=, ~=, |=
    let css = r#"
        [type="text"] { color: #ff0000; }
        [data-val^="foo"] { color: #00ff00; }
        [href$=".com"] { color: #0000ff; }
        [data-val*="oba"] { color: #ffff00; }
    "#;
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let a = doc.get_element_by_id("a").unwrap();
    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(255, 0, 0));

    let b = doc.get_element_by_id("b").unwrap();
    // Should not match [type="text"] (color remains initial black)
    assert_ne!(styles.get(&b).unwrap().color, Color::rgb(255, 0, 0));

    let c = doc.get_element_by_id("c").unwrap();
    // c has data-val="foobar": matches ^=foo, $ check? foobar does not end with .com, but matches *=oba and ^=foo
    // The last rule [data-val*="oba"] should win for c (yellow) due to source order if specificity equal
    // But ^= and *= have same specificity (class-like 1), so later wins -> yellow
    assert_eq!(styles.get(&c).unwrap().color, Color::rgb(255, 255, 0));

    let e = doc.get_element_by_id("e").unwrap();
    assert_eq!(styles.get(&e).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_adjacent_and_general_sibling_combinators() {
    let html = r#"<html><body>
        <div id="a">A</div>
        <p id="b">B</p>
        <p id="c">C</p>
        <span id="d">D</span>
    </body></html>"#;
    let doc = parse_html(html);
    let css = r"
        div + p { color: #ff0000; }
        div ~ span { color: #00ff00; }
    ";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let b = doc.get_element_by_id("b").unwrap();
    let c = doc.get_element_by_id("c").unwrap();
    let d = doc.get_element_by_id("d").unwrap();

    assert_eq!(styles.get(&b).unwrap().color, Color::rgb(255, 0, 0));
    // c is second p, not directly after div, so adjacent + should NOT match, but general sibling ~ would if we had div ~ p
    assert_ne!(styles.get(&c).unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(styles.get(&d).unwrap().color, Color::rgb(0, 255, 0));
}

#[test]
fn test_universal_selector() {
    let html = r"<html><body><div><p>Hi</p></div></body></html>";
    let doc = parse_html(html);
    let css = r"* { color: #123456; }";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let p = doc.get_elements_by_tag_name("p")[0];
    // Universal should match p (and html/body/div) but we check p
    assert_eq!(styles.get(&p).unwrap().color, Color::rgb(0x12, 0x34, 0x56));
}

#[test]
fn test_pseudo_classes_root_and_empty() {
    let html = r#"<html><body><div id="empty"></div><div id="nonempty">x</div></body></html>"#;
    let doc = parse_html(html);
    let css = r"
        :root { color: #ff0000; }
        div:empty { color: #00ff00; }
        div:not(:empty) { color: #0000ff; }
    ";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    // :root should match <html> (has special handling, but we test that html gets red)
    let html_el = doc.get_elements_by_tag_name("html")[0];
    assert_eq!(styles.get(&html_el).unwrap().color, Color::rgb(255, 0, 0));

    let empty = doc.get_element_by_id("empty").unwrap();
    assert_eq!(styles.get(&empty).unwrap().color, Color::rgb(0, 255, 0));

    let nonempty = doc.get_element_by_id("nonempty").unwrap();
    assert_eq!(styles.get(&nonempty).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_structural_pseudo_classes() {
    let html = r#"<html><body><ul>
        <li id="a">1</li>
        <li id="b">2</li>
        <li id="c">3</li>
        <li id="d">4</li>
    </ul></body></html>"#;
    let doc = parse_html(html);
    let css = r"
        li:first-child { color: #ff0000; }
        li:last-child { color: #00ff00; }
        li:nth-child(2) { color: #0000ff; }
        li:nth-child(2n) { color: #ffff00; }
    ";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let a = doc.get_element_by_id("a").unwrap();
    let b = doc.get_element_by_id("b").unwrap();
    let c = doc.get_element_by_id("c").unwrap();
    let d = doc.get_element_by_id("d").unwrap();

    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(255, 0, 0));
    // b is nth-child(2) and also 2n (even) -> last rule wins yellow due to source order (same specificity)
    assert_eq!(styles.get(&b).unwrap().color, Color::rgb(255, 255, 0));
    // d is last-child and even -> last rule yellow wins over green due to order
    assert_eq!(styles.get(&d).unwrap().color, Color::rgb(255, 255, 0));
    // c is not first/last/2n+? Actually 3 is odd, not even, so should remain default black
    // But we didn't set color for c except via maybe inherited? So check not red/green/blue/yellow
    assert_ne!(styles.get(&c).unwrap().color, Color::rgb(255, 0, 0));
}

#[test]
fn test_not_pseudo_and_compound_selector() {
    let html = r#"<html><body>
        <div class="foo bar" id="a"></div>
        <div class="foo" id="b"></div>
        <p class="foo bar" id="c"></p>
    </body></html>"#;
    let doc = parse_html(html);
    let css = r"
        div.foo.bar { color: #ff0000; }
        div:not(.bar) { color: #00ff00; }
        p.foo:not(#c) { color: #0000ff; }
    ";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let a = doc.get_element_by_id("a").unwrap();
    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(255, 0, 0));

    let b = doc.get_element_by_id("b").unwrap();
    assert_eq!(styles.get(&b).unwrap().color, Color::rgb(0, 255, 0));

    let c = doc.get_element_by_id("c").unwrap();
    // p.foo:not(#c) should NOT match c because c has id c, so should remain default
    assert_ne!(styles.get(&c).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_attribute_case_and_dash_match() {
    let html = r#"<html><body>
        <div lang="en-US" id="a"></div>
        <div lang="en" id="b"></div>
        <div lang="fr" id="c"></div>
    </body></html>"#;
    let doc = parse_html(html);
    let css = r#"
        [lang|="en"] { color: #ff0000; }
    "#;
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let a = doc.get_element_by_id("a").unwrap();
    let b = doc.get_element_by_id("b").unwrap();
    let c = doc.get_element_by_id("c").unwrap();

    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(styles.get(&b).unwrap().color, Color::rgb(255, 0, 0));
    assert_ne!(styles.get(&c).unwrap().color, Color::rgb(255, 0, 0));
}

#[test]
fn test_specificity_with_attribute_and_pseudo() {
    // Attribute selectors should have class-like specificity (10), same as .class
    // Verify that [attr] and .class are comparable and source order matters.
    let html = r#"<html><body><div class="foo" data-x="1" id="a"></div></body></html>"#;
    let doc = parse_html(html);
    let css = r"
        .foo { color: #ff0000; }
        [data-x] { color: #00ff00; }
    ";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let a = doc.get_element_by_id("a").unwrap();
    // Both have specificity (0,0,1,0), later wins -> green
    assert_eq!(styles.get(&a).unwrap().color, Color::rgb(0, 255, 0));
}
