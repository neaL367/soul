//! Child process launcher enforcing Windows Job Object boundaries and sandboxing profiles.

use crate::error::SandboxError;
use crate::job_object::JobObject;
use crate::profile::SandboxProfile;
use crate::restricted_token::RestrictedToken;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Security::{DISABLE_MAX_PRIVILEGE, LUA_TOKEN};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, GetExitCodeProcess, OpenThread, PROCESS_CREATION_FLAGS,
    PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, THREAD_SUSPEND_RESUME, TerminateProcess,
    WaitForSingleObject,
};

/// Win32 `CREATE_SUSPENDED` process creation flag.
const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Underlying representation of a spawned sandboxed child process.
enum ProcessInner {
    Std(Child),
    Win32 { handle: HANDLE, pid: u32 },
}

// SAFETY: Both `Child` and Win32 `HANDLE` are movable across threads;
// `Drop` safely closes Win32 handles.
unsafe impl Send for ProcessInner {}
unsafe impl Sync for ProcessInner {}

/// Running sandboxed child process bundled with its enclosing Windows `JobObject`.
pub struct SandboxedChild {
    inner: ProcessInner,
    job: JobObject,
}

impl SandboxedChild {
    /// Creates a new `SandboxedChild` wrapping the standard child process and its job object.
    #[must_use]
    pub const fn new(child: Child, job: JobObject) -> Self {
        Self {
            inner: ProcessInner::Std(child),
            job,
        }
    }

    /// Creates a new `SandboxedChild` wrapping a Win32 process handle and its job object.
    #[must_use]
    pub const fn from_raw_handle(handle: HANDLE, pid: u32, job: JobObject) -> Self {
        Self {
            inner: ProcessInner::Win32 { handle, pid },
            job,
        }
    }

    /// Returns the OS process identifier (PID) of the sandboxed child.
    #[must_use]
    pub fn pid(&self) -> u32 {
        match &self.inner {
            ProcessInner::Std(child) => child.id(),
            ProcessInner::Win32 { pid, .. } => *pid,
        }
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
        match &mut self.inner {
            ProcessInner::Std(child) => child.wait(),
            ProcessInner::Win32 { handle, .. } => {
                let wait_res = unsafe { WaitForSingleObject(*handle, u32::MAX) };
                if wait_res == WAIT_OBJECT_0 {
                    let mut exit_code: u32 = 0;
                    let code_res = unsafe { GetExitCodeProcess(*handle, &raw mut exit_code) };
                    if code_res.is_ok() {
                        Ok(ExitStatusExt::from_raw(exit_code))
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                } else {
                    Err(std::io::Error::last_os_error())
                }
            }
        }
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

impl Drop for SandboxedChild {
    fn drop(&mut self) {
        if let ProcessInner::Win32 { handle, .. } = self.inner
            && !handle.is_invalid()
        {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
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
    // SAFETY: `CreateToolhelp32Snapshot` returns a valid snapshot handle on
    // success or a Win32 error surfaced by `?`.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }?;

    let mut entry = THREADENTRY32 {
        // `dwSize` is a fixed-size struct; the cast cannot truncate in practice.
        #[allow(clippy::cast_possible_truncation)]
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..THREADENTRY32::default()
    };

    let mut primary_tid = None;
    // SAFETY: `entry.dwSize` is set to the correct struct size and the
    // enumeration operates on the valid snapshot handle created above;
    // `Thread32First`/`Thread32Next` only write `entry` in bounds. The
    // snapshot is closed exactly once after the loop, which always
    // terminates because `Thread32Next` failure breaks the loop.
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
    // SAFETY: `thread_handle` is a valid handle from the checked
    // `OpenThread` call; it is closed exactly once after the resume.
    unsafe {
        let _ = ResumeThread(thread_handle);
        let _ = CloseHandle(thread_handle);
    }
    Ok(())
}

impl ProcessLauncher {
    /// Spawns a child process and automatically confines it within a configured `JobObject`.
    ///
    /// If `profile.use_restricted_token` is enabled, creates a restricted security token
    /// and uses `CreateProcessAsUserW`. Otherwise, uses standard process creation.
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
        if profile.use_restricted_token {
            let token = RestrictedToken::create_with_flags(
                DISABLE_MAX_PRIVILEGE | LUA_TOKEN,
                profile.low_integrity,
            )?;
            Self::spawn_with_restricted_token(executable, args, profile, &token)
        } else {
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

    /// Spawns a child process using an explicit `RestrictedToken` via Win32 `CreateProcessAsUserW`.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError` if token process spawning, job assignment, or thread resumption fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn spawn_with_restricted_token(
        executable: &Path,
        args: &[&str],
        profile: &SandboxProfile,
        token: &RestrictedToken,
    ) -> Result<SandboxedChild, SandboxError> {
        let job = profile.build_job()?;

        // Build command line string for CreateProcessAsUserW.
        let mut full_cmd_str = format!("\"{}\"", executable.display());
        for arg in args {
            full_cmd_str.push(' ');
            if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
                full_cmd_str.push('"');
                full_cmd_str.push_str(&arg.replace('"', "\\\""));
                full_cmd_str.push('"');
            } else {
                full_cmd_str.push_str(arg);
            }
        }

        let mut wide_cmd: Vec<u16> = full_cmd_str.encode_utf16().chain(Some(0)).collect();

        let startup_info = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut process_info = PROCESS_INFORMATION::default();

        unsafe {
            CreateProcessAsUserW(
                token.raw_handle(),
                windows::core::PCWSTR::null(),
                windows::core::PWSTR(wide_cmd.as_mut_ptr()),
                None,
                None,
                false,
                PROCESS_CREATION_FLAGS(CREATE_SUSPENDED),
                None,
                windows::core::PCWSTR::null(),
                &raw const startup_info,
                &raw mut process_info,
            )?;
        }

        let proc_handle = process_info.hProcess;
        let thread_handle = process_info.hThread;
        let pid = process_info.dwProcessId;

        // Fail-closed: if Job assignment fails, terminate process immediately and close handles.
        if let Err(err) = job.assign_process(proc_handle) {
            unsafe {
                let _ = TerminateProcess(proc_handle, 1);
                let _ = CloseHandle(proc_handle);
                let _ = CloseHandle(thread_handle);
            }
            return Err(err);
        }

        // Resume primary thread.
        let resume_res = unsafe { ResumeThread(thread_handle) };
        unsafe {
            let _ = CloseHandle(thread_handle);
        }

        if resume_res == u32::MAX {
            unsafe {
                let _ = TerminateProcess(proc_handle, 1);
                let _ = CloseHandle(proc_handle);
            }
            return Err(SandboxError::InvalidHandle(format!(
                "failed to resume thread for child PID {pid}"
            )));
        }

        Ok(SandboxedChild::from_raw_handle(proc_handle, pid, job))
    }
}
