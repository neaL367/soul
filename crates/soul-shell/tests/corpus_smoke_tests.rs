//! Target-site corpus smoke — primary MVP signal (docs/blogs, static-to-moderately-dynamic).
//! Informational harness that verifies the full pipeline (parse → style → layout → paint → raster)
//! does not panic and produces non-zero layout for representative fixtures.

use css::{Origin, parse_stylesheet};
use html::parse_html;
use layout::geometry::{Dimensions, Rect};
use layout::{build_box_tree, layout_block};
use paint::builder::DisplayListBuilder;
use raster::CpuRasterizer;
use std::collections::HashMap;

fn layout_html(html: &str, css: &str, width: f32) -> f32 {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = css::CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let mut tree = build_box_tree(&doc, doc.root_id(), &styles).expect("box tree");
    let containing = Dimensions {
        content: Rect::new(0.0, 0.0, width, 0.0),
        ..Default::default()
    };
    layout_block(&mut tree, &containing);
    // Height of root's content
    tree.dimensions.content.height
}

#[test]
fn corpus_docs_page_smoke() {
    let html = r##"<html><head><title>Docs</title></head><body>
        <header><nav><a href="/">Home</a> <a href="/docs">Docs</a></nav></header>
        <main><article><h1>Getting Started</h1><p>This is a <a href="#install">guide</a> for the project.</p>
        <pre><code>cargo install soul-browser</code></pre><ul><li>Fast</li><li>Safe</li></ul></article>
        <aside><h2>On this page</h2><ul><li>Install</li><li>Usage</li></ul></aside></main>
        <footer><p>© 2026 Soul</p></footer>
    </body></html>"##;
    let css = r"body { font-family: sans-serif; font-size: 16px; color: #111; }
        header { display: block; background: #f5f5f5; padding: 10px; }
        main { display: flex; flex-direction: row; }
        article { display: block; width: 70%; padding: 20px; }
        aside { display: block; width: 30%; background: #fafafa; }
        h1 { font-size: 24px; color: #222; }
        a[href] { color: #06c; }
        pre { background: #eee; padding: 10px; }
    ";
    let h = layout_html(html, css, 800.0);
    assert!(
        h > 100.0,
        "docs page should have substantial height, got {h}"
    );
}

#[test]
fn corpus_blog_post_smoke() {
    let html = r#"<html><body>
        <article class="post"><h1>My Blog Post</h1><p class="meta">By Alice — Jan 2026</p>
        <p>Hello world. This is a <em>blog</em> post with <strong>formatting</strong>.</p>
        <blockquote><p>Quote here</p></blockquote>
        <p>More text with <a href="https://example.com">a link</a> and an image:</p>
        <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==" />
        </article>
    </body></html>"#;
    let css = r"article.post { display: block; margin: 20px; }
        h1 { font-size: 28px; }
        p { display: block; margin: 10px 0; }
        blockquote { border-left: 4px solid #ccc; padding-left: 10px; color: #555; }
        img { display: block; width: 100px; height: 100px; }
    ";
    let h = layout_html(html, css, 600.0);
    assert!(h > 150.0, "blog post should layout, got {h}");
}

#[test]
fn corpus_form_page_smoke() {
    let html = r#"<html><body>
        <form id="f"><label for="q">Search</label><input type="text" id="q" name="q" />
        <button type="submit">Go</button></form>
        <div class="results"><div class="result"><h2><a href="/a">Result A</a></h2><p>Snippet</p></div>
        <div class="result"><h2><a href="/b">Result B</a></h2><p>Snippet</p></div></div>
    </body></html>"#;
    let css = r#"form { display: flex; flex-direction: row; gap: 10px; padding: 10px; }
        input[type="text"] { flex-grow: 1; border: 1px solid #ccc; padding: 5px; }
        .results { display: block; }
        .result { display: block; margin: 10px; padding: 10px; border: 1px solid #eee; }
        a[href] { color: blue; }
    "#;
    let h = layout_html(html, css, 700.0);
    assert!(h > 80.0, "form page should layout, got {h}");
}

#[test]
fn corpus_raster_smoke_produces_pixels() {
    let html = r#"<html><body><div id="a">A</div></body></html>"#;
    let doc = parse_html(html);
    let css = r"#a { display: block; width: 100px; height: 50px; background-color: #ff0000; }";
    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = css::CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();
    let mut tree = build_box_tree(&doc, doc.root_id(), &styles).unwrap();
    let containing = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut tree, &containing);

    let list = DisplayListBuilder::build(&tree, &HashMap::new());
    assert!(!list.items.is_empty(), "display list should not be empty");

    let buffer = CpuRasterizer::rasterize(&list, 800, 600).expect("raster");
    assert_eq!(buffer.width, 800);
    assert_eq!(buffer.height, 600);
    // Check that at least some pixel is non-white (background drawn)
    let non_white = buffer
        .data
        .chunks(4)
        .any(|c| c[0] != 255 || c[1] != 255 || c[2] != 255);
    assert!(non_white, "raster should contain drawn content");
}
