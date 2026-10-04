//! Tests for the worker supervisor, with stand-in workers that misbehave.
//!
//! This test program is its own stand-in: when `CATCHWORD_TEST_ROLE` is set,
//! it acts as a worker in that role instead of running the tests. A role
//! needs full control of its output, so this file has a small runner of its
//! own instead of the standard test harness (`harness = false`).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::panic;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use catchword_engine::extract::protocol::{self, Refusal, Response};
use catchword_engine::extract::{run, run_while, Limits, Outcome, Reason};

const ROLE: &str = "CATCHWORD_TEST_ROLE";

fn main() {
    if let Ok(role) = env::var(ROLE) {
        play(&role);
        return;
    }
    let tests: &[(&str, fn())] = &[
        ("a_well_behaved_worker_returns_its_pages", well_behaved),
        ("refusals_become_reasons", refusals),
        ("pages_without_text_need_ocr", pages_without_text),
        ("a_hanging_worker_is_stopped_at_the_time_limit", hanging),
        ("a_worker_is_stopped_at_once_when_asked", stopped_when_asked),
        ("garbage_output_is_rejected", garbage),
        ("an_oversized_answer_is_rejected_at_once", oversized),
        ("bytes_after_the_answer_are_rejected", trailing),
        ("quitting_without_an_answer_is_invalid", silent_exit),
        ("a_crash_is_recorded", crash),
        ("large_files_are_skipped_without_a_worker", large_file),
        ("a_missing_file_cannot_be_opened", missing_file),
        ("a_missing_worker_program_is_an_error", missing_worker),
        #[cfg(windows)]
        ("a_worker_that_floods_memory_is_stopped", memory_flood),
        #[cfg(windows)]
        ("a_worker_cannot_start_other_programs", child_process),
    ];
    let filter = env::args().skip(1).find(|arg| !arg.starts_with('-'));
    let selected: Vec<_> = tests
        .iter()
        .filter(|(name, _)| filter.as_ref().is_none_or(|f| name.contains(f.as_str())))
        .collect();

    println!("\nrunning {} tests", selected.len());
    let mut failed = Vec::new();
    for (name, test) in &selected {
        let passed = panic::catch_unwind(test).is_ok();
        println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
        if !passed {
            failed.push(*name);
        }
    }
    let status = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {status}. {} passed; {} failed\n",
        selected.len() - failed.len(),
        failed.len()
    );
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

// ---- The stand-in worker ----

fn play(role: &str) {
    // Like the real worker, do nothing until the request arrives: by then the
    // supervisor has applied the limits.
    let request = protocol::read_request(&mut io::stdin().lock()).expect("a request");
    let mut out = io::stdout().lock();
    match role {
        "echo" => answer(&["first page", "second page"]),
        "refuse" => send(&Response::Refused(Refusal::Encrypted)),
        "blank" => answer(&["  ", "\n\t"]),
        "hang" => {
            // Prove it is alive by growing a file until it is stopped.
            let mut heartbeat = OpenOptions::new().append(true).open(&request.path).unwrap();
            loop {
                heartbeat.write_all(b".").unwrap();
                thread::sleep(Duration::from_millis(20));
            }
        }
        "garbage" => out.write_all(b"this is not a message").unwrap(),
        "oversized" => {
            out.write_all(&u32::MAX.to_le_bytes()).unwrap();
            out.flush().unwrap();
            thread::sleep(Duration::from_secs(60));
        }
        "trailing" => {
            protocol::write_response(&mut out, &Response::Refused(Refusal::Damaged)).unwrap();
            loop {
                if out.write_all(&[0; 4096]).is_err() {
                    break;
                }
            }
        }
        "silent" => {}
        "crash" => std::process::abort(),
        "memory" => {
            // Commit 16 MiB at a time. Under the limit this fails long
            // before 2 GiB, and the allocation failure ends the process.
            let mut hoard = Vec::new();
            for _ in 0..128 {
                hoard.push(vec![1u8; 16 << 20]);
            }
            answer(&["no memory limit"]);
        }
        "spawn" => {
            let started = Command::new(env::current_exe().unwrap())
                .env(ROLE, "silent")
                .spawn();
            match started {
                Ok(mut child) => {
                    let _ = child.kill();
                    answer(&["started"]);
                }
                Err(_) => answer(&["blocked"]),
            }
        }
        other => panic!("unknown role {other}"),
    }
}

fn answer(pages: &[&str]) {
    send(&Response::Pages(
        pages.iter().map(|p| p.to_string()).collect(),
    ));
}

fn send(response: &Response) {
    protocol::write_response(&mut io::stdout().lock(), response).unwrap();
}

// ---- Helpers ----

