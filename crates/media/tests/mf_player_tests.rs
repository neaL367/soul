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
