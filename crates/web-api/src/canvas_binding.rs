//! HTML5 Canvas 2D JavaScript API bindings (WHATWG §4.12.5).
//!
//! Provides the global `HTMLCanvasElement` constructor, `<canvas>` DOM integration,
//! and the full `CanvasRenderingContext2D` drawing state machine.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::option_if_let_else
)]

use boa_engine::{
    Context, JsArgs, JsError, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
};
use media::Canvas2DContext;
use std::sync::{Arc, Mutex};

/// Traceable wrapper holding a shared reference to the raster [`Canvas2DContext`].
#[derive(Clone, Trace, Finalize)]
pub struct CanvasHolder(#[unsafe_ignore_trace] pub Arc<Mutex<Canvas2DContext>>);

/// Registers the global `HTMLCanvasElement` constructor into a Boa `Context`.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn register_canvas(ctx: &mut Context) -> JsResult<()> {
    let canvas_constructor = NativeFunction::from_copy_closure(|_this, args, ctx| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let width = args
            .first()
            .and_then(JsValue::as_number)
            .map_or(300, |n| n.max(1.0) as u32);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let height = args
            .get(1)
            .and_then(JsValue::as_number)
            .map_or(150, |n| n.max(1.0) as u32);

        let canvas_obj = create_canvas_element(ctx, width, height)?;
        Ok(JsValue::from(canvas_obj))
    });

    let js_fn = FunctionObjectBuilder::new(ctx.realm(), canvas_constructor)
        .constructor(true)
        .name(js_string!("HTMLCanvasElement"))
        .length(0)
        .build();

    ctx.register_global_property(
        js_string!("HTMLCanvasElement"),
        js_fn,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}