/// A file for the worker to "read", in a folder of this test's own.
fn test_file(name: &str, contents: &[u8]) -> PathBuf {
    let folder = env::temp_dir().join("catchword-supervisor-test");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn short_limits() -> Limits {
    Limits {
        timeout: Duration::from_secs(20),
        ..Limits::default()
    }
}

/// Run the stand-in worker in `role` on a small file.
fn run_role(role: &str, limits: &Limits) -> Outcome {
    run_role_on(role, &test_file(&format!("{role}.txt"), b"content"), limits)
}

fn run_role_on(role: &str, file: &Path, limits: &Limits) -> Outcome {
    // The runner is single-threaded, so setting the variable here is safe.
    env::set_var(ROLE, role);
    let outcome = run(&env::current_exe().unwrap(), file, limits).unwrap();
    env::remove_var(ROLE);
    outcome
}

fn pages(texts: &[&str]) -> Outcome {
    Outcome::Pages(texts.iter().map(|t| t.to_string()).collect())
}

// ---- Tests ----

fn well_behaved() {
    assert_eq!(
        run_role("echo", &short_limits()),
        pages(&["first page", "second page"])
    );
}

fn refusals() {
    assert_eq!(
        run_role("refuse", &short_limits()),
        Outcome::NotIndexed(Reason::Encrypted)
    );
}

fn pages_without_text() {
    assert_eq!(
        run_role("blank", &short_limits()),
        Outcome::NotIndexed(Reason::NeedsOcr)
    );
}

fn hanging() {
    let heartbeat = test_file("heartbeat.txt", b"");
    let limits = Limits {
        timeout: Duration::from_secs(1),
        ..Limits::default()
    };
    let started = Instant::now();
    let outcome = run_role_on("hang", &heartbeat, &limits);
    assert_eq!(outcome, Outcome::NotIndexed(Reason::TimedOut));
    assert!(started.elapsed() < Duration::from_secs(5));

    // The worker was alive, and is now really gone: the file stopped growing.
    let before = fs::metadata(&heartbeat).unwrap().len();
    assert!(before > 0, "the worker never ran");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(fs::metadata(&heartbeat).unwrap().len(), before);
}

fn stopped_when_asked() {
    let heartbeat = test_file("heartbeat-asked.txt", b"");
    let started = Instant::now();
    env::set_var(ROLE, "hang");
    let mut asked = 0;
    let outcome = run_while(
        &env::current_exe().unwrap(),
        &heartbeat,
        &short_limits(),
        &mut || {
            asked += 1;
            started.elapsed() < Duration::from_millis(500)
        },
    )
    .unwrap();
    env::remove_var(ROLE);
    assert_eq!(outcome, Outcome::Stopped);
    // Asked every tenth of a second, and stopped soon after the answer was
    // no: not at the 20-second time limit.
    assert!(asked >= 3, "{asked}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let before = fs::metadata(&heartbeat).unwrap().len();
    thread::sleep(Duration::from_millis(300));
    assert_eq!(fs::metadata(&heartbeat).unwrap().len(), before);
}

fn garbage() {
    assert_eq!(
        run_role("garbage", &short_limits()),
        Outcome::NotIndexed(Reason::InvalidOutput)
    );
}

fn oversized() {
    let started = Instant::now();
    assert_eq!(
        run_role("oversized", &short_limits()),
        Outcome::NotIndexed(Reason::InvalidOutput)
    );
    // Rejected from the length alone, not after waiting for the time limit.
    assert!(started.elapsed() < Duration::from_secs(5));
}

fn trailing() {
    let started = Instant::now();
    assert_eq!(
        run_role("trailing", &short_limits()),
        Outcome::NotIndexed(Reason::InvalidOutput)
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

fn silent_exit() {
    assert_eq!(
        run_role("silent", &short_limits()),
        Outcome::NotIndexed(Reason::InvalidOutput)
    );
}

fn crash() {
    assert_eq!(
        run_role("crash", &short_limits()),
        Outcome::NotIndexed(Reason::Crashed)
    );
}

fn large_file() {
    let limits = Limits {
        max_file_bytes: 3,
        ..short_limits()
    };
    // The "crash" role would show if a worker had been started.
    assert_eq!(
        run_role("crash", &limits),
        Outcome::NotIndexed(Reason::TooLarge)
    );
}

fn missing_file() {
    let missing = env::temp_dir().join("catchword-supervisor-test/no-such-file.pdf");
    assert_eq!(
        run_role_on("crash", &missing, &short_limits()),
        Outcome::NotIndexed(Reason::CannotOpen)
    );
}

fn missing_worker() {
    let file = test_file("any.txt", b"content");
    let worker = env::temp_dir().join("catchword-supervisor-test/no-such-worker");
    assert!(run(&worker, &file, &short_limits()).is_err());
}

#[cfg(windows)]
fn memory_flood() {
    let limits = Limits {
        memory_bytes: 64 << 20,
        ..short_limits()
    };
    assert_eq!(
        run_role("memory", &limits),
        Outcome::NotIndexed(Reason::MemoryLimit)
    );
}

#[cfg(windows)]
fn child_process() {
    assert_eq!(run_role("spawn", &short_limits()), pages(&["blocked"]));
}
