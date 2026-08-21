//! Integration tests for CSS transforms, gradients, and transitions.

use css::{
    CascadeResolver, Color, ColorStop, Gradient, Origin, TimingFunction, Transform2D, TransformOp,
    parse_stylesheet,
};
use html::parse_html;

#[test]
fn test_transform_matrix_math() {
    let t = Transform2D::translate(10.0, 20.0);
    let (x, y) = t.transform_point(5.0, 5.0);
    assert_eq!((x, y), (15.0, 25.0));

    let s = Transform2D::scale(2.0, 3.0);
    let (x, y) = s.transform_point(5.0, 5.0);
    assert_eq!((x, y), (10.0, 15.0));

    let combined = t.multiply(&s);
    let (x, y) = combined.transform_point(5.0, 5.0);
    assert_eq!((x, y), (20.0, 35.0));
}

#[test]
fn test_transform_css_parsing() {
    let html = "<html><body><div id=\"target\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = "#target { transform: translate(15px, 25px) rotate(90deg) scale(2); }";
    let sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let elem = doc.get_elements_by_tag_name("div")[0];
    let style = styles.get(&elem).expect("style resolved");
    assert_eq!(style.transform.len(), 3);
    assert_eq!(style.transform[0], TransformOp::Translate(15.0, 25.0));
    match style.transform[1] {
        TransformOp::Rotate(rad) => {
            let deg = rad.to_degrees();
            assert!((deg - 90.0).abs() < 1e-3);
        }
        TransformOp::Translate(..)
        | TransformOp::Scale(..)
        | TransformOp::Skew(..)
        | TransformOp::Matrix(..) => panic!("expected rotate"),
    }
    assert_eq!(style.transform[2], TransformOp::Scale(2.0, 2.0));
}

#[test]
fn test_transform_origin_parsing() {
    let html = "<html><body><div id=\"target\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = "#target { transform-origin: top left; }";
    let sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let elem = doc.get_elements_by_tag_name("div")[0];
    let style = styles.get(&elem).expect("style resolved");
    assert_eq!(style.transform_origin, (0.0, 0.0));
}

#[test]
fn test_linear_gradient_parsing() {
    let html = "<html><body><div id=\"target\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = "#target { background-image: linear-gradient(to right, red 0%, blue 100%); }";
    let sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let elem = doc.get_elements_by_tag_name("div")[0];
    let style = styles.get(&elem).expect("style resolved");
    let grad = style.background_gradient.as_ref().expect("gradient parsed");
    match grad {
        Gradient::Linear { angle_deg, stops } => {
            assert!((angle_deg - 90.0).abs() < 1e-3);
            assert_eq!(stops.len(), 2);
            assert_eq!(
                stops[0],
                ColorStop {
                    position: 0.0,
                    color: Color::rgb(255, 0, 0)
                }
            );
            assert_eq!(
                stops[1],
                ColorStop {
                    position: 1.0,
                    color: Color::rgb(0, 0, 255)
                }
            );
        }
        Gradient::Radial { .. } => panic!("expected linear gradient"),
    }
}

#[test]
fn test_radial_gradient_parsing() {
    let html = "<html><body><div id=\"target\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = "#target { background: radial-gradient(white, black); }";
    let sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let elem = doc.get_elements_by_tag_name("div")[0];
    let style = styles.get(&elem).expect("style resolved");
    let grad = style.background_gradient.as_ref().expect("gradient parsed");
    match grad {
        Gradient::Radial { stops, .. } => {
            assert_eq!(stops.len(), 2);
            assert_eq!(stops[0].color, Color::WHITE);
            assert_eq!(stops[1].color, Color::BLACK);
        }
        Gradient::Linear { .. } => panic!("expected radial gradient"),
    }
}

#[test]
#[allow(clippy::float_cmp)]
fn test_transition_parsing() {
    let html = "<html><body><div id=\"target\">Content</div></body></html>";
    let doc = parse_html(html);

    let css = "#target { transition: opacity 300ms ease-in-out 50ms, transform 0.5s linear; }";
    let sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let elem = doc.get_elements_by_tag_name("div")[0];
    let style = styles.get(&elem).expect("style resolved");
    assert_eq!(style.transition_properties.len(), 2);

    assert_eq!(style.transition_properties[0].property, "opacity");
    assert_eq!(style.transition_properties[0].duration_ms, 300.0);
    assert_eq!(
        style.transition_properties[0].timing_function,
        TimingFunction::EaseInOut
    );
    assert_eq!(style.transition_properties[0].delay_ms, 50.0);

    assert_eq!(style.transition_properties[1].property, "transform");
    assert_eq!(style.transition_properties[1].duration_ms, 500.0);
    assert_eq!(
        style.transition_properties[1].timing_function,
        TimingFunction::Linear
    );
    assert_eq!(style.transition_properties[1].delay_ms, 0.0);
}
