//! Integration tests for CSS box model geometry, box tree generation, and normal flow block layout.

use css::{CascadeResolver, Origin, parse_stylesheet};
use html::parse_html;
use layout::{BoxType, Dimensions, EdgeSizes, Rect, build_box_tree, layout_block};

#[test]
fn test_geometry_box_model_math() {
    let dims = Dimensions {
        content: Rect::new(10.0, 20.0, 100.0, 50.0),
        padding: EdgeSizes::new(5.0, 10.0, 5.0, 10.0),
        border: EdgeSizes::new(2.0, 2.0, 2.0, 2.0),
        margin: EdgeSizes::new(15.0, 20.0, 15.0, 20.0),
    };

    let p_box = dims.padding_box();
    assert_eq!(p_box, Rect::new(0.0, 15.0, 120.0, 60.0));

    let b_box = dims.border_box();
    assert_eq!(b_box, Rect::new(-2.0, 13.0, 124.0, 64.0));

    let m_box = dims.margin_box();
    assert_eq!(m_box, Rect::new(-22.0, -2.0, 164.0, 94.0));
}

#[test]
fn test_box_tree_anonymous_block_creation() {
    let html = "<div><span>Inline 1</span><p>Block</p><span>Inline 2</span></div>";
    let doc = parse_html(html);

    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();

    let div_id = doc.get_elements_by_tag_name("div")[0];
    let box_tree = build_box_tree(&doc, div_id, &styles).expect("box tree failed");

    assert!(box_tree.is_block());
    // Should have 3 children: AnonymousBlock, BlockNode(p), AnonymousBlock
    assert_eq!(box_tree.children.len(), 3);
    assert_eq!(box_tree.children[0].box_type, BoxType::AnonymousBlock);
    assert!(matches!(
        box_tree.children[1].box_type,
        BoxType::BlockNode(_)
    ));
    assert_eq!(box_tree.children[2].box_type, BoxType::AnonymousBlock);
}

#[test]
fn test_box_tree_display_none_pruning() {
    let html = "<html><head><style>div { display: block; }</style></head><body><div>Visible</div><script>Hidden</script></body></html>";
    let doc = parse_html(html);

    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();

    let body_id = doc.get_elements_by_tag_name("body")[0];
    let box_tree = build_box_tree(&doc, body_id, &styles).expect("box tree failed");

    // Script and style tags have display: none in UA stylesheet and should not generate boxes
    let descendants_count = count_boxes(&box_tree);
    // Body -> Div -> Text
    assert_eq!(descendants_count, 3);
}

fn count_boxes(b: &layout::LayoutBox) -> usize {
    1 + b.children.iter().map(count_boxes).sum::<usize>()
}

#[test]
fn test_nested_block_layout_geometry() {
    let html = r#"<html><body>
        <div id="container">
            <div id="child1">Box 1</div>
            <div id="child2">Box 2</div>
        </div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #container {
            margin: 0px;
            padding: 10px;
            border: 0px;
        }
        #child1 {
            margin-top: 10px;
            margin-right: 0px;
            margin-bottom: 10px;
            margin-left: 0px;
            padding: 5px;
            height: 100px;
        }
        #child2 {
            margin: 20px 0px;
            padding: 0px;
            height: 50px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let container_id = doc.get_element_by_id("container").unwrap();
    let mut root_box = build_box_tree(&doc, container_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };

    layout_block(&mut root_box, &viewport);

    // Container content width = 800 - 20 (padding) = 780
    assert!((root_box.dimensions.content.width - 780.0).abs() < f32::EPSILON);
    assert!((root_box.dimensions.content.x - 10.0).abs() < f32::EPSILON);
    assert!((root_box.dimensions.content.y - 10.0).abs() < f32::EPSILON);

    // Child 1:
    let child1 = &root_box.children[0];
    // x = container.x (10) + child1.padding.left (5) = 15
    assert!((child1.dimensions.content.x - 15.0).abs() < f32::EPSILON);
    // y = container.y (10) + margin.top (10) + padding.top (5) = 25
    assert!((child1.dimensions.content.y - 25.0).abs() < f32::EPSILON);
    assert!((child1.dimensions.content.height - 100.0).abs() < f32::EPSILON);

    // Child 2 (with vertical margin collapsing max(10, 20) = 20):
    let child2 = &root_box.children[1];
    assert!((child2.dimensions.content.x - 10.0).abs() < f32::EPSILON);
    // y = container.y (10) + child1.margin.top (10) + child1.border_box (110) + collapsed_margin (20) = 150
    assert!((child2.dimensions.content.y - 150.0).abs() < f32::EPSILON);
    assert!((child2.dimensions.content.height - 50.0).abs() < f32::EPSILON);
}

#[test]
fn test_box_sizing_border_box_geometry() {
    let html = r#"<html><body>
        <div id="content_box">Box 1</div>
        <div id="border_box">Box 2</div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #content_box {
            box-sizing: content-box;
            width: 200px;
            height: 100px;
            padding: 10px;
            border: 5px;
        }
        #border_box {
            box-sizing: border-box;
            width: 200px;
            height: 100px;
            padding: 10px;
            border: 5px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let body_id = doc.get_elements_by_tag_name("body")[0];
    let mut root_box = build_box_tree(&doc, body_id, &styles).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut root_box, &viewport);

    let content_box = &root_box.children[0];
    let border_box = &root_box.children[1];

    // content-box: content.width = 200, border_box.width = 200 + 20 + 10 = 230
    assert!((content_box.dimensions.content.width - 200.0).abs() < f32::EPSILON);
    assert!((content_box.dimensions.border_box().width - 230.0).abs() < f32::EPSILON);

    // border-box: border_box.width = 200, content.width = 200 - 20 - 10 = 170
    assert!((border_box.dimensions.content.width - 170.0).abs() < f32::EPSILON);
    assert!((border_box.dimensions.border_box().width - 200.0).abs() < f32::EPSILON);

    // content-box height = 100, border-box height = 100 - 20 - 10 = 70
    assert!((content_box.dimensions.content.height - 100.0).abs() < f32::EPSILON);
    assert!((border_box.dimensions.content.height - 70.0).abs() < f32::EPSILON);
}
