//! In-memory RGBA pixel buffer representation.

/// 2D pixel buffer containing 32-bit RGBA rasterized image data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelBuffer {
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
    /// Raw RGBA8 pixel byte array (length = width * height * 4).
    pub data: Vec<u8>,
}

impl PixelBuffer {
    /// Creates a new empty `PixelBuffer` filled with transparent pixels.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            data: vec![0; size],
        }
    }

    /// Creates a `PixelBuffer` from existing RGBA pixel byte data.
    #[must_use]
    pub const fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data,
        }
    }

    /// Returns the RGBA pixel value at coordinate `(x, y)` if within bounds.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        if idx + 4 <= self.data.len() {
            Some([
                self.data[idx],
                self.data[idx + 1],
                self.data[idx + 2],
                self.data[idx + 3],
            ])
        } else {
            None
        }
    }

    /// Returns the raw pixel byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}
