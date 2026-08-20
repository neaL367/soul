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
#[derive(Debug, Clone)]
pub struct MediaPipeline {
    src_url: String,
    state: MediaPlaybackState,
    position_secs: f64,
    duration_secs: f64,
    playback_rate: f64,
    volume: f32,
    muted: bool,
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
            playback_rate: 1.0,
            volume: 1.0,
            muted: false,
        }
    }

    /// Begins playback of the media stream.
    pub const fn play(&mut self) {
        if matches!(self.state, MediaPlaybackState::Ended) {
            self.position_secs = 0.0;
        }
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
        let clamped = if self.duration_secs > 0.0 {
            timestamp_secs.clamp(0.0, self.duration_secs)
        } else {
            timestamp_secs.max(0.0)
        };
        self.position_secs = clamped;
        if self.duration_secs > 0.0 && self.position_secs >= self.duration_secs {
            self.state = MediaPlaybackState::Ended;
        }
    }

    /// Advances playback position by elapsed real-time delta seconds multiplied by playback rate.
    pub fn step_time(&mut self, delta_secs: f64) {
        if !matches!(self.state, MediaPlaybackState::Playing) || delta_secs <= 0.0 {
            return;
        }

        self.position_secs = delta_secs.mul_add(self.playback_rate.max(0.0), self.position_secs);
        if self.duration_secs > 0.0 && self.position_secs >= self.duration_secs {
            self.position_secs = self.duration_secs;
            self.state = MediaPlaybackState::Ended;
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

    /// Sets total media stream duration in seconds.
    pub const fn set_duration(&mut self, duration_secs: f64) {
        self.duration_secs = if duration_secs > 0.0 {
            duration_secs
        } else {
            0.0
        };
    }

    /// Total media duration in seconds.
    #[must_use]
    pub const fn duration(&self) -> f64 {
        self.duration_secs
    }

    /// Sets playback speed multiplier.
    pub const fn set_playback_rate(&mut self, rate: f64) {
        self.playback_rate = if rate < 0.0625 {
            0.0625
        } else if rate > 16.0 {
            16.0
        } else {
            rate
        };
    }

    /// Current playback speed multiplier.
    #[must_use]
    pub const fn playback_rate(&self) -> f64 {
        self.playback_rate
    }

    /// Sets audio volume level in range 0.0 to 1.0.
    pub const fn set_volume(&mut self, volume: f32) {
        self.volume = if volume < 0.0 {
            0.0
        } else if volume > 1.0 {
            1.0
        } else {
            volume
        };
    }

    /// Current audio volume level (0.0 to 1.0).
    #[must_use]
    pub const fn volume(&self) -> f32 {
        self.volume
    }

    /// Sets audio mute state.
    pub const fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Whether audio is currently muted.
    #[must_use]
    pub const fn is_muted(&self) -> bool {
        self.muted
    }

    /// Source media URL.
    #[must_use]
    pub fn src_url(&self) -> &str {
        &self.src_url
    }

    /// Generates or decodes an RGBA pixel buffer frame for the current playback position.
    #[must_use]
    pub fn generate_frame(&self, width: u32, height: u32) -> PixelBuffer {
        self.generate_frame_with_tint(width, height, (30, 144, 255))
    }

    /// Generates an RGBA pixel buffer frame with custom base tint.
    #[must_use]
    pub fn generate_frame_with_tint(
        &self,
        width: u32,
        height: u32,
        tint: (u8, u8, u8),
    ) -> PixelBuffer {
        let mut buffer = PixelBuffer::new(width, height);
        let alpha = match self.state {
            MediaPlaybackState::Playing => 255,
            MediaPlaybackState::Paused => 200,
            MediaPlaybackState::Idle | MediaPlaybackState::Ended => 100,
        };
        for chunk in buffer.data.chunks_exact_mut(4) {
            chunk[0] = tint.0;
            chunk[1] = tint.1;
            chunk[2] = tint.2;
            chunk[3] = alpha;
        }
        buffer
    }
}
