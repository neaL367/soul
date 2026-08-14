//! Audio and video media playback stream pipeline state machine.

use raster::PixelBuffer;

/// State of an audio or video playback pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPlaybackState {
    /// Playback is idle or stopped.
    Idle,
    /// Media stream is actively playing.
    Playing,
    /// Playback is paused at current position.
    Paused,
    /// Media stream has reached the end.
    Ended,
}

/// Controls audio/video media playback lifecycle.
#[derive(Debug)]
pub struct MediaPipeline {
    src_url: String,
    state: MediaPlaybackState,
    position_secs: f64,
    duration_secs: f64,
}

impl MediaPipeline {
    /// Creates a new `MediaPipeline` bound to a source media URL.
    #[must_use]
    pub const fn new(src_url: String) -> Self {
        Self {
            src_url,
            state: MediaPlaybackState::Idle,
            position_secs: 0.0,
            duration_secs: 0.0,
        }
    }

    /// Begins playback of the media stream.
    pub const fn play(&mut self) {
        self.state = MediaPlaybackState::Playing;
    }

    /// Pauses active playback.
    pub const fn pause(&mut self) {
        if matches!(self.state, MediaPlaybackState::Playing) {
            self.state = MediaPlaybackState::Paused;
        }
    }

    /// Seeks playback to the target timestamp in seconds.
    pub fn seek(&mut self, timestamp_secs: f64) {
        if timestamp_secs > 0.0 {
            self.position_secs = timestamp_secs;
        } else {
            self.position_secs = 0.0;
        }
    }

    /// Current playback state.
    #[must_use]
    pub const fn state(&self) -> MediaPlaybackState {
        self.state
    }

    /// Current playback position timestamp in seconds.
    #[must_use]
    pub const fn position(&self) -> f64 {
        self.position_secs
    }

    /// Total media duration in seconds.
    #[must_use]
    pub const fn duration(&self) -> f64 {
        self.duration_secs
    }

    /// Source media URL.
    #[must_use]
    pub fn src_url(&self) -> &str {
        &self.src_url
    }

    /// Generates or decodes an RGBA pixel buffer frame for the current playback position.
    #[must_use]
    pub fn generate_frame(&self, width: u32, height: u32) -> PixelBuffer {
        let mut buffer = PixelBuffer::new(width, height);
        // Fill frame with active media test pattern
        let alpha = if matches!(self.state, MediaPlaybackState::Playing) {
            255
        } else {
            128
        };
        for chunk in buffer.data.chunks_exact_mut(4) {
            chunk[0] = 30; // R
            chunk[1] = 144; // G
            chunk[2] = 255; // B
            chunk[3] = alpha;
        }
        buffer
    }
}
