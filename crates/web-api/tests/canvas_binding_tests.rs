//! Integration tests for Canvas 2D JavaScript bindings and DOM integration.

use boa_engine::{Context, Source};
use dom::Document;
use std::sync::{Arc, Mutex};
use web_api::bind_web_apis;

#[test]
fn test_html_canvas_element_global_and_context() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("bind web apis");

    let script = r#"
        const canvas = new HTMLCanvasElement(400, 200);
        const ctx = canvas.getContext("2d");
        
        ctx.setFillStyle("red");
        ctx.fillRect(10, 10, 50, 50);
        ctx.clearRect(20, 20, 10, 10);
        
        ctx.beginPath();
        ctx.arc(100, 100, 30, 0, Math.PI, false);
        ctx.rect(50, 50, 40, 40);
        ctx.stroke();

        ctx.save();
        ctx.translate(10, 10);
        ctx.scale(2, 2);
        ctx.rotate(0.5);
        ctx.restore();

        const metrics = ctx.measureText("Soul Browser");
        metrics.width > 0 && canvas.width === 400 && canvas.height === 200;
    "#;

    let res = context
        .eval(Source::from_bytes(script))
        .expect("evaluate canvas script");
    assert_eq!(res.as_boolean(), Some(true));
}

#[test]
fn test_dom_canvas_get_context_2d() {
    let doc = Arc::new(Mutex::new(Document::new()));
    {
        let mut d = doc.lock().unwrap();
        let cid = d.create_element("canvas");
        d.set_attribute(cid, "id", "myCanvas");
        let root = d.root_id();
        d.append_child(root, cid);
    };

    let mut context = Context::default();
    bind_web_apis(&mut context, Some(doc), None, None, None).expect("bind web apis");

    let script = r#"
        const canvas = document.getElementById("myCanvas");
        const ctx = canvas.getContext("2d");
        ctx.fillText("Test", 10, 20);
        canvas.tagName === "CANVAS" && ctx !== null;
    "#;

    let res = context
        .eval(Source::from_bytes(script))
        .expect("evaluate DOM canvas script");
    assert_eq!(res.as_boolean(), Some(true));
}
