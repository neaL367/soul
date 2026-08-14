//! 2D CPU software rasterization backend powered by `tiny-skia`.

pub mod buffer;
pub mod cpu;
pub mod error;

pub use buffer::PixelBuffer;
pub use cpu::CpuRasterizer;
pub use error::RasterError;
