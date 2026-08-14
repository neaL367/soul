//! Child process launcher enforcing Windows Job Object boundaries and sandboxing profiles.

use crate::error::SandboxError;
use crate::profile::SandboxProfile;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command};
use windows::Win32::Foundation::HANDLE;

/// Spawns and encloses isolated child processes within sandboxed Job Objects.
pub struct ProcessLauncher;

impl ProcessLauncher {
    /// Spawns a child process and automatically confines it within a configured `JobObject`.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError` if process spawning or Job Object assignment fails.
    pub fn spawn_sandboxed(
        executable: &Path,
        args: &[&str],
        profile: &SandboxProfile,
    ) -> Result<Child, SandboxError> {
        let job = profile.build_job()?;

        let child = Command::new(executable).args(args).spawn()?;

        let raw_handle = child.as_raw_handle();
        job.assign_process(HANDLE(raw_handle.cast()))?;

        Ok(child)
    }
}
