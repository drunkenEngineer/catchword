# Handoff: Catchword

You are taking over development of Catchword. Read this file first, then `docs/specification.md`.

## What Catchword is

A free, open-source desktop app for Windows that searches a person's own documents by meaning as well as by exact words, and shows the passage, the file and the page. Nothing about the user's documents ever leaves their computer.

## Where things stand

- **Planning is complete.** The full specification is `docs/specification.md`. Section 26 lists every decision, section 21 is the backlog, section 20 is the roadmap.
- **Code so far** is a Rust workspace with nine crates and a desktop interface:
  - `crates/engine`: scan a folder (with exclusions, `exclude`), read text files, hash content, split text into passages sized in model tokens (with page numbers for PDFs), run the extraction worker under limits (`extract`), and combine keyword and meaning results (`fuse`, reciprocal rank fusion).
  - `crates/worker`: the extraction worker. Reads one PDF with PDFium and returns the text of each page, or a reason code.
  - `crates/embed`: the embedding runtime. ONNX Runtime and the provisional model, granite-embedding-97m-multilingual-r2 in 8-bit form, loaded by full path after a checksum check.
  - `crates/store`: the SQLite index, with FTS5 keyword search over passages and over file paths, and sqlite-vec vectors. Identical files are stored once. Layout version 5: version 4 recorded the files that were not indexed, version 5 indexes paths.
  - `crates/service`: the use cases both front ends share: index a folder, embed what is new, search, group by file.
  - `crates/cli`: the commands `index`, `search` and `status`.
  - `apps/desktop/src-tauri`: the Tauri 2 shell: typed command contract, command handlers, background indexing, settings (version 2: folders, exclusions, first-launch flag), local logs (`log`) and the diagnostics report (`diagnostics`).
  - `apps/desktop/ui`: the React and TypeScript interface: first launch, Search, Library and Settings.
  - `crates/eval`: the retrieval evaluation and benchmark tool (`catchword-eval`), never shipped. The evaluation set is in `eval/` (ADR-19).
  - `crates/test-support`: test PDFs written by code, never shipped.
