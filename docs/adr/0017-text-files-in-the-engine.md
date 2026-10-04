# ADR-17: Plain text and Markdown are read by the engine, for now

Status: accepted, 3 October 2026. An exception to rule 2 ("untrusted files are parsed only in a worker"), recorded so it stays visible.

Update, 4 October 2026: text is no longer decoded as UTF-8 only. Its encoding is detected (EXT-2) with `encoding_rs` and `chardetng`, both memory-safe Rust from the Firefox team. That is still decoding, not parsing a format, so the reasoning below holds.

## Context

Rule 2 and threat T1 exist because complex parsers, such as those for PDF, Office files and images, can be exploited by a crafted file. Plain text and Markdown files are only decoded as UTF-8 and split into words: no format is parsed, and Catchword never renders Markdown.

Starting a worker costs time for every file. Measured on 3 October 2026 on the owner's Windows 11 PC, with 100 one-page PDFs:

- about 53 ms per file in a debug build;
- about 42 ms per file in a release build.

That is about 7 minutes per 10,000 files, before any real work.

## Options

1. Send text files through the worker too.
2. Read them in the engine, with the same size limit and clean-up.

## Trade-offs

- Option 1 follows rule 2 literally, but adds minutes to every first index for no security gain we can name.
- Option 2 keeps untrusted bytes in the main process, but only through Rust's own UTF-8 decoding:
  - memory-safe, and without `unsafe` code;
  - capped at 200 MB per file, and that read cap holds even if the file grows during the scan;
  - with control characters removed.

## Direction

Option 2, as `engine::read_text`. Every other format goes through the worker, from its first version.

## Revisit if

- Encoding detection (EXT-2) brings a decoding library. Run it in the worker, or show that it is memory-safe and has no `unsafe` parsing code.
- A text-like format needs real parsing, for example RTF, HTML, `.eml` or `.mbox`. Those go to the worker.
- Worker start-up becomes cheap enough, for example through a long-lived worker (ADR-16), that the exception no longer buys anything.
