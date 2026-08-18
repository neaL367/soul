//! Integration tests for image decoding, PNG encoding, format sniffing, and animation.

use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, RgbaImage};
use image_decode::{ImageDecoder, encode_png};

#[test]
fn test_decode_svg_vector() {
    let svg = r#"
    <svg width="100" height="100" xmlns="http://www.w3.org/2000/svg">
        <rect width="100" height="100" fill="red" />
    </svg>
    "#;

    let decoded = ImageDecoder::decode_svg(svg.as_bytes(), 50, 50).expect("failed to decode SVG");
    assert_eq!(decoded.width, 50);
    assert_eq!(decoded.height, 50);
    assert_eq!(decoded.rgba_pixels.len(), 50 * 50 * 4);

    // Verify first pixel is red
    assert_eq!(decoded.rgba_pixels[0], 255); // R
    assert_eq!(decoded.rgba_pixels[1], 0); // G
    assert_eq!(decoded.rgba_pixels[2], 0); // B
    assert_eq!(decoded.rgba_pixels[3], 255); // A
}

#[test]
fn test_decode_svg_zero_dimensions_rejected() {
    let svg = r#"
    <svg width="0" height="0" xmlns="http://www.w3.org/2000/svg">
        <rect width="100" height="100" fill="red" />
    </svg>
    "#;

    let result = ImageDecoder::decode_svg(svg.as_bytes(), 50, 50);
    assert!(
        result.is_err(),
        "zero-dimension SVG must not produce NaN scaling"
    );
}

#[test]
fn test_png_encode_and_decode_roundtrip() {
    // 2x2 blue image with alpha
    let pixels: Vec<u8> = vec![
        0, 0, 255, 255, // top-left
        0, 0, 255, 255, // top-right
        0, 0, 255, 255, // bottom-left
        0, 0, 255, 255, // bottom-right
    ];

    let png_bytes = encode_png(&pixels, 2, 2).expect("failed to encode PNG");
    assert!(!png_bytes.is_empty());

    let decoded = ImageDecoder::decode_raster(&png_bytes).expect("failed to decode PNG");
    assert_eq!(decoded.width, 2);
    assert_eq!(decoded.height, 2);
    assert_eq!(decoded.rgba_pixels, pixels);
}

#[test]
fn test_decode_auto_detects_svg_and_raster() {
    let svg = r#"<svg width="10" height="10" xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10" fill="green"/></svg>"#;
    let decoded_svg = ImageDecoder::decode_auto(svg.as_bytes()).expect("auto-decode svg");
    assert_eq!(decoded_svg.width, 10);
    assert_eq!(decoded_svg.height, 10);

    let pixels: Vec<u8> = vec![255, 255, 0, 255]; // 1x1 yellow
    let png_bytes = encode_png(&pixels, 1, 1).expect("encode 1x1 png");
    let decoded_png = ImageDecoder::decode_auto(&png_bytes).expect("auto-decode png");
    assert_eq!(decoded_png.width, 1);
    assert_eq!(decoded_png.height, 1);
    assert_eq!(decoded_png.rgba_pixels, pixels);
}

#[test]
fn test_decode_gif_animation_frames() {
    // Create a 2-frame GIF in memory
    let mut gif_data = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut gif_data);
        let frame1_img = RgbaImage::from_raw(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        )
        .unwrap();
        let frame2_img = RgbaImage::from_raw(
            2,
            2,
            vec![
                0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ],
        )
        .unwrap();

        let frame1 = Frame::from_parts(frame1_img, 0, 0, Delay::from_numer_denom_ms(100, 1));
        let frame2 = Frame::from_parts(frame2_img, 0, 0, Delay::from_numer_denom_ms(200, 1));

        encoder.encode_frame(frame1).expect("encode frame 1");
        encoder.encode_frame(frame2).expect("encode frame 2");
    }

    let anim = ImageDecoder::decode_animation(&gif_data).expect("decode animated gif");
    assert_eq!(anim.width, 2);
    assert_eq!(anim.height, 2);
    assert_eq!(anim.frames.len(), 2);
    assert_eq!(anim.frames[0].duration_ms, 100);
    assert_eq!(anim.frames[1].duration_ms, 200);
}