/// Constructs a new `HTMLCanvasElement` JavaScript object with backing raster store.
///
/// # Errors
///
/// Returns `JsResult` on allocation or property creation failure.
pub fn create_canvas_element(ctx: &mut Context, width: u32, height: u32) -> JsResult<JsObject> {
    let canvas_ctx = Canvas2DContext::new(width, height)
        .map_err(|e| JsError::from_opaque(JsValue::from(js_string!(e.to_string()))))?;
    let shared_ctx = Arc::new(Mutex::new(canvas_ctx));
    let holder = CanvasHolder(shared_ctx);

    let get_context_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let context_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                if context_id.eq_ignore_ascii_case("2d") {
                    let ctx2d_obj = create_canvas_context_2d(ctx, captures.clone())?;
                    Ok(JsValue::from(ctx2d_obj))
                } else {
                    Ok(JsValue::null())
                }
            },
            holder,
        ),
    )
    .build();

    let obj = ObjectInitializer::new(ctx)
        .property(js_string!("width"), f64::from(width), Attribute::all())
        .property(js_string!("height"), f64::from(height), Attribute::all())
        .property(
            js_string!("getContext"),
            JsValue::from(get_context_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .build();

    Ok(obj)
}

/// Constructs a `CanvasRenderingContext2D` JavaScript wrapper.
#[allow(clippy::too_many_lines)]
pub fn create_canvas_context_2d(ctx: &mut Context, holder: CanvasHolder) -> JsResult<JsObject> {
    let fill_rect_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let w = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let h = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.fill_rect(x, y, w, h);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let stroke_rect_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let w = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let h = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.stroke_rect(x, y, w, h);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let clear_rect_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let w = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let h = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.clear_rect(x, y, w, h);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let begin_path_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.begin_path();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let close_path_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.close_path();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let move_to_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.move_to(x, y);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let line_to_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.line_to(x, y);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let arc_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let r = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let sa = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;
                let ea = args.get_or_undefined(4).as_number().unwrap_or(0.0) as f32;
                let ccw = args.get(5).and_then(JsValue::as_boolean).unwrap_or(false);
                if let Ok(mut c) = caps.0.lock() {
                    c.arc(x, y, r, sa, ea, ccw);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let rect_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let w = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let h = args.get_or_undefined(3).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.rect(x, y, w, h);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let fill_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.fill();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let stroke_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.stroke();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let save_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.save();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let restore_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, caps, _ctx| {
                if let Ok(mut c) = caps.0.lock() {
                    c.restore();
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let translate_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.translate(x, y);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let scale_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let x = args.get_or_undefined(0).as_number().unwrap_or(1.0) as f32;
                let y = args.get_or_undefined(1).as_number().unwrap_or(1.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.scale(x, y);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let rotate_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let angle = args.get_or_undefined(0).as_number().unwrap_or(0.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.rotate(angle);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let set_fill_style_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, ctx| {
                let color_str = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let (r, g, b, a) = parse_css_color(&color_str);
                if let Ok(mut c) = caps.0.lock() {
                    c.set_fill_style(r, g, b, a);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let set_stroke_style_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, ctx| {
                let color_str = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let (r, g, b, a) = parse_css_color(&color_str);
                if let Ok(mut c) = caps.0.lock() {
                    c.set_stroke_style(r, g, b, a);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let set_line_width_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, _ctx| {
                let width = args.get_or_undefined(0).as_number().unwrap_or(1.0) as f32;
                if let Ok(mut c) = caps.0.lock() {
                    c.set_line_width(width);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let fill_text_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, ctx| {
                let text = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let x = args.get_or_undefined(1).as_number().unwrap_or(0.0) as f32;
                let y = args.get_or_undefined(2).as_number().unwrap_or(0.0) as f32;
                let max_w = args.get(3).and_then(JsValue::as_number).map(|n| n as f32);
                if let Ok(mut c) = caps.0.lock() {
                    c.fill_text(&text, x, y, max_w);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let measure_text_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, caps, ctx| {
                let text = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let metrics = if let Ok(c) = caps.0.lock() {
                    c.measure_text(&text)
                } else {
                    media::TextMetrics {
                        width: 0.0,
                        actual_bounding_box_ascent: 0.0,
                        actual_bounding_box_descent: 0.0,
                    }
                };

                let res = ObjectInitializer::new(ctx)
                    .property(
                        js_string!("width"),
                        f64::from(metrics.width),
                        Attribute::READONLY,
                    )
                    .property(
                        js_string!("actualBoundingBoxAscent"),
                        f64::from(metrics.actual_bounding_box_ascent),
                        Attribute::READONLY,
                    )
                    .property(
                        js_string!("actualBoundingBoxDescent"),
                        f64::from(metrics.actual_bounding_box_descent),
                        Attribute::READONLY,
                    )
                    .build();

                Ok(JsValue::from(res))
            },
            holder,
        ),
    )
    .build();

    let obj = ObjectInitializer::new(ctx)
        .property(
            js_string!("fillRect"),
            JsValue::from(fill_rect_fn),
            Attribute::all(),
        )
        .property(
            js_string!("strokeRect"),
            JsValue::from(stroke_rect_fn),
            Attribute::all(),
        )
        .property(
            js_string!("clearRect"),
            JsValue::from(clear_rect_fn),
            Attribute::all(),
        )
        .property(
            js_string!("beginPath"),
            JsValue::from(begin_path_fn),
            Attribute::all(),
        )
        .property(
            js_string!("closePath"),
            JsValue::from(close_path_fn),
            Attribute::all(),
        )
        .property(
            js_string!("moveTo"),
            JsValue::from(move_to_fn),
            Attribute::all(),
        )
        .property(
            js_string!("lineTo"),
            JsValue::from(line_to_fn),
            Attribute::all(),
        )
        .property(js_string!("arc"), JsValue::from(arc_fn), Attribute::all())
        .property(js_string!("rect"), JsValue::from(rect_fn), Attribute::all())
        .property(js_string!("fill"), JsValue::from(fill_fn), Attribute::all())
        .property(
            js_string!("stroke"),
            JsValue::from(stroke_fn),
            Attribute::all(),
        )
        .property(js_string!("save"), JsValue::from(save_fn), Attribute::all())
        .property(
            js_string!("restore"),
            JsValue::from(restore_fn),
            Attribute::all(),
        )
        .property(
            js_string!("translate"),
            JsValue::from(translate_fn),
            Attribute::all(),
        )
        .property(
            js_string!("scale"),
            JsValue::from(scale_fn),
            Attribute::all(),
        )
        .property(
            js_string!("rotate"),
            JsValue::from(rotate_fn),
            Attribute::all(),
        )
        .property(
            js_string!("setFillStyle"),
            JsValue::from(set_fill_style_fn),
            Attribute::all(),
        )
        .property(
            js_string!("setStrokeStyle"),
            JsValue::from(set_stroke_style_fn),
            Attribute::all(),
        )
        .property(
            js_string!("setLineWidth"),
            JsValue::from(set_line_width_fn),
            Attribute::all(),
        )
        .property(
            js_string!("fillText"),
            JsValue::from(fill_text_fn),
            Attribute::all(),
        )
        .property(
            js_string!("measureText"),
            JsValue::from(measure_text_fn),
            Attribute::all(),
        )
        .build();

    Ok(obj)
}

/// Parses standard CSS color strings into RGBA components (0.0 to 1.0).
fn parse_css_color(color: &str) -> (f32, f32, f32, f32) {
    let c = color.trim().to_ascii_lowercase();
    match c.as_str() {
        "red" => (1.0, 0.0, 0.0, 1.0),
        "green" => (0.0, 0.5, 0.0, 1.0),
        "lime" => (0.0, 1.0, 0.0, 1.0),
        "blue" => (0.0, 0.0, 1.0, 1.0),
        "black" => (0.0, 0.0, 0.0, 1.0),
        "white" => (1.0, 1.0, 1.0, 1.0),
        "yellow" => (1.0, 1.0, 0.0, 1.0),
        "cyan" | "aqua" => (0.0, 1.0, 1.0, 1.0),
        "magenta" | "fuchsia" => (1.0, 0.0, 1.0, 1.0),
        "gray" | "grey" => (0.5, 0.5, 0.5, 1.0),
        "orange" => (1.0, 0.65, 0.0, 1.0),
        "purple" => (0.5, 0.0, 0.5, 1.0),
        "transparent" => (0.0, 0.0, 0.0, 0.0),
        hex if hex.starts_with('#') => parse_hex_color(hex),
        _ => (0.0, 0.0, 0.0, 1.0),
    }
}

fn parse_hex_color(hex: &str) -> (f32, f32, f32, f32) {
    let s = hex.trim_start_matches('#');
    if s.len() == 6
        && let Ok(v) = u32::from_str_radix(s, 16)
    {
        let r = ((v >> 16) & 0xFF) as f32 / 255.0;
        let g = ((v >> 8) & 0xFF) as f32 / 255.0;
        let b = (v & 0xFF) as f32 / 255.0;
        return (r, g, b, 1.0);
    }
    if s.len() == 3
        && let Ok(v) = u16::from_str_radix(s, 16)
    {
        let r = (((v >> 8) & 0xF) * 17) as f32 / 255.0;
        let g = (((v >> 4) & 0xF) * 17) as f32 / 255.0;
        let b = ((v & 0xF) * 17) as f32 / 255.0;
        return (r, g, b, 1.0);
    }
    (0.0, 0.0, 0.0, 1.0)
}
