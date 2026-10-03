# ADR-15: Worker protocol, version 1

Status: accepted, 3 October 2026. Backlog item ARC-4.

## Context

The engine and the extraction worker exchange one request and one response per file, over the worker's standard input and output (ADR-2). The worker parses untrusted files, so a malicious file could take it over and make it send anything back (threat T4). The protocol is one of the three guarded areas in section 13.

## Options

1. JSON through `serde` and `serde_json`.
2. A small hand-written binary format.

## Trade-offs

- JSON is readable and easy to extend, but adds three libraries and their helpers at the security boundary.
- The binary format is about 150 lines with no dependency. Every check is in plain sight in one file, and it is a small target for the fuzzing harness (SEC-4). Extending it means editing that file and bumping the version.

## Direction

A hand-written binary format in `crates/engine/src/extract/protocol.rs`:

- Each message is a frame: a 4-byte little-endian length, then the payload. The payload starts with the protocol version (2 bytes) and a kind (1 byte).
- Request: a page limit, a text limit, and the file path in the operating system's own form, so no file name is altered on the way.
- Response: the text of each page in order (page numbers are implied, so they cannot be out of order), or a refusal code: encrypted, too large, damaged, cannot open, library missing.
- The path goes over standard input, not the command line, so it never shows up in process listings.
- When reading the worker's response, the engine:
  - rejects a frame longer than its limit before reading it;
  - rejects a page count above the limit, or one the frame is too short to hold;
  - requires valid UTF-8;
  - rejects any bytes after the message;
  - never reserves memory based on a length the worker claims.

## Revisit if

The messages grow beyond a few fields, for example for Office formats with headings and sheet names. Then a schema-based format may be worth its dependencies.
