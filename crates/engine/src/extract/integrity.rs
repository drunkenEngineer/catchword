//! Low integrity for the extraction worker (SEC-6, threat T1).
//!
//! Windows labels every process with an integrity level. A process at low
//! integrity can still read the user's files, but cannot write to them, to
//! the registry, or into the user's other programs: Windows refuses writes
//! to anything at a higher level. A process may lower its own level, never
//! raise it. The worker lowers itself before it reads the request, so
//! whatever a hostile file might make it do runs without the right to change
//! anything the user owns. It adds to the job object's limits (ADR-16).
//!
//! These are calls into the Windows API, which Rust cannot check, so each
//! `unsafe` block says why it is sound.

use std::mem::{size_of, size_of_val};
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    CreateWellKnownSid, GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation,
    SetTokenInformation, TokenIntegrityLevel, WinLowLabelSid, SECURITY_MAX_SID_SIZE,
    SID_AND_ATTRIBUTES, TOKEN_ACCESS_MASK, TOKEN_ADJUST_DEFAULT, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY,
};
use windows_sys::Win32::System::SystemServices::{SECURITY_MANDATORY_LOW_RID, SE_GROUP_INTEGRITY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The level of a low-integrity process. An ordinary program runs at
/// medium, 8192.
pub const LOW: u32 = SECURITY_MANDATORY_LOW_RID as u32;

/// Lower this process to low integrity, for good. True if it worked.
pub fn lower_this_process() -> bool {
    // SAFETY: the pseudo-handle of the current process needs no closing.
    let process = unsafe { GetCurrentProcess() };
    let Some(token) = Token::open(process, TOKEN_ADJUST_DEFAULT | TOKEN_QUERY) else {
        return false;
    };
    let mut sid = [0u8; SECURITY_MAX_SID_SIZE as usize];
    let mut size = SECURITY_MAX_SID_SIZE;
    // SAFETY: the buffer holds SECURITY_MAX_SID_SIZE bytes, as `size` says.
    let made = unsafe {
        CreateWellKnownSid(
            WinLowLabelSid,
            ptr::null_mut(),
            sid.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if made == 0 {
        return false;
    }
    let label = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid.as_mut_ptr().cast(),
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    // SAFETY: `label` points to the SID above, which outlives the call, and
    // the length counts the label and the SID.
    let set = unsafe {
        SetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            (&label as *const TOKEN_MANDATORY_LABEL).cast(),
            size_of::<TOKEN_MANDATORY_LABEL>() as u32 + size,
        )
    };
    set != 0
}

/// The integrity level of this process, if it can be read.
pub fn of_this_process() -> Option<u32> {
    // SAFETY: the pseudo-handle of the current process needs no closing.
    level(unsafe { GetCurrentProcess() })
}

/// The integrity level of a process this one started, if it can be read.
pub fn of_child(child: &Child) -> Option<u32> {
    level(child.as_raw_handle())
}

fn level(process: HANDLE) -> Option<u32> {
    let token = Token::open(process, TOKEN_QUERY)?;
    // Room for the label and its SID, aligned for the label.
    let mut buffer = [0u64; 16];
    let mut needed = 0u32;
    // SAFETY: the buffer is as long as passed, and aligned for the label.
    let read = unsafe {
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
            &mut needed,
        )
    };
    if read == 0 {
        return None;
    }
    // SAFETY: Windows wrote a TOKEN_MANDATORY_LABEL at the start of the
    // buffer, whose SID lies in the buffer too; a label SID has one
    // sub-authority or more, the last being the level.
    unsafe {
        let label = &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        Some(*GetSidSubAuthority(
            label.Label.Sid,
            u32::from(count).checked_sub(1)?,
        ))
    }
}

/// A process's access token, closed when dropped.
struct Token(HANDLE);

impl Token {
    fn open(process: HANDLE, access: TOKEN_ACCESS_MASK) -> Option<Token> {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: `process` is a valid process handle; the out-pointer is valid.
        let opened = unsafe { OpenProcessToken(process, access, &mut token) };
        (opened != 0).then_some(Token(token))
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle was opened above and is closed only here.
        unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_test_runs_above_low_integrity() {
        // Lowering the test process itself would stop the other tests from
        // writing their files; the worker's own test checks the lowering.
        assert!(of_this_process().unwrap() > LOW);
    }
}
