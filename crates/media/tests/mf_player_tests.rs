//! Integration tests for Windows Media Foundation container sniffing and format probing.

use media::{MediaContainerFormat, MfPlayer};

#[test]
fn test_media_container_sniffing() {
    // MP4 header (ftyp at offset 4)
    let mp4_header = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00";
    assert_eq!(
        MfPlayer::sniff_format(mp4_header),
        MediaContainerFormat::Mp4
    );

    // WebM header
    let webm_header = b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81";
    assert_eq!(
        MfPlayer::sniff_format(webm_header),
        MediaContainerFormat::WebM
    );

    // WAV header
    let wav_header = b"RIFF\x24\x08\x00\x00WAVEfmt ";
    assert_eq!(
        MfPlayer::sniff_format(wav_header),
        MediaContainerFormat::Wav
    );

    // MP3 ID3 header
    let mp3_header = b"ID3\x03\x00\x00\x00\x00\x0f\x76";
    assert_eq!(
        MfPlayer::sniff_format(mp3_header),
        MediaContainerFormat::Mp3
    );
}

#[test]
fn test_media_stream_probe_and_frame_decode() {
    let mp4_header = b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00";
    let descriptor = MfPlayer::probe_stream("https://example.com/video.mp4", mp4_header)
        .expect("probe mp4 stream");

    assert_eq!(descriptor.format, MediaContainerFormat::Mp4);
    assert!(descriptor.has_video);
    assert!(descriptor.has_audio);
    assert_eq!(descriptor.video_dimensions, Some((1920, 1080)));

    let player = MfPlayer;
    let frame = player.decode_frame(1.5, 640, 360, (255, 0, 0));
    assert_eq!(frame.width, 640);
    assert_eq!(frame.height, 360);
    assert_eq!(frame.data[0], 255);
}

#[test]
fn test_truncated_headers_do_not_panic() {
    // Short "RIFF" prefixes of every length must never panic on slicing.
    for len in 0..12 {
        let truncated: Vec<u8> = b"RIFF\x24\x08\x00\x00WAVEfmt "
            .iter()
            .take(len)
            .copied()
            .collect();
        assert_eq!(
            MfPlayer::sniff_format(&truncated),
            MediaContainerFormat::Unknown,
            "len {len} must be Unknown, not a panic"
        );
    }

    // A single 0xFF byte (truncated MP3 sync frame) must not index byte 1.
    assert_eq!(
        MfPlayer::sniff_format(&[0xFF]),
        MediaContainerFormat::Unknown
    );

    // Empty and tiny inputs.
    assert_eq!(MfPlayer::sniff_format(&[]), MediaContainerFormat::Unknown);
    assert_eq!(
        MfPlayer::sniff_format(&[0x1A, 0x45]),
        MediaContainerFormat::Unknown
    );

    // Full-length headers still sniff correctly after the guard change.
    let wav = b"RIFF\x24\x08\x00\x00WAVEfmt ";
    assert_eq!(MfPlayer::sniff_format(wav), MediaContainerFormat::Wav);
    let mp3_sync = [0xFF, 0xFB, 0x90, 0x64, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(MfPlayer::sniff_format(&mp3_sync), MediaContainerFormat::Mp3);
}

#[test]
fn test_mf_context_initialization() {
    let ctx = media::MfContext::init();
    assert!(ctx.is_ok());
}

#[test]
fn test_media_audio_probing() {
    let wav_header = b"RIFF\x24\x08\x00\x00WAVEfmt ";
    let descriptor = MfPlayer::probe_stream("https://example.com/audio.wav", wav_header)
        .expect("probe wav audio");
    assert_eq!(descriptor.format, MediaContainerFormat::Wav);
    assert!(descriptor.has_audio);
    assert!(!descriptor.has_video);
    assert_eq!(descriptor.codec, "pcm");

    let mp3_header = b"ID3\x03\x00\x00\x00\x00\x0f\x76";
    let descriptor = MfPlayer::probe_stream("https://example.com/audio.mp3", mp3_header)
        .expect("probe mp3 audio");
    assert_eq!(descriptor.format, MediaContainerFormat::Mp3);
    assert!(descriptor.has_audio);
    assert!(!descriptor.has_video);
    assert_eq!(descriptor.codec, "mp3");
}
