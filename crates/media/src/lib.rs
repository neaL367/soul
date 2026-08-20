//! Media playback and HTML5 Canvas 2D raster engine.

#![allow(unsafe_code)]

pub mod canvas;
pub mod error;
pub mod mf_player;
pub mod pipeline;

pub use canvas::Canvas2DContext;
pub use error::MediaError;
pub use mf_player::{MediaContainerFormat, MediaStreamDescriptor, MfContext, MfPlayer};
pub use pipeline::{MediaPipeline, MediaPlaybackState};
