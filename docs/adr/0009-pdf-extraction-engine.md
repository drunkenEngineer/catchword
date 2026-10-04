# ADR-9: PDF extraction engine

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

PDF is the main format, and the main attack surface.

## Options

1. PDFium.
2. Pure-Rust libraries.
3. MuPDF.
4. Poppler.

## Trade-offs

- PDFium is robust, but written in C++.
- Pure Rust is safer, but weaker on real files.
- MuPDF and Poppler bring copyleft licences.

## Direction

PDFium, inside the worker.

## Revisit if

A pure-Rust library matches PDFium on the fixture corpus.

## Since then

As of 4 October 2026:

- PDFium comes from pinned prebuilt releases, checked by checksum (ADR-14), and runs only in the worker, under its limits (ADR-16).
- Tests read 150 damaged PDFs with the real worker: 73 read, 74 damaged, 3 without text, and none crashed or ran out of time (`crates/worker/tests/hostile_pdfs.rs`).
- Tests also read files made to exhaust resources (`crates/worker/tests/bombs.rs`). A PDF that expands to about 600 MB is stopped by the 512 MB memory limit in 0.33 s on Windows.
