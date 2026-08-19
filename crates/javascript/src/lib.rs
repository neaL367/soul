//! JavaScript engine embedding, event loop, and runtime management.

pub mod error;
pub mod job_executor;
pub mod runtime;

pub use error::JsError;
pub use job_executor::{BoundedJobExecutor, MAX_JOBS_PER_DRAIN};
pub use runtime::{JsRuntime, JsTask, MAX_SCRIPT_BYTES};
