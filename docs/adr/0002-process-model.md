# ADR-2: Process model

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Parsers crash and can be exploited. Privacy forbids open ports.

## Options

1. One process.
2. A shell plus worker processes.
3. A background Windows service.
4. A local web server with a browser interface.

## Trade-offs

- Workers add a protocol and supervision.
- A service needs administrator rights.
- A local server opens a port.

## Direction

The shell and the engine in one process, extraction in supervised workers, no service, no port.

## Revisit if

- Users need indexing while logged out.
- Worker start-up dominates throughput on small files.

## Since then

As of 4 October 2026:

- Each file is read by a new `catchword-worker` process. The protocol is ADR-15; the time and memory limits, and a Windows job object, are ADR-16.
- There is no service and no port. The window talks to the engine only through Tauri's own channel; its content security policy allows no other connection.
- The second "revisit" condition was met for plain text: starting a worker cost about 42 ms a file in a release build. Plain text and Markdown are therefore read in the engine, as a recorded exception (ADR-17). Every other format goes through the worker.
