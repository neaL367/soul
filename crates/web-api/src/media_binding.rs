//! W3C HTML5 `HTMLMediaElement`, `HTMLVideoElement`, and `HTMLAudioElement` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsObject, JsResult, JsString, JsValue};

const MEDIA_SRC_PROP: &str = "__soul_media_src__";
const MEDIA_PAUSED_PROP: &str = "__soul_media_paused__";
const MEDIA_CURRENT_TIME_PROP: &str = "__soul_media_current_time__";
const MEDIA_DURATION_PROP: &str = "__soul_media_duration__";
const MEDIA_MUTED_PROP: &str = "__soul_media_muted__";
const MEDIA_VOLUME_PROP: &str = "__soul_media_volume__";

/// Registers `HTMLMediaElement`, `HTMLVideoElement`, `HTMLAudioElement`, and `Audio` into the JS context.
pub fn register_media(context: &mut Context) {
    let video_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_video_constructor),
    )
    .constructor(true)
    .name(js_string!("HTMLVideoElement"))
    .length(0)
    .build();

    let audio_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_audio_constructor),
    )
    .constructor(true)
    .name(js_string!("HTMLAudioElement"))
    .length(0)
    .build();

    let audio_factory = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_audio_constructor),
    )
    .constructor(true)
    .name(js_string!("Audio"))
    .length(1)
    .build();

    let global = context.global_object();
    let _ = global.set(js_string!("HTMLVideoElement"), video_ctor, false, context);
    let _ = global.set(js_string!("HTMLAudioElement"), audio_ctor, false, context);
    let _ = global.set(js_string!("Audio"), audio_factory, false, context);
}

/// Creates a new `HTMLVideoElement` JS instance object.
#[must_use]
pub fn create_video_element(context: &mut Context) -> JsObject {
    let base = create_media_element_base(context, true);
    let _ = base.set(
        js_string!("videoWidth"),
        JsValue::from(1920),
        false,
        context,
    );
    let _ = base.set(
        js_string!("videoHeight"),
        JsValue::from(1080),
        false,
        context,
    );
    base
}

/// Creates a new `HTMLAudioElement` JS instance object.
#[must_use]
pub fn create_audio_element(context: &mut Context) -> JsObject {
    create_media_element_base(context, false)
}

