//! Integration tests for CSS Custom Properties (`var()`), theming (`prefers-color-scheme`), and `box-shadow`.

use css::{CascadeResolver, Color, ColorScheme, Origin, parse_stylesheet};
use html::parse_html;

#[test]
fn test_css_custom_property_declaration_and_var_substitution() {
    let html = r#"<html><body><div id="target">Text</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            --brand-color: #ff0000;
            color: var(--brand-color);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();
    assert_eq!(style.color, Color::rgb(255, 0, 0));
    assert_eq!(
        style
            .custom_properties
            .get("--brand-color")
            .map(String::as_str),
        Some("#ff0000")
    );
}

#[test]
fn test_css_custom_property_fallback() {
    let html = r#"<html><body><div id="target">Text</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            color: var(--non-existent, #008000);
            font-size: var(--missing-size, 24px);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();
    assert_eq!(style.color, Color::rgb(0, 128, 0));
    assert!((style.font_size - 24.0).abs() < f32::EPSILON);
}

#[test]
fn test_css_custom_property_inheritance() {
    let html = r#"<html><body><div id="parent"><p id="child">Nested</p></div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #parent {
            --theme-bg: #0000ff;
            --box-pad: 15px;
        }
        #child {
            background-color: var(--theme-bg);
            padding-top: var(--box-pad);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let child = doc.get_element_by_id("child").unwrap();
    let style = styles.get(&child).unwrap();
    assert_eq!(style.background_color, Color::rgb(0, 0, 255));
    assert!((style.padding_top - 15.0).abs() < f32::EPSILON);
    assert_eq!(
        style
            .custom_properties
            .get("--theme-bg")
            .map(String::as_str),
        Some("#0000ff")
    );
}

#[test]
fn test_css_custom_property_nested_var() {
    let html = r#"<html><body><div id="target">Nested Var</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            --base: #ffff00;
            --alias: var(--base);
            color: var(--alias);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();
    assert_eq!(style.color, Color::rgb(255, 255, 0));
}

#[test]
fn test_prefers_color_scheme_dark_matches_when_dark_requested() {
    let html = r#"<html><body><div id="target">Dark Test</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            color: #000000;
        }
        @media (prefers-color-scheme: dark) {
            #target {
                color: #ffffff;
            }
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    // 1. In default / light mode: #target is black (#000000)
    let light_resolver =
        CascadeResolver::new_with_scheme(&doc, &[&author_sheet], ColorScheme::Light);
    let light_styles = light_resolver.resolve_all();
    let target = doc.get_element_by_id("target").unwrap();
    assert_eq!(
        light_styles.get(&target).unwrap().color,
        Color::rgb(0, 0, 0)
    );

    // 2. In dark mode: #target is white (#ffffff)
    let dark_resolver = CascadeResolver::new_with_scheme(&doc, &[&author_sheet], ColorScheme::Dark);
    let dark_styles = dark_resolver.resolve_all();
    assert_eq!(
        dark_styles.get(&target).unwrap().color,
        Color::rgb(255, 255, 255)
    );
}

#[test]
fn test_dark_theme_variable_override() {
    let html = r#"<html><body><div id="target">Theme Vars</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        :root {
            --bg-color: #ffffff;
            --text-color: #111111;
        }
        @media (prefers-color-scheme: dark) {
            :root {
                --bg-color: #1a1a1a;
                --text-color: #eeeeee;
            }
        }
        #target {
            background-color: var(--bg-color);
            color: var(--text-color);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let dark_resolver = CascadeResolver::new_with_scheme(&doc, &[&author_sheet], ColorScheme::Dark);
    let styles = dark_resolver.resolve_all();
    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();

    assert_eq!(style.background_color, Color::rgb(0x1a, 0x1a, 0x1a));
    assert_eq!(style.color, Color::rgb(0xee, 0xee, 0xee));
}

#[test]
fn test_box_shadow_single_layer_parsing() {
    let html = r#"<html><body><div id="target">Shadow</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            box-shadow: 2px 4px 6px 1px rgba(0, 0, 0, 0.5);
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();

    assert_eq!(style.box_shadow.len(), 1);
    let s = &style.box_shadow[0];
    assert!((s.offset_x - 2.0).abs() < f32::EPSILON);
    assert!((s.offset_y - 4.0).abs() < f32::EPSILON);
    assert!((s.blur_radius - 6.0).abs() < f32::EPSILON);
    assert!((s.spread_radius - 1.0).abs() < f32::EPSILON);
    assert_eq!(s.color, Color::parse("rgba(0, 0, 0, 0.5)").unwrap());
    assert!(!s.inset);
}

#[test]
fn test_box_shadow_multiple_layers_and_inset() {
    let html = r#"<html><body><div id="target">Multi Shadow</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #target {
            box-shadow: inset 0 0 10px #ff0000, 5px 5px #0000ff;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target = doc.get_element_by_id("target").unwrap();
    let style = styles.get(&target).unwrap();

    assert_eq!(style.box_shadow.len(), 2);
    let s1 = &style.box_shadow[0];
    assert!(s1.inset);
    assert!((s1.blur_radius - 10.0).abs() < f32::EPSILON);
    assert_eq!(s1.color, Color::rgb(255, 0, 0));

    let s2 = &style.box_shadow[1];
    assert!(!s2.inset);
    assert!((s2.offset_x - 5.0).abs() < f32::EPSILON);
    assert!((s2.offset_y - 5.0).abs() < f32::EPSILON);
    assert_eq!(s2.color, Color::rgb(0, 0, 255));
}
