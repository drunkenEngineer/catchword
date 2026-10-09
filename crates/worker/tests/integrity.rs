//! The worker gives up the right to write before it reads anything
//! (SEC-6, threat T1): it runs at low integrity, on Windows.
#![cfg(windows)]

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use catchword_engine::extract::integrity;

#[test]
fn the_worker_runs_at_low_integrity_before_it_reads_a_request() {
    // No request is sent: the worker lowers itself first, then waits.
    let mut worker = Command::new(env!("CARGO_BIN_EXE_catchword-worker"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut level = None;
    while started.elapsed() < Duration::from_secs(10) {
        level = integrity::of_child(&worker);
        if level == Some(integrity::LOW) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = worker.kill();
    let _ = worker.wait();
    assert_eq!(level, Some(integrity::LOW));
    // Only the worker is lowered; the program that started it is not.
    assert!(integrity::of_this_process().unwrap() > integrity::LOW);
}
