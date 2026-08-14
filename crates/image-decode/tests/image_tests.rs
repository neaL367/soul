//! Integration tests for PNG and SVG decoding.

use image_decode::ImageDecoder;

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

    // Verify first pixel is red (premultiplied RGBA)
    assert_eq!(decoded.rgba_pixels[0], 255); // R
    assert_eq!(decoded.rgba_pixels[1], 0); // G
    assert_eq!(decoded.rgba_pixels[2], 0); // B
    assert_eq!(decoded.rgba_pixels[3], 255); // A
}
