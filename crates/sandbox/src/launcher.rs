//! Child process launcher enforcing Windows Job Object boundaries and sandboxing profiles.

use crate::error::SandboxError;
use crate::job_object::JobObject;
use crate::profile::SandboxProfile;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use windows::Win32::Foundation::HANDLE;

/// Running sandboxed child process bundled with its enclosing Windows `JobObject`.
pub struct SandboxedChild {
    child: Child,
    job: JobObject,
}

impl SandboxedChild {
    /// Creates a new `SandboxedChild` wrapping the child process and its job object.
    #[must_use]
    pub const fn new(child: Child, job: JobObject) -> Self {
        Self { child, job }
    }

    /// Returns a reference to the inner `std::process::Child`.
    #[must_use]
    pub const fn child(&self) -> &Child {
        &self.child
    }

    /// Returns a mutable reference to the inner `std::process::Child`.
    pub const fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Returns a reference to the enclosing `JobObject`.
    #[must_use]
    pub const fn job(&self) -> &JobObject {
        &self.job
    }

    /// Waits for the child process to exit completely, returning its exit status.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Error` if waiting on the child fails.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Forces the entire Job Object (and all child processes inside it) to terminate with `exit_code`.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError` if Win32 termination fails.
    pub fn kill_job(&self, exit_code: u32) -> Result<(), SandboxError> {
        self.job.terminate(exit_code)
    }
}

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
    ) -> Result<SandboxedChild, SandboxError> {
        let job = profile.build_job()?;

        let child = Command::new(executable).args(args).spawn()?;

        let raw_handle = child.as_raw_handle();
        job.assign_process(HANDLE(raw_handle.cast()))?;

        Ok(SandboxedChild::new(child, job))
    }
}
