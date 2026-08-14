//! JavaScript engine embedding, event loop, and runtime management.

pub mod error;
pub mod runtime;

pub use error::JsError;
pub use runtime::{JsRuntime, JsTask};
