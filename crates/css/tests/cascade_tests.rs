//! Integration tests for CSS parsing, selector matching, specificity, and cascade resolution.

use css::{CascadeResolver, Color, Display, Origin, parse_stylesheet};
use html::parse_html;

#[test]
fn test_user_agent_stylesheet_defaults() {
    let html = "<html><head><script></script></head><body><div><p>Hello</p><span>World</span></div></body></html>";
    let doc = parse_html(html);

    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_elements_by_tag_name("div")[0];
    let p_id = doc.get_elements_by_tag_name("p")[0];
    let span_id = doc.get_elements_by_tag_name("span")[0];
    let script_id = doc.get_elements_by_tag_name("script")[0];

    assert_eq!(styles.get(&div_id).unwrap().display, Display::Block);
    assert_eq!(styles.get(&p_id).unwrap().display, Display::Block);
    assert_eq!(styles.get(&span_id).unwrap().display, Display::Inline);
    assert_eq!(styles.get(&script_id).unwrap().display, Display::None);
}

#[test]
fn test_specificity_id_beats_class_and_tag() {
    let html = "<html><body><div id=\"main\" class=\"box\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = r"
        div { color: #0000ff; }
        .box { color: #008000; }
        #main { color: #ff0000; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_elements_by_tag_name("div")[0];
    let div_style = styles.get(&div_id).unwrap();

    assert_eq!(div_style.color, Color::rgb(255, 0, 0));
}

#[test]
fn test_important_declaration_overrides_specificity() {
    let html = "<html><body><div id=\"main\" class=\"important-box\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = r"
        #main { color: #ff0000; }
        .important-box { color: #ffff00 !important; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_elements_by_tag_name("div")[0];
    let div_style = styles.get(&div_id).unwrap();

    assert_eq!(div_style.color, Color::rgb(255, 255, 0));
}

#[test]
fn test_top_down_property_inheritance() {
    let html = "<html><body><div id=\"container\"><p id=\"text\"><span>Inner</span></p></div></body></html>";
    let doc = parse_html(html);

    let css = r"
        body {
            color: #ff0000;
            font-size: 20px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_element_by_id("container").unwrap();
    let p_id = doc.get_element_by_id("text").unwrap();
    let span_id = doc.get_elements_by_tag_name("span")[0];

    assert_eq!(styles.get(&div_id).unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(styles.get(&p_id).unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(styles.get(&span_id).unwrap().color, Color::rgb(255, 0, 0));

    assert!((styles.get(&span_id).unwrap().font_size - 20.0).abs() < f32::EPSILON);
}

#[test]
fn test_combinators_child_and_descendant() {
    let html = r#"<html><body>
        <main id="app">
            <section><p class="direct">Direct</p></section>
            <div><section><span><p class="deep">Deep</p></span></section></div>
        </main>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        section > p { color: #ff0000; }
        #app p { font-size: 24px; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let direct_p = doc.get_elements_by_class_name("direct")[0];
    let deep_p = doc.get_elements_by_class_name("deep")[0];

    // Direct child of section matches `section > p`
    assert_eq!(styles.get(&direct_p).unwrap().color, Color::rgb(255, 0, 0));
    // Deep descendant matches `#app p`
    assert!((styles.get(&deep_p).unwrap().font_size - 24.0).abs() < f32::EPSILON);
}
