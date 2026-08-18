//! Media playback and HTML5 Canvas 2D raster engine.

pub mod canvas;
pub mod error;
pub mod mf_player;
pub mod pipeline;

pub use canvas::Canvas2DContext;
pub use error::MediaError;
pub use mf_player::{MediaContainerFormat, MediaStreamDescriptor, MfPlayer};
pub use pipeline::{MediaPipeline, MediaPlaybackState};
