# ADR-1: Desktop shell and engine language

Status: accepted, a founding decision. The two-week Rust test that confirms it (Q2, section 26) is still the owner's to settle.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Heavy local processing of untrusted files, a very small team, and an intent to go cross-platform.

## Options

1. Tauri with a Rust engine.
2. Electron with TypeScript and native modules.
3. A Python engine bundled beside either shell.

## Trade-offs

- Rust gives safety and one binary, but a smaller contributor pool.
- Electron gives reach, but size and a second language for heavy work.
- Python gives the best libraries, but unreliable packaging.

## Direction

Tauri 2 with a Rust engine.

## Revisit if

- The two-week Rust test at the start of Phase 0 fails (Q2).
- A required format has no viable Rust or C library.

The replacement would be a language the maintainer already ships in.

## Since then

As of 4 October 2026:

- The engine, store, worker, embedding runtime, service, command-line tool and desktop shell are written in Rust, pinned to 1.99.0 in `rust-toolchain.toml`.
- The shell is Tauri 2.12.1. The interface is React and TypeScript.
- The one required format so far without a Rust library, PDF, is read with PDFium, a C++ library, in the worker (ADR-9, ADR-14).
