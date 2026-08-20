//! Sandbox profile configuration and role-based policy assignment.

use crate::error::SandboxError;
use crate::job_object::JobObject;

/// Specific sandbox policy profiles tailored to each out-of-process browser subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxKind {
    /// Highly restricted renderer process (untrusted web content).
    Renderer,
    /// Network process handling socket I/O and TLS handshakes.
    Network,
    /// GPU compositing and hardware rasterization process.
    Gpu,
    /// Auxiliary utility tasks.
    Utility,
}

/// Builder configuring sandbox resource quotas and security constraints.
#[derive(Debug, Clone)]
pub struct SandboxProfile {
    /// Subsystem role.
    pub kind: SandboxKind,
    /// Maximum allowed memory in bytes.
    pub memory_limit: Option<usize>,
    /// Whether to apply full Win32 UI restrictions.
    pub restrict_ui: bool,
    /// Whether to spawn with a stripped restricted security token.
    pub use_restricted_token: bool,
    /// Whether to enforce mandatory Low Integrity level on the security token.
    pub low_integrity: bool,
    /// Maximum number of active processes allowed within the job simultaneously.
    pub max_active_processes: Option<u32>,
    /// Optional CPU rate limit percentage (1..=100).
    pub cpu_rate_limit_percent: Option<u32>,
}

impl SandboxProfile {
    /// Creates a default sandbox profile for the given subsystem kind.
    #[must_use]
    pub const fn for_kind(kind: SandboxKind) -> Self {
        match kind {
            SandboxKind::Renderer => Self {
                kind,
                memory_limit: Some(1024 * 1024 * 1024), // 1 GB limit for renderers
                restrict_ui: true,
                use_restricted_token: true,
                low_integrity: true,
                max_active_processes: Some(1),
                cpu_rate_limit_percent: Some(80),
            },
            SandboxKind::Network => Self {
                kind,
                memory_limit: Some(512 * 1024 * 1024), // 512 MB limit for network
                restrict_ui: true,
                use_restricted_token: true,
                low_integrity: false,
                max_active_processes: Some(2),
                cpu_rate_limit_percent: None,
            },
            SandboxKind::Gpu => Self {
                kind,
                memory_limit: Some(2048 * 1024 * 1024), // 2 GB limit for GPU
                restrict_ui: false,
                use_restricted_token: false,
                low_integrity: false,
                max_active_processes: Some(2),
                cpu_rate_limit_percent: None,
            },
            SandboxKind::Utility => Self {
                kind,
                memory_limit: Some(256 * 1024 * 1024),
                restrict_ui: true,
                use_restricted_token: false,
                low_integrity: false,
                max_active_processes: Some(1),
                cpu_rate_limit_percent: None,
            },
        }
    }

    /// Spawns and configures a `JobObject` matching this profile's constraints.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError` if Windows Job Object creation or configuration fails.
    pub fn build_job(&self) -> Result<JobObject, SandboxError> {
        let job = JobObject::create()?;

        if let Some(mem_limit) = self.memory_limit {
            job.set_memory_limit(mem_limit)?;
        }

        if let Some(max_proc) = self.max_active_processes {
            job.set_active_process_limit(max_proc)?;
        }

        if let Some(cpu_rate) = self.cpu_rate_limit_percent {
            job.set_cpu_rate_limit(cpu_rate)?;
        }

        if self.restrict_ui {
            job.set_ui_restrictions()?;
        }

        Ok(job)
    }
}
