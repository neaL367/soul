//! Windows Job Object resource isolation, memory limits, and UI lockdown.

use crate::error::SandboxError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOB_OBJECT_UILIMIT_DESKTOP, JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
    JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES, JOB_OBJECT_UILIMIT_READCLIPBOARD,
    JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS, JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};

/// Safe RAII wrapper around a Windows Win32 Job Object handle.
#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

// Windows HANDLEs for Job Objects can be safely sent across threads.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    /// Creates a new unnamed Windows Job Object.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if Job Object creation fails.
    pub fn create() -> Result<Self, SandboxError> {
        let handle = unsafe { CreateJobObjectW(None, None)? };
        Ok(Self { handle })
    }

    /// Sets process memory limit (in bytes) and ensures child processes terminate when the Job closes.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if configuration fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_memory_limit(&self, max_bytes: usize) -> Result<(), SandboxError> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.ProcessMemoryLimit = max_bytes;
        info.JobMemoryLimit = max_bytes;

        unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;
        }
        Ok(())
    }

    /// Enforces strict UI restrictions (locks clipboard, prevents desktop/display modification).
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if configuration fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_ui_restrictions(&self) -> Result<(), SandboxError> {
        let ui_info = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_HANDLES
                | JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
                | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
                | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                | JOB_OBJECT_UILIMIT_GLOBALATOMS
                | JOB_OBJECT_UILIMIT_DESKTOP
                | JOB_OBJECT_UILIMIT_EXITWINDOWS,
        };

        unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectBasicUIRestrictions,
                std::ptr::from_ref(&ui_info).cast(),
                size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
            )?;
        }
        Ok(())
    }

    /// Assigns a running process to this Job Object.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if assignment fails.
    pub fn assign_process(&self, process_handle: HANDLE) -> Result<(), SandboxError> {
        unsafe {
            AssignProcessToJobObject(self.handle, process_handle)?;
        }
        Ok(())
    }

    /// Terminates all processes inside the Job Object immediately.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if termination fails.
    pub fn terminate(&self, exit_code: u32) -> Result<(), SandboxError> {
        unsafe {
            TerminateJobObject(self.handle, exit_code)?;
        }
        Ok(())
    }

    /// Returns the raw Win32 `HANDLE`.
    #[must_use]
    pub const fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
