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

#[test]
fn test_box_sizing_and_shorthand_expansions() {
    let html = r#"<html><body><div id="card">Card</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #card {
            box-sizing: border-box;
            margin: 10px 20px;
            padding: 5px 10px 15px 20px;
            border: 2px solid red;
            line-height: 1.5;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let card_id = doc.get_element_by_id("card").unwrap();
    let card_style = styles.get(&card_id).unwrap();

    assert_eq!(card_style.box_sizing, css::BoxSizing::BorderBox);
    assert!((card_style.margin_top - 10.0).abs() < f32::EPSILON);
    assert!((card_style.margin_right - 20.0).abs() < f32::EPSILON);
    assert!((card_style.margin_bottom - 10.0).abs() < f32::EPSILON);
    assert!((card_style.margin_left - 20.0).abs() < f32::EPSILON);

    assert!((card_style.padding_top - 5.0).abs() < f32::EPSILON);
    assert!((card_style.padding_right - 10.0).abs() < f32::EPSILON);
    assert!((card_style.padding_bottom - 15.0).abs() < f32::EPSILON);
    assert!((card_style.padding_left - 20.0).abs() < f32::EPSILON);

    assert!((card_style.border_top_width - 2.0).abs() < f32::EPSILON);
    assert_eq!(card_style.border_top_color, Color::rgb(255, 0, 0));
    assert!(card_style.line_height.is_some());
}

#[test]
fn test_hsl_color_and_border_radius() {
    let html = r#"<html><body><div id="badge">Badge</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #badge {
            background-color: hsl(120, 100%, 50%);
            border-radius: 8px;
            font-style: italic;
            text-decoration: underline;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let badge_id = doc.get_element_by_id("badge").unwrap();
    let badge_style = styles.get(&badge_id).unwrap();

    // HSL(120, 100%, 50%) is pure green (0, 255, 0)
    assert_eq!(badge_style.background_color, Color::rgb(0, 255, 0));
    assert!((badge_style.border_radius_top_left - 8.0).abs() < f32::EPSILON);
    assert_eq!(badge_style.font_style, css::FontStyle::Italic);
    assert_eq!(badge_style.text_decoration, css::TextDecoration::Underline);
}
