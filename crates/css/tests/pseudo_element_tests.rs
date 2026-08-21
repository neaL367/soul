//! Integration tests for CSS pseudo-elements and Custom Properties.

use css::cascade::CascadeResolver;
use css::parser::parse_stylesheet;
use css::rule::Origin;
use html::parse_html;

#[test]
fn test_pseudo_element_selector_parsing() {
    let css = r"
        p::before {
            content: '>> ';
            color: #ff0000;
        }
        button::after {
            content: ' [OK]';
        }
        input::placeholder {
            color: #888888;
        }
    ";

    let sheet = parse_stylesheet(css, Origin::Author);
    assert_eq!(sheet.rules.len(), 3);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_custom_properties_var_resolution() {
    let css = r"
        html {
            --main-bg: #123456;
            --main-pad: 24px;
        }
        div {
            background-color: var(--main-bg);
            padding-top: var(--main-pad);
        }
    ";

    let sheet = parse_stylesheet(css, Origin::Author);
    let doc = parse_html("<html><body><div>Hello</div></body></html>");

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_elements_by_tag_name("div")[0];
    let div_style = styles.get(&div_id).expect("div style");
    assert_eq!(
        div_style.background_color,
        css::Color::rgba(0x12, 0x34, 0x56, 0xFF)
    );
    assert_eq!(div_style.padding_top, 24.0);
}
