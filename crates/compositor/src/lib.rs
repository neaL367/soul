//! Compositor subsystem providing layer composition, damage tracking, and GPU presentation.

pub mod damage;
pub mod error;
pub mod layer;

pub use damage::DamageTracker;
pub use error::CompositorError;
pub use layer::CompositorLayer;
