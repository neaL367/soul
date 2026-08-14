//! Windows 11 sandboxing subsystem providing Job Object limits and restricted security tokens.

#![allow(unsafe_code)]

pub mod error;
pub mod job_object;
pub mod profile;
pub mod restricted_token;

pub use error::SandboxError;
pub use job_object::JobObject;
pub use profile::{SandboxKind, SandboxProfile};
pub use restricted_token::RestrictedToken;
