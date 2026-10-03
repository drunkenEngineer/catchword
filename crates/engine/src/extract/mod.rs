//! Text extraction in a separate worker process.
//!
//! Untrusted files are never parsed inside the engine. A worker program reads
//! one file and sends its text back; the engine runs it under limits and
//! treats its answer as untrusted (ADR-16).
//!
//! On Windows the worker's memory, CPU time and child processes are limited
//! by a job object. On Linux and macOS only the timeout and the output checks
//! apply for now; worker sandboxing there is planned for 1.0.

pub mod protocol;

#[cfg(windows)]
mod job_windows;

use std::fs;
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::without_controls;
#[cfg(windows)]
use job_windows::Containment;
use protocol::{ProtocolError, Refusal, Request, Response};

/// Limits for extracting one file. The defaults suit an 8 GB laptop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Larger files are skipped without starting the worker.
    pub max_file_bytes: u64,
    pub max_pages: u32,
    /// The most text one file may produce, in bytes.
    pub max_text_bytes: u32,
    /// Wall-clock time before the worker is stopped.
    pub timeout: Duration,
    /// Memory the worker may commit. Enforced on Windows only, for now.
    pub memory_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 200 << 20,
            max_pages: 5_000,
            max_text_bytes: 64 << 20,
            timeout: Duration::from_secs(60),
            memory_bytes: 512 << 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The text of each page, in order: the first entry is page 1.
    Pages(Vec<String>),
    NotIndexed(Reason),
}

/// Why a file has no text in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NeedsOcr,
    Encrypted,
    TooLarge,
    CannotOpen,
    Damaged,
    TimedOut,
    MemoryLimit,
    Crashed,
    InvalidOutput,
    LibraryMissing,
}

impl Reason {
    /// A plain explanation for the user.
    pub fn describe(self) -> &'static str {
        match self {
            Reason::NeedsOcr => {
                "no text layer (a scan?); needs text recognition, not available yet"
            }
            Reason::Encrypted => "protected by a password",
            Reason::TooLarge => "over the size, page or text limit",
            Reason::CannotOpen => "could not be opened (access denied, or in use?)",
            Reason::Damaged => "not a readable PDF (damaged?)",
            Reason::TimedOut => "took too long to read and was stopped",
            Reason::MemoryLimit => "needed too much memory to read and was stopped",
            Reason::Crashed => "the reader crashed on this file",
            Reason::InvalidOutput => "the reader returned invalid data",
            Reason::LibraryMissing => "PDF support is not installed (PDFium library not found)",
        }
    }

    /// True when reading was tried and went wrong; false when the file was
    /// skipped by a rule, such as a size limit or a password.
    pub fn is_failure(self) -> bool {
        matches!(
            self,
            Reason::Damaged
                | Reason::TimedOut
                | Reason::MemoryLimit
                | Reason::Crashed
                | Reason::InvalidOutput
                | Reason::LibraryMissing
        )
    }
}

/// What the operating system reported about a finished worker.
#[derive(Debug, Default, Clone, Copy)]
struct Events {
    memory_limit: bool,
    cpu_time_limit: bool,
}

/// Extract the text of `file` with the worker program at `worker`.
///
/// Every problem with the file becomes an `Outcome`. An error means the
/// worker could not be started or put under its limits; then no file was
/// handed to it.
pub fn run(worker: &Path, file: &Path, limits: &Limits) -> io::Result<Outcome> {
    match fs::metadata(file) {
        Ok(meta) if meta.len() > limits.max_file_bytes => {
            return Ok(Outcome::NotIndexed(Reason::TooLarge))
        }
        Ok(_) => {}
        Err(_) => return Ok(Outcome::NotIndexed(Reason::CannotOpen)),
    }

    let deadline = Instant::now() + limits.timeout;
    let containment = Containment::new(limits)?;
    let mut command = Command::new(worker);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;

    // The worker starts before it can be contained, but it reads nothing
    // until it gets the request, which is sent only once the limits apply.
    if let Err(error) = containment.contain(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    let request = Request {
        path: file.to_path_buf(),
        max_pages: limits.max_pages,
        max_text_bytes: limits.max_text_bytes,
    };
    if let Some(mut stdin) = child.stdin.take() {
        // A worker that already died closed this pipe; that is judged below.
        let _ = protocol::write_request(&mut stdin, &request);
    }

    // Read the answer on another thread, so the wait for it can time out.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let (max_bytes, max_pages) = (frame_limit(limits), limits.max_pages);
    let (sender, receiver) = mpsc::channel();
    // Not joined: the thread ends when the pipe closes, which the kill below
    // causes on Windows. Waiting for it could hang if something else held it.
    thread::spawn(move || {
        let _ = sender.send(protocol::read_response(&mut stdout, max_bytes, max_pages));
    });

    let answer = match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(answer) => Some(answer),
        Err(RecvTimeoutError::Timeout) => None,
        Err(RecvTimeoutError::Disconnected) => Some(Err(ProtocolError::Truncated)),
    };
    let timed_out = match &answer {
        None => true,
        // A good answer: the worker still has until the deadline to exit.
        Some(Ok(_)) => !exits_by(&mut child, deadline)?,
        Some(Err(_)) => false,
    };
    if timed_out || !matches!(answer, Some(Ok(_))) {
        containment.kill(&mut child);
    }
    let exit = child.wait()?;
    let events = containment.events();
    Ok(judge(answer, exit, events))
}

