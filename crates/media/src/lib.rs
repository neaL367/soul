//! Media playback and HTML5 Canvas 2D raster engine.

pub mod canvas;
pub mod error;
pub mod pipeline;

pub use canvas::Canvas2DContext;
pub use error::MediaError;
pub use pipeline::{MediaPipeline, MediaPlaybackState};
