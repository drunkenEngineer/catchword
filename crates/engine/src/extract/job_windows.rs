//! Windows job objects: hard limits on the extraction worker (ADR-16).
//!
//! The worker runs inside a job that
//! - caps the memory it can commit, so a hostile file cannot exhaust the machine;
//! - allows exactly one process, so the worker cannot start other programs;
//! - kills the worker when the job is closed, which also happens when the
//!   engine exits or crashes;
//! - ends the worker at once on a crash, instead of waiting on an error dialog;
//! - caps its CPU time, as a backstop to the engine's wall-clock timeout.
//!
//! Windows reports limit violations through an I/O completion port. That is
//! how a worker stopped for using too much memory is told apart from an
//! ordinary crash. Windows does not promise to deliver every report, so a
//! missed one makes a memory stop look like a crash, never the reverse.
//!
//! These are calls into the Windows API, which Rust cannot check, so each
//! `unsafe` block says why it is sound.

use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectAssociateCompletionPortInformation,
    JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
    JOBOBJECTINFOCLASS, JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOB_OBJECT_LIMIT_PROCESS_TIME,
};
use windows_sys::Win32::System::SystemServices::{
    JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO, JOB_OBJECT_MSG_END_OF_PROCESS_TIME,
    JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT,
};
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED,
};

use super::{Events, Limits};

/// How long to wait for each report once the worker has exited.
const REPORT_WAIT_MS: u32 = 1000;

pub(super) struct Containment {
    job: HANDLE,
    port: HANDLE,
}

impl Containment {
    pub(super) fn new(limits: &Limits) -> io::Result<Self> {
        // SAFETY: null attributes and name are allowed; the result is checked.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: INVALID_HANDLE_VALUE with no existing port asks for a new
        // port, as documented; the result is checked.
        let port = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, ptr::null_mut(), 0, 1) };
        if port.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: `job` was opened above and is closed exactly once.
            unsafe { CloseHandle(job) };
            return Err(error);
        }
        // From here on, Drop closes both handles.
        let containment = Self { job, port };

        let reports = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
            CompletionKey: ptr::null_mut(),
            CompletionPort: port,
        };
        containment.set(JobObjectAssociateCompletionPortInformation, &reports)?;

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let basic = &mut info.BasicLimitInformation;
        basic.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION
            | JOB_OBJECT_LIMIT_PROCESS_TIME;
        basic.ActiveProcessLimit = 1;
        // Windows counts CPU time in units of 100 nanoseconds.
        basic.PerProcessUserTimeLimit =
            i64::try_from(limits.timeout.as_nanos() / 100).unwrap_or(i64::MAX);
        info.ProcessMemoryLimit = usize::try_from(limits.memory_bytes).unwrap_or(usize::MAX);
        containment.set(JobObjectExtendedLimitInformation, &info)?;
        Ok(containment)
    }

    /// Pass one settings struct to the job. `T` must be the struct `class` expects.
    fn set<T>(&self, class: JOBOBJECTINFOCLASS, value: &T) -> io::Result<()> {
        // SAFETY: `value` is a live `T` of exactly the size passed, and both
        // callers pair each class with its documented struct.
        let ok = unsafe {
            SetInformationJobObject(
                self.job,
                class,
                (value as *const T).cast(),
                size_of::<T>() as u32,
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Put the worker under the limits. Until this succeeds it must not be
    /// given a file.
    pub(super) fn contain(&self, child: &Child) -> io::Result<()> {
        // SAFETY: the job handle is open (owned by self) and the process
        // handle is open (owned by `child`, which outlives this call).
        let ok = unsafe { AssignProcessToJobObject(self.job, child.as_raw_handle() as HANDLE) };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn kill(&self, child: &mut Child) {
        // SAFETY: the job handle is open. Failure means nothing was running.
        unsafe { TerminateJobObject(self.job, 1) };
        let _ = child.kill();
    }

    /// Read the job's reports. Call only after the worker has exited.
    pub(super) fn events(&self) -> Events {
        let mut events = Events::default();
        loop {
            let mut message = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut OVERLAPPED = ptr::null_mut();
            // SAFETY: the port is open and every out-pointer is a valid local.
            let ok = unsafe {
                GetQueuedCompletionStatus(
                    self.port,
                    &mut message,
                    &mut key,
                    &mut overlapped,
                    REPORT_WAIT_MS,
                )
            };
            if ok == 0 {
                break;
            }
            match message {
                JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT => events.memory_limit = true,
                JOB_OBJECT_MSG_END_OF_PROCESS_TIME => events.cpu_time_limit = true,
                // The last report: no process is left in the job.
                JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO => break,
                _ => {}
            }
        }
        events
    }
}

impl Drop for Containment {
    fn drop(&mut self) {
        // SAFETY: both handles were opened in `new` and are closed only here.
        // Closing the job kills anything still inside it.
        unsafe {
            CloseHandle(self.job);
            CloseHandle(self.port);
        }
    }
}
