//! Windows DPI scaling and multi-monitor geometry conversion utilities.

/// Standard baseline Windows desktop DPI (100% scale factor).
pub const BASELINE_DPI: f64 = 96.0;

/// Represents a display DPI and its corresponding scale factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiScale {
    dpi: f64,
    scale_factor: f64,
}

impl Default for DpiScale {
    fn default() -> Self {
        Self {
            dpi: BASELINE_DPI,
            scale_factor: 1.0,
        }
    }
}

impl DpiScale {
    /// Creates a new `DpiScale` from a raw Windows DPI value (e.g., 96, 120, 144, 192).
    #[must_use]
    pub fn from_dpi_value(dpi: u32) -> Self {
        let dpi_float = f64::from(dpi);
        let scale = if dpi_float > 0.0 {
            dpi_float / BASELINE_DPI
        } else {
            1.0
        };
        Self {
            dpi: dpi_float,
            scale_factor: scale,
        }
    }

    /// Creates a new `DpiScale` from a scale factor multiplier (e.g. 1.0, 1.25, 1.5, 2.0).
    #[must_use]
    pub fn from_scale_factor(scale: f64) -> Self {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        Self {
            dpi: scale * BASELINE_DPI,
            scale_factor: scale,
        }
    }

    /// Returns the raw DPI value.
    #[must_use]
    pub const fn dpi(&self) -> f64 {
        self.dpi
    }

    /// Returns the scale factor multiplier.
    #[must_use]
    pub const fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Converts physical pixels to logical (device-independent) pixels.
    #[must_use]
    pub fn physical_to_logical(&self, physical: f64) -> f64 {
        physical / self.scale_factor
    }

    /// Converts logical (device-independent) pixels to physical pixels.
    #[must_use]
    pub fn logical_to_physical(&self, logical: f64) -> f64 {
        logical * self.scale_factor
    }

    /// Converts a physical pixel dimension to an integer logical dimension with rounding.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn physical_to_logical_u32(&self, physical: u32) -> u32 {
        (f64::from(physical) / self.scale_factor).round() as u32
    }

    /// Converts a logical pixel dimension to an integer physical dimension with rounding.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn logical_to_physical_u32(&self, logical: u32) -> u32 {
        (f64::from(logical) * self.scale_factor).round() as u32
    }
}

/// Bounding rectangle and DPI metadata for a connected monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorBounds {
    /// Horizontal virtual desktop origin in physical pixels.
    pub x: i32,
    /// Vertical virtual desktop origin in physical pixels.
    pub y: i32,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Native DPI of this monitor.
    pub dpi: u32,
    /// Whether this is the primary system display.
    pub is_primary: bool,
}

impl MonitorBounds {
    /// Returns the `DpiScale` for this monitor.
    #[must_use]
    pub fn dpi_scale(&self) -> DpiScale {
        DpiScale::from_dpi_value(self.dpi)
    }

    /// Returns width in device-independent logical pixels.
    #[must_use]
    pub fn logical_width(&self) -> u32 {
        self.dpi_scale().physical_to_logical_u32(self.width)
    }

    /// Returns height in device-independent logical pixels.
    #[must_use]
    pub fn logical_height(&self) -> u32 {
        self.dpi_scale().physical_to_logical_u32(self.height)
    }
}
