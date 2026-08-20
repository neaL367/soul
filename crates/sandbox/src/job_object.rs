//! Windows Job Object resource isolation, memory limits, and UI lockdown.

use crate::error::SandboxError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE,
    JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_UILIMIT_DESKTOP,
    JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
    JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES, JOB_OBJECT_UILIMIT_READCLIPBOARD,
    JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS, JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_UI_RESTRICTIONS,
    JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectBasicUIRestrictions, JobObjectCpuRateControlInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};

/// Safe RAII wrapper around a Windows Win32 Job Object handle.
#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

// SAFETY: `HANDLE` wraps a Win32 kernel handle with no per-thread state;
// Job Object handles are usable from any thread. `Drop` closes it via
// `CloseHandle`, which is thread-safe and happens exactly once.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    /// Creates a new unnamed Windows Job Object.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if Job Object creation fails.
    pub fn create() -> Result<Self, SandboxError> {
        // SAFETY: `CreateJobObjectW` returns a valid handle on success or a
        // Win32 error that the `windows` crate surfaces via `?`.
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
        let mut info = self.query_extended_limit_info().unwrap_or_default();
        info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.ProcessMemoryLimit = max_bytes;
        info.JobMemoryLimit = max_bytes;

        // SAFETY: `info` is fully initialized and its address is valid for the
        // duration of the call; `size_of` matches the struct Win32 expects and
        // the handle was returned by a successful `CreateJobObjectW`.
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

    /// Sets the maximum number of active processes allowed within the Job Object simultaneously.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if configuration fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_active_process_limit(&self, max_processes: u32) -> Result<(), SandboxError> {
        let mut info = self.query_extended_limit_info().unwrap_or_default();
        info.BasicLimitInformation.LimitFlags |=
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.BasicLimitInformation.ActiveProcessLimit = max_processes;

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

    /// Sets a hard CPU rate cap on the Job Object (1–100 percent).
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::InvalidLimit` if percentage is out of 1..=100 range,
    /// or `SandboxError::Win32` if Win32 configuration fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_cpu_rate_limit(&self, cpu_percent: u32) -> Result<(), SandboxError> {
        if cpu_percent == 0 || cpu_percent > 100 {
            return Err(SandboxError::InvalidLimit(format!(
                "CPU rate limit percent must be 1..=100, got {cpu_percent}"
            )));
        }

        // Rate is in 1/100 of 1% (i.e. 10,000 = 100%).
        let rate_val = cpu_percent * 100;
        let cpu_info = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
            ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: rate_val },
        };

        unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectCpuRateControlInformation,
                std::ptr::from_ref(&cpu_info).cast(),
                size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
            )?;
        }
        Ok(())
    }

    /// Queries the current extended limit information struct.
    #[allow(clippy::cast_possible_truncation)]
    fn query_extended_limit_info(
        &self,
    ) -> Result<JOBOBJECT_EXTENDED_LIMIT_INFORMATION, SandboxError> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let mut return_length: u32 = 0;

        unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                Some(&raw mut return_length),
            )?;
        }
        Ok(info)
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

        // SAFETY: `ui_info` is fully initialized and the handle is valid; the
        // struct size matches the Win32 definition.
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
        // SAFETY: `self.handle` is a live Job Object handle created by
        // `create` and not yet closed; `process_handle` is borrowed from the
        // caller and must remain valid for the duration of the call.
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
        // SAFETY: `self.handle` is a live Job Object handle not yet closed.
        unsafe {
            TerminateJobObject(self.handle, exit_code)?;
        }
        Ok(())
    }

    /// Queries accounting and resource utilization metrics for this Job Object.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::Win32` if query fails.
    #[allow(clippy::cast_possible_truncation)]
    pub fn query_accounting(&self) -> Result<JobAccounting, SandboxError> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let mut return_length: u32 = 0;

        // SAFETY: `info` and `return_length` are sized out-parameters with the
        // exact structure size Win32 writes; the handle is live.
        unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut info).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                Some(&raw mut return_length),
            )?;
        }

        let ext = self.query_extended_limit_info().unwrap_or_default();

        Ok(JobAccounting {
            active_processes: info.ActiveProcesses,
            total_processes: info.TotalProcesses,
            total_terminated_processes: info.TotalTerminatedProcesses,
            peak_process_memory_bytes: ext.PeakProcessMemoryUsed,
            peak_job_memory_bytes: ext.PeakJobMemoryUsed,
            total_user_time_100ns: info.TotalUserTime,
            total_kernel_time_100ns: info.TotalKernelTime,
        })
    }

    /// Returns the raw Win32 `HANDLE` for use with Win32 APIs.
    ///
    /// The handle is **borrowed** from this `JobObject`: callers must not
    /// close it (it is closed by `Drop`), and must not use it after the
    /// `JobObject` is dropped.
    #[must_use]
    pub const fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

/// Process count and resource statistics for a running Windows Job Object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JobAccounting {
    /// Total number of processes currently active in the job.
    pub active_processes: u32,
    /// Total number of processes that have ever been assigned to the job.
    pub total_processes: u32,
    /// Total number of processes that have terminated in the job.
    pub total_terminated_processes: u32,
    /// Peak memory used by any single process in the job (bytes).
    pub peak_process_memory_bytes: usize,
    /// Peak total memory used by all processes in the job simultaneously (bytes).
    pub peak_job_memory_bytes: usize,
    /// Total CPU time spent in user mode across all processes (in 100ns intervals).
    pub total_user_time_100ns: i64,
    /// Total CPU time spent in kernel mode across all processes (in 100ns intervals).
    pub total_kernel_time_100ns: i64,
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            // SAFETY: the handle is a valid, non-null Job Object handle that
            // has not been closed before; `CloseHandle` runs exactly once per
            // object because `Drop` runs once.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