/// The largest answer the worker may send: all text, plus 4 bytes per page,
/// plus room for the header.
fn frame_limit(limits: &Limits) -> u32 {
    limits
        .max_text_bytes
        .saturating_add(limits.max_pages.saturating_mul(4))
        .saturating_add(64)
}

/// Wait for the worker to exit, but not past `deadline`. True if it exited.
fn exits_by(child: &mut Child, deadline: Instant) -> io::Result<bool> {
    loop {
        if child.try_wait()?.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Decide what happened. `answer` is None when the time ran out.
fn judge(
    answer: Option<Result<Response, ProtocolError>>,
    exit: ExitStatus,
    events: Events,
) -> Outcome {
    if events.memory_limit {
        return Outcome::NotIndexed(Reason::MemoryLimit);
    }
    if events.cpu_time_limit {
        return Outcome::NotIndexed(Reason::TimedOut);
    }
    let reason = match answer {
        None => Reason::TimedOut,
        Some(Ok(response)) if exit.success() => return accept(response),
        Some(Ok(_)) => Reason::Crashed,
        // The answer stopped early: the worker died, or quit without answering.
        Some(Err(ProtocolError::Truncated | ProtocolError::Io(_))) if !exit.success() => {
            Reason::Crashed
        }
        Some(Err(_)) => Reason::InvalidOutput,
    };
    Outcome::NotIndexed(reason)
}

fn accept(response: Response) -> Outcome {
    match response {
        Response::Pages(pages) => {
            let pages: Vec<String> = pages.iter().map(|page| without_controls(page)).collect();
            if pages.iter().all(|page| page.trim().is_empty()) {
                Outcome::NotIndexed(Reason::NeedsOcr)
            } else {
                Outcome::Pages(pages)
            }
        }
        Response::Refused(refusal) => Outcome::NotIndexed(match refusal {
            Refusal::Encrypted => Reason::Encrypted,
            Refusal::TooLarge => Reason::TooLarge,
            Refusal::Damaged => Reason::Damaged,
            Refusal::CannotOpen => Reason::CannotOpen,
            Refusal::LibraryMissing => Reason::LibraryMissing,
        }),
    }
}

/// Outside Windows there is no job object yet: only the timeout and the
/// output checks apply.
#[cfg(not(windows))]
struct Containment;

#[cfg(not(windows))]
impl Containment {
    fn new(_limits: &Limits) -> io::Result<Self> {
        Ok(Self)
    }

    fn contain(&self, _child: &Child) -> io::Result<()> {
        Ok(())
    }

    fn kill(&self, child: &mut Child) {
        let _ = child.kill();
    }

    fn events(&self) -> Events {
        Events::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_are_removed_from_page_text() {
        let response = Response::Pages(vec![
            "plain\ttext\r\nnext line".to_string(),
            "\u{1b}[2Jerased\u{0}\u{2}screen".to_string(),
        ]);
        assert_eq!(
            accept(response),
            Outcome::Pages(vec![
                "plain\ttext\r\nnext line".to_string(),
                "[2Jerasedscreen".to_string(),
            ])
        );
    }

    #[test]
    fn pages_with_only_control_characters_need_ocr() {
        let response = Response::Pages(vec!["\u{0}\u{7}".to_string(), " \n".to_string()]);
        assert_eq!(accept(response), Outcome::NotIndexed(Reason::NeedsOcr));
    }
}
