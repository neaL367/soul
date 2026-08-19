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
    // Unitless line-height factor: 16px inherited font-size * 1.5 = 24px
    assert!((card_style.line_height.unwrap() - 24.0).abs() < f32::EPSILON);
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

#[test]
fn test_font_family_and_flex_properties_apply() {
    let html = r#"<html><body><div id="nav">Nav</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #nav {
            font-family: 'Open Sans', Helvetica, sans-serif;
            letter-spacing: 1px;
            word-spacing: 2px;
            flex-direction: column;
            flex-wrap: wrap;
            justify-content: space-between;
            align-items: center;
            align-self: flex-end;
            flex-grow: 2;
            flex-shrink: 0.5;
            flex-basis: 50%;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let nav_id = doc.get_element_by_id("nav").unwrap();
    let nav = styles.get(&nav_id).unwrap();

    assert_eq!(nav.font_family, "Open Sans");
    assert!((nav.letter_spacing - 1.0).abs() < f32::EPSILON);
    assert!((nav.word_spacing - 2.0).abs() < f32::EPSILON);
    assert_eq!(nav.flex_direction, css::FlexDirection::Column);
    assert_eq!(nav.flex_wrap, css::FlexWrap::Wrap);
    assert_eq!(nav.justify_content, css::JustifyContent::SpaceBetween);
    assert_eq!(nav.align_items, css::AlignItems::Center);
    assert_eq!(nav.align_self, css::AlignSelf::FlexEnd);
    assert!((nav.flex_grow - 2.0).abs() < f32::EPSILON);
    assert!((nav.flex_shrink - 0.5).abs() < f32::EPSILON);
    assert_eq!(nav.flex_basis, css::Length::Percent(50.0));
}

#[test]
fn test_unitless_lengths_only_zero_is_valid() {
    let html = r#"<html><body><div id="w">W</div><div id="z">Z</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #w { width: 10; margin: 5; }
        #z { width: 0; margin: 0; line-height: 0; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let w_id = doc.get_element_by_id("w").unwrap();
    let z_id = doc.get_element_by_id("z").unwrap();

    // Unitless non-zero lengths are invalid per CSS 2.1 §4.3.2 and must be ignored.
    assert_eq!(styles.get(&w_id).unwrap().width, css::Length::Auto);
    assert!((styles.get(&w_id).unwrap().margin_top - 0.0).abs() < f32::EPSILON);
    // Unitless zero is always valid.
    assert_eq!(styles.get(&z_id).unwrap().width, css::Length::Px(0.0));
    assert!((styles.get(&z_id).unwrap().margin_left - 0.0).abs() < f32::EPSILON);
    assert_eq!(styles.get(&z_id).unwrap().line_height, Some(0.0));
}

#[test]
fn test_multibyte_hex_color_does_not_panic() {
    let html = r#"<html><body><div id="a">A</div><div id="b">B</div></body></html>"#;
    let doc = parse_html(html);

    // Non-ASCII hex digits must be rejected without panicking on byte slicing.
    let css = r"
        #a { color: #🎨; }
        #b { color: #🎨🎨🎨🎨; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let a_id = doc.get_element_by_id("a").unwrap();
    let b_id = doc.get_element_by_id("b").unwrap();

    assert_eq!(styles.get(&a_id).unwrap().color, Color::BLACK);
    assert_eq!(styles.get(&b_id).unwrap().color, Color::BLACK);
}

#[test]
fn test_negative_hue_wraps_per_css_color_4() {
    let html = r#"<html><body><div id="c">C</div></body></html>"#;
    let doc = parse_html(html);

    // -120deg is equivalent to 240deg: pure blue.
    let css = "#c { color: hsl(-120, 100%, 50%); }";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let c_id = doc.get_element_by_id("c").unwrap();
    assert_eq!(styles.get(&c_id).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_non_finite_numbers_rejected() {
    let html = r#"<html><body><div id="d">D</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #d {
            opacity: NaN;
            flex-grow: NaN;
            width: infpx;
            line-height: inf;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let d_id = doc.get_element_by_id("d").unwrap();
    let d = styles.get(&d_id).unwrap();

    assert!((d.opacity - 1.0).abs() < f32::EPSILON);
    assert!((d.flex_grow - 0.0).abs() < f32::EPSILON);
    assert_eq!(d.width, css::Length::Auto);
    assert_eq!(d.line_height, None);
}

#[test]
fn test_cornflowerblue_named_color() {
    let html = r#"<html><body><div id="e">E</div></body></html>"#;
    let doc = parse_html(html);

    let css = "#e { color: cornflowerblue; }";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let e_id = doc.get_element_by_id("e").unwrap();
    assert_eq!(styles.get(&e_id).unwrap().color, Color::rgb(100, 149, 237));
}

#[test]
fn test_important_spacing_and_case_variants() {
    let html = r#"<html><body><div id="main" class="box">Content</div></body></html>"#;
    let doc = parse_html(html);

    // No whitespace before `!`, uppercase marker, and whitespace after `!`
    // must all be recognized as `!important`.
    let css = r"
        #main { color: #ff0000!important; }
        .box { color: #00ff00; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_element_by_id("main").unwrap();
    assert_eq!(styles.get(&div_id).unwrap().color, Color::rgb(255, 0, 0));

    let css2 = r"
        #main { color: #0000ff ! IMPORTANT; }
        .box { color: #00ff00; }
    ";
    let author_sheet2 = parse_stylesheet(css2, Origin::Author);
    let resolver2 = CascadeResolver::new(&doc, &[&author_sheet2]);
    let styles2 = resolver2.resolve_all();

    assert_eq!(styles2.get(&div_id).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_child_combinator_without_whitespace() {
    let html = r#"<html><body><section><p class="kid">Kid</p></section></body></html>"#;
    let doc = parse_html(html);

    // `section>p` (no spaces) must parse as child combinator, not a literal tag.
    let css = "section>p { color: #ff0000; }";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let kid = doc.get_elements_by_class_name("kid")[0];
    assert_eq!(styles.get(&kid).unwrap().color, Color::rgb(255, 0, 0));
}

#[test]
fn test_strings_and_urls_do_not_break_declarations() {
    let html = r#"<html><body><div id="f">F</div></body></html>"#;
    let doc = parse_html(html);

    // Semicolons and braces inside quoted strings and url(...) values must not
    // terminate the declaration or the rule.
    let css = r#"
        #f {
            content: "a;b"; /* semicolon inside string */
            background: url(data:image/svg+xml;utf8,<svg>{}</svg>); /* brace + semicolon inside url */
            color: #0000ff;
        }
    "#;
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let f_id = doc.get_element_by_id("f").unwrap();
    assert_eq!(styles.get(&f_id).unwrap().color, Color::rgb(0, 0, 255));
}

#[test]
fn test_at_rule_body_is_not_misparsed() {
    let html = r#"<html><body><p class="outside">Text</p></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        @media screen {
            p { color: #ff0000; }
        }
        p { color: #00ff00; }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    // The rule inside the unsupported @media block must not leak into the sheet.
    let p = doc.get_elements_by_class_name("outside")[0];
    assert_eq!(styles.get(&p).unwrap().color, Color::rgb(0, 255, 0));
}
