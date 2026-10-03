# Handoff: Catchword

You are taking over development of Catchword. Read this file first, then `docs/specification.md`.

## What Catchword is

A free, open-source desktop app for Windows that searches a person's own documents by meaning as well as by exact words, and shows the passage, the file and the page. Nothing about the user's documents ever leaves their computer.

## Where things stand

- **Planning is complete.** The full specification is `docs/specification.md`. Section 26 lists every decision, section 21 is the backlog, section 20 is the roadmap.
- **Code so far** is a Rust workspace with seven crates:
  - `crates/engine`: scan a folder, read text files, hash content, split text into passages sized in model tokens (with page numbers for PDFs), run the extraction worker under limits (`extract`), and combine keyword and meaning results (`fuse`, reciprocal rank fusion).
  - `crates/worker`: the extraction worker. Reads one PDF with PDFium and returns the text of each page, or a reason code.
  - `crates/embed`: the embedding runtime. ONNX Runtime and the provisional model, granite-embedding-97m-multilingual-r2 in 8-bit form, loaded by full path after a checksum check.
  - `crates/store`: the SQLite index, with FTS5 keyword search and sqlite-vec vectors. Identical files are stored once. Layout version 3.
  - `crates/cli`: the commands `index`, `search` and `status`.
  - `crates/eval`: the retrieval evaluation and benchmark tool (`catchword-eval`), never shipped. The evaluation set is in `eval/` (ADR-19).
  - `crates/test-support`: test PDFs written by code, never shipped.
- **Verified on Windows 11 with Rust 1.99.0** (pinned in `rust-toolchain.toml`): 103 tests pass, and format, lint and the privacy check are clean. PDFium, ONNX Runtime and the model come from `scripts/fetch-pdfium.sh` and `scripts/fetch-embedding.sh` (ADR-14, ADR-18).
- **Not built yet:** the evaluation set and benchmark, the Tauri desktop app, OCR, the installer and the Store package. Skipped and failed files are reported but not yet recorded in the index, so they are tried again on every run (CORE-6).

## Decisions already made

Do not reopen these without asking the owner.

- Tauri 2 shell, Rust engine, TypeScript and React interface.
- One SQLite file: FTS5 for keywords, sqlite-vec for vectors, exact search first.
- ONNX Runtime for embeddings. Provisional model: `granite-embedding-97m-multilingual-r2` in 8-bit form. Baseline to beat: `multilingual-e5-small`.
- PDFium for PDF text, inside a separate worker process.
- Keyword and vector results combined by reciprocal rank fusion.
- Apache-2.0 licence. No GPL or AGPL dependencies.
- Windows 11 x64 first. Microsoft Store (MSIX) first; a GitHub installer stays unsigned until SignPath accepts the project.
- No telemetry and no cloud models, ever.

## Rules that must hold in every change

1. No network code in `crates/engine` or `crates/store`. `sh scripts/check-no-network.sh` must pass.
2. Untrusted files are parsed only in a worker process with time and memory limits.
3. The index is derived data. One transaction per document. Anything in it can be rebuilt from the files.
4. Never modify, move or delete the user's documents.
5. Document text is untrusted. The interface shows it as plain text only.
6. Before every commit these pass: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
7. Every new behaviour has a test. Test files must be redistributable and contain no real personal data.
8. Ask before adding a dependency, and say why it is needed.

## Next tasks, in order

1. ~~**Build and test on Windows.**~~ Done, 3 October 2026.
2. ~~**PDF extraction** (backlog CORE-2, CORE-3).~~ Done, 3 October 2026. See ADR-14 to ADR-17.
3. ~~**Embeddings and combined search** (CORE-4, CORE-5, CORE-7, DB-2).~~ Done, 3 October 2026. Passages are 350 tokens with 50 shared, provisionally. Embedding runs at about 13 passages a second on the owner's laptop; see ADR-18 before choosing the model.
4. ~~**Evaluation set and benchmark** (TST-1, ARC-5).~~ Done, 3 October 2026; report in `docs/benchmarks/2026-10-03-phase0.md`. ADR-20 is **proposed**: the owner must confirm keeping Granite, because the spec's speed clause, read literally, says to switch to e5.
5. **Only then** the Tauri desktop shell (UI-1 onward).

## How to work with the owner

- One backlog item per change. Keep changes small.
- The owner reviews every change and must be able to understand it. Explain what you did and why, in plain words.
- When the specification and reality disagree, stop and ask. Record real decisions as ADRs in `docs/adr`.
- Before the first public push, replace the `OWNER` placeholders listed in `docs/README.md`.

## First message to send

> Read `HANDOFF.md` and `docs/specification.md`. Then do task 1 in `HANDOFF.md`: build and test the workspace on Windows and report exactly what passed and what failed. Do not start task 2 until I say so.
