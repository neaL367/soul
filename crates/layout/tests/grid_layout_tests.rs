//! Integration tests for CSS Grid layout (M8).

use css::{ComputedStyle, Display, GridTrack, Length, apply_declaration};
use layout::box_tree::{BoxType, LayoutBox, build_box_tree};
use layout::geometry::{Dimensions, Rect};
use layout::grid::layout_grid;
use layout::layout_block;

fn grid_style(cols: Vec<GridTrack>, rows: Vec<GridTrack>, gap: f32) -> ComputedStyle {
    ComputedStyle {
        display: Display::Grid,
        grid_template_columns: cols,
        grid_template_rows: rows,
        grid_gap: gap,
        width: Length::Px(300.0),
        ..ComputedStyle::initial()
    }
}

fn child_auto_style() -> ComputedStyle {
    ComputedStyle {
        width: Length::Auto,
        height: Length::Px(50.0),
        ..ComputedStyle::initial()
    }
}

fn child_fixed_style(width: f32) -> ComputedStyle {
    ComputedStyle {
        width: Length::Px(width),
        height: Length::Px(50.0),
        ..ComputedStyle::initial()
    }
}

#[test]
fn grid_two_equal_columns() {
    let container = grid_style(
        vec![GridTrack::Px(150.0), GridTrack::Px(150.0)],
        vec![],
        0.0,
    );
    let c0 = child_auto_style();
    let c1 = child_auto_style();
    let styles: Vec<(usize, &ComputedStyle)> = vec![(0, &c0), (1, &c1)];
    let result = layout_grid(&container, 300.0, &styles);
    assert_eq!(result.items.len(), 2);
    assert!((result.items[0].dimensions.content.x - 0.0).abs() < 1.0);
    assert!((result.items[0].dimensions.content.width - 150.0).abs() < 1.0);
    assert!((result.items[1].dimensions.content.x - 150.0).abs() < 1.0);
    assert!((result.items[1].dimensions.content.width - 150.0).abs() < 1.0);
}

#[test]
fn grid_fr_tracks() {
    let container = grid_style(vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)], vec![], 0.0);
    let c0 = child_auto_style();
    let c1 = child_auto_style();
    let styles: Vec<(usize, &ComputedStyle)> = vec![(0, &c0), (1, &c1)];
    let result = layout_grid(&container, 300.0, &styles);
    assert_eq!(result.items.len(), 2);
    // 1fr : 2fr = 100px : 200px
    assert!((result.items[0].dimensions.content.width - 100.0).abs() < 2.0);
    assert!((result.items[1].dimensions.content.width - 200.0).abs() < 2.0);
}

#[test]
fn grid_with_gap() {
    let container = grid_style(
        vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        vec![],
        10.0,
    );
    let c0 = child_fixed_style(100.0);
    let c1 = child_fixed_style(100.0);
    let styles: Vec<(usize, &ComputedStyle)> = vec![(0, &c0), (1, &c1)];
    let result = layout_grid(&container, 300.0, &styles);
    assert_eq!(result.items.len(), 2);
    // Second item should be offset by first item width + gap
    assert!((result.items[1].dimensions.content.x - 110.0).abs() < 2.0);
}

#[test]
fn grid_single_auto_column() {
    let container = grid_style(vec![], vec![], 0.0);
    let c0 = child_auto_style();
    let styles: Vec<(usize, &ComputedStyle)> = vec![(0, &c0)];
    let result = layout_grid(&container, 300.0, &styles);
    assert_eq!(result.items.len(), 1);
    assert!((result.items[0].dimensions.content.width - 300.0).abs() < 2.0);
}

#[test]
fn grid_percent_tracks() {
    let mut container = grid_style(
        vec![GridTrack::Percent(50.0), GridTrack::Percent(50.0)],
        vec![],
        0.0,
    );
    container.width = Length::Px(400.0);
    let c0 = child_auto_style();
    let c1 = child_auto_style();
    let styles: Vec<(usize, &ComputedStyle)> = vec![(0, &c0), (1, &c1)];
    let result = layout_grid(&container, 400.0, &styles);
    assert_eq!(result.items.len(), 2);
    assert!((result.items[0].dimensions.content.width - 200.0).abs() < 2.0);
    assert!((result.items[1].dimensions.content.width - 200.0).abs() < 2.0);
}

#[test]
fn grid_integrated_block_layout() {
    let grid = grid_style(
        vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        vec![],
        0.0,
    );
    let mut root = LayoutBox {
        box_type: BoxType::BlockNode(dom::NodeId(0)),
        style: Some(grid),
        children: vec![
            LayoutBox {
                box_type: BoxType::BlockNode(dom::NodeId(1)),
                style: Some(child_fixed_style(50.0)),
                children: vec![],
                dimensions: Dimensions::default(),
                intrinsic: None,
            },
            LayoutBox {
                box_type: BoxType::BlockNode(dom::NodeId(2)),
                style: Some(child_fixed_style(50.0)),
                children: vec![],
                dimensions: Dimensions::default(),
                intrinsic: None,
            },
        ],
        dimensions: Dimensions::default(),
        intrinsic: None,
    };

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 300.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut root, &viewport);
    assert!((root.dimensions.content.height - 50.0).abs() < 2.0);
}

#[test]
fn grid_css_cascade_parsing() {
    use css::Declaration;

    let mut style = ComputedStyle::initial();
    style.display = Display::Grid;

    let decls = vec![
        Declaration::new("grid-template-columns", "100px 1fr 50%", false),
        Declaration::new("gap", "8px", false),
    ];

    for decl in &decls {
        apply_declaration(&mut style, decl);
    }

    assert_eq!(style.grid_template_columns.len(), 3);
    assert_eq!(style.grid_template_columns[0], GridTrack::Px(100.0));
    assert!((style.grid_template_columns[1].to_fr().unwrap() - 1.0).abs() < 0.01);
    assert!((style.grid_template_columns[2].to_percent().unwrap() - 50.0).abs() < 0.01);
    assert!((style.grid_gap - 8.0).abs() < 0.01);
}

#[test]
fn test_grid_html_css_end_to_end() {
    use css::{CascadeResolver, Origin, parse_stylesheet};
    use html::parse_html;

    let html = r#"<html><body>
        <div id="grid-container">
            <div id="item1">Item 1</div>
            <div id="item2">Item 2</div>
        </div>
    </body></html>"#;
    let css = r"
        #grid-container {
            display: grid;
            grid-template-columns: 100px 200px;
            grid-gap: 10px;
            width: 310px;
        }
        #item1 {
            height: 40px;
        }
        #item2 {
            height: 60px;
        }
    ";
    let doc = parse_html(html);
    let author_sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let container_id = doc.get_element_by_id("grid-container").unwrap();
    let mut root_box = build_box_tree(&doc, container_id, &styles).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut root_box, &viewport);

    assert_eq!(root_box.children.len(), 2);
    let item1 = &root_box.children[0];
    let item2 = &root_box.children[1];

    assert!((item1.dimensions.content.x - 0.0).abs() < 1.0);
    assert!((item1.dimensions.content.width - 100.0).abs() < 1.0);
    assert!((item2.dimensions.content.x - 110.0).abs() < 1.0); // 100 + 10 gap
    assert!((item2.dimensions.content.width - 200.0).abs() < 1.0);
}
