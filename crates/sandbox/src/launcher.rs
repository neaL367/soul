//! Child process launcher enforcing Windows Job Object boundaries and sandboxing profiles.

use crate::error::SandboxError;
use crate::job_object::JobObject;
use crate::profile::SandboxProfile;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// Win32 `CREATE_SUSPENDED` process creation flag.
const CREATE_SUSPENDED: u32 = 0x0000_0004;

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

/// Resumes the first thread owned by the given process (the process's primary
/// thread after `CREATE_SUSPENDED`).
///
/// # Errors
///
/// Returns `SandboxError` if the process's threads cannot be enumerated or its
/// primary thread cannot be resumed.
fn resume_primary_thread(pid: u32) -> Result<(), SandboxError> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }?;

    let mut entry = THREADENTRY32 {
        // `dwSize` is a fixed-size struct; the cast cannot truncate in practice.
        #[allow(clippy::cast_possible_truncation)]
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..THREADENTRY32::default()
    };

    let mut primary_tid = None;
    unsafe {
        if Thread32First(snapshot, &raw mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    primary_tid = Some(entry.th32ThreadID);
                    break;
                }
                if Thread32Next(snapshot, &raw mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    let Some(tid) = primary_tid else {
        return Err(SandboxError::InvalidHandle(format!(
            "no primary thread found for pid {pid}"
        )));
    };

    // SAFETY: `tid` was obtained from a thread enumeration of this process
    // family; the handle is used only for the resume call and closed below.
    let thread_handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, tid) }?;
    unsafe {
        let _ = ResumeThread(thread_handle);
        let _ = CloseHandle(thread_handle);
    }
    Ok(())
}

impl ProcessLauncher {
    /// Spawns a child process and automatically confines it within a configured `JobObject`.
    ///
    /// The child is created with `CREATE_SUSPENDED`, assigned to the Job
    /// Object, and only then resumed, so it can never execute code (or spawn
    /// descendants) outside the sandbox. If any step after spawning fails, the
    /// child is killed before the error is returned — the sandbox fails
    /// closed, never open.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError` if process spawning, Job Object assignment, or
    /// thread resumption fails.
    pub fn spawn_sandboxed(
        executable: &Path,
        args: &[&str],
        profile: &SandboxProfile,
    ) -> Result<SandboxedChild, SandboxError> {
        let job = profile.build_job()?;

        let mut command = Command::new(executable);
        command.args(args).creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        // Fail closed: never return an error while the child is still running
        // outside the Job Object.
        let assign = job.assign_process(HANDLE(child.as_raw_handle().cast()));
        if let Err(err) = assign {
            let _ = child.kill();
            return Err(err);
        }

        if let Err(err) = resume_primary_thread(child.id()) {
            let _ = child.kill();
            return Err(err);
        }

        Ok(SandboxedChild::new(child, job))
    }
}
