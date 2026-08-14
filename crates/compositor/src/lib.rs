//! Compositor subsystem providing layer composition, damage tracking, and GPU presentation.

pub mod damage;
pub mod error;
pub mod gpu_compositor;
pub mod layer;

pub use damage::DamageTracker;
pub use error::CompositorError;
pub use gpu_compositor::GpuCompositor;
pub use layer::CompositorLayer;
