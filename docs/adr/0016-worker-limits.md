# ADR-16: How the extraction worker is limited

Status: accepted, 3 October 2026. Backlog item CORE-2; requirement SEC-1; threats T1, T2 and T4.

Update, 9 October 2026 (SEC-6): on Windows the worker also lowers itself to low integrity, first thing, before it reads the request. Windows lets a process lower its integrity level, never raise it. At low integrity the worker can still read the document, but cannot write to the user's files, settings or other programs. That adds to the job object's limits: a parser taken over by a hostile file can no longer change what the user owns. See `crates/engine/src/extract/integrity.rs`; `crates/worker/tests/integrity.rs` checks it on the real worker.

## Context

A malicious or broken file can make a parser hang, eat memory or run attacker code. The worker process contains that, but only if the operating system enforces limits on it, and the engine must survive whatever the worker does.

## Options

1. A wall-clock timeout only.
2. A Windows job object, plus the timeout and checks on the worker's output.
3. A restricted token or AppContainer as well, to cut the worker's file and system access.

## Trade-offs

- A timeout alone does not stop a worker that fills memory within the time, or one that starts other programs.
- Job objects are a standard, documented Windows feature that works for a standard user without administrator rights.
- Option 3 is stronger, but much more work. The spec schedules reduced privileges for 0.2.

## Direction

Option 2. Each worker gets its own job object:

| Limit | Default | Effect |
| --- | --- | --- |
| Memory the worker may commit | 512 MB | Allocations beyond it fail; recorded as "needed too much memory" |
| Processes in the job | 1 | The worker cannot start other programs |
| Kill on job close | always | If the engine exits or crashes, Windows ends the worker too |
| Die on unhandled exception | always | A crash ends at once, with no error dialog left waiting |
| CPU time | equal to the timeout | Backstop for the wall-clock limit |

The engine also enforces:

- **Time:** 60 seconds of wall-clock time, then the whole job is killed and the file is recorded as "took too long".
- **File size:** files over 200 MB are skipped before any worker starts.
- **Answer size:** at most 5,000 pages and 64 MB of text. The worker stops early and refuses as "too large"; the engine refuses anything bigger regardless.

Details:

- Windows reports a memory-limit hit through an I/O completion port. Windows does not guarantee delivery, so a missed report shows a memory stop as a crash, never the reverse.
- Rust's standard library cannot start a process paused, so the worker runs briefly before it joins the job. It reads nothing until the request arrives, and the engine sends the request only after the limits apply. If the limits cannot be applied, the worker is killed and no file is handed over.
- The limits live in `engine::extract::Limits`, ready to become user settings (SRC-7).

## Not covered yet

- **Linux and macOS:** only the timeout and the output checks apply. Memory and process limits there come with worker sandboxing in 1.0.
- **Access:** the worker can still read any file the user can. Reduced privileges arrive in 0.2 (threat T1).

## Revisit if

- Real documents hit the memory or time limit (the coverage view will show it).
- Starting one process per file dominates indexing time (ADR-2). A long-lived worker restarted after each failure is the likely answer.