- **Verified on Windows 11 with Rust 1.99.0** (pinned in `rust-toolchain.toml`): 189 Rust tests and 36 interface tests pass, and format, lint, type check and the privacy check are clean. PDFium, ONNX Runtime and the model come from `scripts/fetch-pdfium.sh` and `scripts/fetch-embedding.sh` (ADR-14, ADR-18).
- **Not built yet:** pause and resource modes, appearance settings, cloud-placeholder handling (SRC-4), OCR, the updater (and so the first launch's update-check step), the GitHub installer, notices for the Rust libraries, and the Store listing. An MSIX package builds (`docs/packaging.md`).

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
4. ~~**Evaluation set and benchmark** (TST-1, ARC-5).~~ Done, 3 October 2026; report in `docs/benchmarks/2026-10-03-phase0.md`. The owner kept Granite (ADR-20), although the spec's speed clause, read literally, said to switch to e5.
5. ~~**The Tauri desktop shell** (UI-1 onward), first slice.~~ Done, 3 October 2026: shell, command contract (ARC-3), lock-down (SEC-1), Search (UI-2), Library (UI-3, first part), background indexing.

Next, in the order of the spec's Phase 2 (section 20):

6. ~~**Coverage in the index** (CORE-6, COV-2).~~ Done, 3 October 2026: index schema version 4 records each file that was not indexed, with its reason and attempts, and upgrades a version 3 index in place. A skipped file is not read again until it changes; a failed one gets a second try, then is parked; Library's Try again and `index --retry` read failed files again. Files that could not be opened (locked?) are tried on every run.
7. ~~**Settings and first launch** (UI-4, UI-5, APP-1, APP-3, APP-4).~~ Done, 3 October 2026: exclusions (SRC-2) of sub-folders and name patterns, with a default list for system, development and credential files, applied by the scan, which also skips hidden and system files; Settings for exclusions, the data location and size (COV-3), and delete all data; a two-step first launch (privacy promise, folders). The update-check step waits for the updater (APP-2), since the Store build has none. Resource mode (IDX-5) and appearance are not done.
8. **Packaging spike** (REL-1, REL-2). Partly done, 3 October 2026: `scripts/package-msix.sh` builds an MSIX holding the app, the worker, PDFium, ONNX Runtime and the model, with no download (makeappx from the Windows SDK); a test proves the packaged files read a PDF and search by meaning. Still to do: **the owner** installs the package with a test certificate and checks it (steps in `docs/packaging.md`), registers as a Store developer and reserves the name; then the Store identity in the manifest, notices for the Rust libraries, and the NSIS installer, the last two needing an OK for new tools.
9. ~~**Logs and diagnostics** (OBS-1, APP-5).~~ Done, 3 October 2026: JSON-line logs in the app's `logs` folder, rotated at 1 MB with three files kept. Event names are fixed text and values are numbers or fixed codes, so no document text, query or file name can be logged by accident (PRIV-3); paths and error details are written only with detailed logs on, and those lines are marked private. Panics are logged with their place in the code. Settings shows a diagnostics report, read before saving, with file names only if the user asks. Delete all data deletes the logs too. Crash reports beyond the panic line (OBS-3) are not done.

The rest of Phase 2: the MUST requirements of sections 6 and 7 that are still open, in the order of the Phase 2 feature list:

10. ~~**Cloud-only files and offline folders** (SRC-4, SRC-5).~~ Done, 4 October 2026: the scan reads attributes from folder listings only; cloud-only and archived files are recorded as `cloud-only`, never opened, and read once they are on the computer again. A folder that cannot be reached is an `Unreachable` error that changes nothing; Library shows it as offline, its files still searchable. Files under a sub-folder that cannot be listed are kept. Windows attribute code sits in `crates/engine/src/attributes.rs` (PORT-4).
11. ~~**Pause, resume and resource mode** (IDX-5, RSC-3).~~ Done, 4 October 2026: Library pauses and resumes indexing, and a user's pause outlasts a restart. Settings offers Light (1 thread), Balanced (half the logical cores, the default) and Fast (all but one); a change loads the model again with the new thread count. ONNX Runtime's own threads are now started by us at below-normal priority, without busy waiting (RSC-2 was only half met before). Indexing pauses when less than 1 GB is free on the index's drive, and Library says why.
12. ~~**Results: names and dates** (SEA-1, SEA-2).~~ Done, 4 October 2026: a second FTS5 index over file paths, kept in step by triggers, filled on upgrade from layout 4. Its matches are a third list in rank fusion: a file found only by its name is labelled so, and a name match found by words or meaning too only rises. Words in more than half of all paths (the folders everything sits in) are ignored. The evaluation still measures passages alone (`search_combined`), because its test files are named after their content; the app uses `search_with_names`. Results show each file's modified date.
13. **Rebuild** (IDX-7): rebuild the index on demand, and automatically after unrecoverable damage, with a quick integrity check at startup.
14. **Text encodings** (EXT-2): detect the encoding of plain-text files. Needs a dependency, so it waits for an OK.
15. **Index health** (COV-1, OBS-2): estimated time, throughput and the last scan in Library.
16. **Announcements and warnings** (A11Y-2, PRIV-4, OBS-3): progress and result counts announced to screen readers; a warning if the data folder is in a synced folder; local crash reports.

Waiting for the owner: the updater (APP-2, REL-3, which needs an update signing key), the GitHub installer and the notices (REL-2), licence and advisory checks (MNT-3), and the Store steps of item 8.

## How to work with the owner

- One backlog item per change. Keep changes small.
- The owner reviews every change and must be able to understand it. Explain what you did and why, in plain words.
- When the specification and reality disagree, stop and ask. Record real decisions as ADRs in `docs/adr`.
- Before the first public push, replace the `OWNER` placeholders listed in `docs/README.md`.

## First message to send

> Read `HANDOFF.md` and `docs/specification.md`. Then do task 1 in `HANDOFF.md`: build and test the workspace on Windows and report exactly what passed and what failed. Do not start task 2 until I say so.
