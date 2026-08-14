//! Sandboxing and process isolation using Windows Job Objects and Restricted Tokens.

#![allow(unsafe_code)]

pub mod error;
pub mod job_object;
pub mod launcher;
pub mod profile;
pub mod restricted_token;

pub use error::SandboxError;
pub use job_object::JobObject;
pub use launcher::{ProcessLauncher, SandboxedChild};
pub use profile::{SandboxKind, SandboxProfile};
pub use restricted_token::RestrictedToken;
