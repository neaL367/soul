//! Integration tests for HTML5 `HTMLMediaElement`, `HTMLVideoElement`, and `HTMLAudioElement`.

use boa_engine::{Context, Source};
use web_api::bind_web_apis;

#[test]
fn test_video_element_properties_and_play_promise() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const video = new HTMLVideoElement();
        video.src = "https://example.com/stream.mp4";
        video.currentTime = 12.5;
        video.volume = 0.8;
        video.muted = true;

        const isPausedBefore = video.paused;
        video.play();
        const isPausedAfter = video.paused;

        video.src === "https://example.com/stream.mp4" &&
        video.currentTime === 12.5 &&
        video.volume === 0.8 &&
        video.muted === true &&
        video.videoWidth === 1920 &&
        video.videoHeight === 1080 &&
        isPausedBefore === true &&
        isPausedAfter === false;
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}

#[test]
fn test_audio_element_constructor_and_can_play_type() {
    let mut context = Context::default();
    bind_web_apis(&mut context, None, None, None, None).expect("failed to bind web APIs");

    let script = Source::from_bytes(
        r#"
        const audio = new Audio("https://example.com/sound.mp3");
        const canPlayMp3 = audio.canPlayType("audio/mp3");
        const canPlayBogus = audio.canPlayType("audio/x-unknown-fake");

        audio.src === "https://example.com/sound.mp3" &&
        audio.paused === true &&
        canPlayMp3 === "probably" &&
        canPlayBogus === "";
        "#,
    );

    let result = context.eval(script).expect("eval failed");
    assert_eq!(result.as_boolean(), Some(true));
}