#[allow(clippy::too_many_lines)]
fn create_media_element_base(context: &mut Context, is_video: bool) -> JsObject {
    let get_src = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::undefined()),
                |o| o.get(js_string!(MEDIA_SRC_PROP), ctx),
            )
        }),
    )
    .build();

    let set_src = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, args, ctx| {
            if let Some(o) = this.as_object() {
                let val = args.get_or_undefined(0);
                let _ = o.set(js_string!(MEDIA_SRC_PROP), val.clone(), false, ctx);
            }
            Ok(JsValue::undefined())
        }),
    )
    .build();

    let get_paused = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(true)),
                |o| o.get(js_string!(MEDIA_PAUSED_PROP), ctx),
            )
        }),
    )
    .build();

    let get_current_time = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(0.0)),
                |o| o.get(js_string!(MEDIA_CURRENT_TIME_PROP), ctx),
            )
        }),
    )
    .build();

    let set_current_time = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, args, ctx| {
            if let Some(o) = this.as_object() {
                let time_val = args.get_or_undefined(0);
                let _ = o.set(
                    js_string!(MEDIA_CURRENT_TIME_PROP),
                    time_val.clone(),
                    false,
                    ctx,
                );
            }
            Ok(JsValue::undefined())
        }),
    )
    .build();

    let get_duration = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(0.0)),
                |o| o.get(js_string!(MEDIA_DURATION_PROP), ctx),
            )
        }),
    )
    .build();

    let get_muted = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(false)),
                |o| o.get(js_string!(MEDIA_MUTED_PROP), ctx),
            )
        }),
    )
    .build();

    let set_muted = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, args, ctx| {
            if let Some(o) = this.as_object() {
                let muted = args.get_or_undefined(0).to_boolean();
                let _ = o.set(
                    js_string!(MEDIA_MUTED_PROP),
                    JsValue::from(muted),
                    false,
                    ctx,
                );
            }
            Ok(JsValue::undefined())
        }),
    )
    .build();

    let get_volume = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(1.0)),
                |o| o.get(js_string!(MEDIA_VOLUME_PROP), ctx),
            )
        }),
    )
    .build();

    let set_volume = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, args, ctx| {
            if let Some(o) = this.as_object() {
                let vol = args.get_or_undefined(0).to_number(ctx).unwrap_or(1.0);
                let clamped = vol.clamp(0.0, 1.0);
                let _ = o.set(
                    js_string!(MEDIA_VOLUME_PROP),
                    JsValue::from(clamped),
                    false,
                    ctx,
                );
            }
            Ok(JsValue::undefined())
        }),
    )
    .build();

    let mut init = ObjectInitializer::new(context);
    let init_ref = init
        .property(
            js_string!(MEDIA_SRC_PROP),
            JsValue::from(js_string!("")),
            Attribute::all(),
        )
        .property(
            js_string!(MEDIA_PAUSED_PROP),
            JsValue::from(true),
            Attribute::all(),
        )
        .property(
            js_string!(MEDIA_CURRENT_TIME_PROP),
            JsValue::from(0.0),
            Attribute::all(),
        )
        .property(
            js_string!(MEDIA_DURATION_PROP),
            JsValue::from(60.0),
            Attribute::all(),
        )
        .property(
            js_string!(MEDIA_MUTED_PROP),
            JsValue::from(false),
            Attribute::all(),
        )
        .property(
            js_string!(MEDIA_VOLUME_PROP),
            JsValue::from(1.0),
            Attribute::all(),
        )
        .property(
            js_string!("readyState"),
            JsValue::from(4), // HAVE_ENOUGH_DATA
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("networkState"),
            JsValue::from(1), // NETWORK_IDLE
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .accessor(
            js_string!("src"),
            Some(get_src),
            Some(set_src),
            Attribute::all(),
        )
        .accessor(
            js_string!("paused"),
            Some(get_paused),
            None,
            Attribute::all(),
        )
        .accessor(
            js_string!("currentTime"),
            Some(get_current_time),
            Some(set_current_time),
            Attribute::all(),
        )
        .accessor(
            js_string!("duration"),
            Some(get_duration),
            None,
            Attribute::all(),
        )
        .accessor(
            js_string!("muted"),
            Some(get_muted),
            Some(set_muted),
            Attribute::all(),
        )
        .accessor(
            js_string!("volume"),
            Some(get_volume),
            Some(set_volume),
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(js_media_play),
            js_string!("play"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_media_pause),
            js_string!("pause"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_media_load),
            js_string!("load"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_media_can_play_type),
            js_string!("canPlayType"),
            1,
        );

    if is_video {
        init_ref
            .property(
                js_string!("videoWidth"),
                JsValue::from(1920),
                Attribute::READONLY | Attribute::ENUMERABLE,
            )
            .property(
                js_string!("videoHeight"),
                JsValue::from(1080),
                Attribute::READONLY | Attribute::ENUMERABLE,
            );
    }

    init_ref.build()
}

fn js_video_constructor(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let instance = create_video_element(context);
    Ok(JsValue::from(instance))
}

fn js_audio_constructor(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let instance = create_audio_element(context);
    if let Some(src_arg) = args.first() {
        let src_str = src_arg.to_string(context)?;
        let _ = instance.set(js_string!("src"), JsValue::from(src_str), false, context);
    }
    Ok(JsValue::from(instance))
}

fn js_media_play(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    if let Some(obj) = this.as_object() {
        let _ = obj.set(
            js_string!(MEDIA_PAUSED_PROP),
            JsValue::from(false),
            false,
            context,
        );
    }

    let promise = JsPromise::new(
        |resolvers, ctx| {
            let _ = resolvers.resolve.call(&JsValue::undefined(), &[], ctx);
            Ok(JsValue::undefined())
        },
        context,
    );

    Ok(JsValue::from(promise))
}

fn js_media_pause(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    if let Some(obj) = this.as_object() {
        let _ = obj.set(
            js_string!(MEDIA_PAUSED_PROP),
            JsValue::from(true),
            false,
            context,
        );
    }
    Ok(JsValue::undefined())
}

const fn js_media_load(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

fn js_media_can_play_type(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let type_str = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped()
        .to_ascii_lowercase();

    let ans = if type_str.contains("video/mp4")
        || type_str.contains("video/webm")
        || type_str.contains("audio/mpeg")
        || type_str.contains("audio/mp3")
        || type_str.contains("audio/wav")
    {
        "probably"
    } else {
        ""
    };

    Ok(JsValue::from(JsString::from(ans)))
}
