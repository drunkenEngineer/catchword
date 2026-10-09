# Project Handover

Persistent handover for whoever, person or LLM, continues Catchword. Read it completely before substantial work, verify it against the repository, and update it after meaningful work (see "LLM Operating Instructions" at the end).

Related files: `HANDOFF.md` holds the original task queue (items 1 to 23) with a dated note per item; `CLAUDE.md` holds the working rules; `docs/specification.md` is the full plan (26 sections). If this file and the code disagree, the code wins: correct this file.

Last updated: 2026-10-04.

---

## 1. Project Overview

- **What:** Catchword, a free, open-source (Apache-2.0) Windows desktop app that searches a person's own documents by meaning as well as by exact words, and shows the passage, the file and the page.
- **Main objective:** a public 0.1 in the Microsoft Store and on GitHub (spec section 20).
- **Current stage:** Phase 2 (core functionality) of the spec's roadmap. Pre-alpha. Every MUST requirement that needs no owner decision is implemented; see section 2.
- **Important constraints:**
  - Nothing about the user's documents ever leaves the computer: no network code anywhere except, one day, the updater in the desktop shell (`CLAUDE.md` rule 1, spec PRIV-1/2).
  - Untrusted file formats are parsed only in a separate worker process with time and memory limits (rule 2; ADR-16). Plain text is the recorded exception (ADR-17).
  - The index is derived data: one transaction per document, everything rebuildable from the files (rule 3; ADR-3).
  - Never modify, move or delete the user's documents (rule 4).
  - Document text is untrusted: shown as plain text only (rule 5).
  - Format, lint and tests pass before every commit (rule 6); every new behaviour has a test (rule 7).
  - Ask the owner before adding a dependency; no GPL or AGPL (rule 8).
  - **No AI attribution** in commits or pull requests: no "Co-Authored-By: Claude", no "Generated with". The owner asked for this explicitly.
- **Target users:** solo and small-firm professionals (lawyers, accountants, consultants), researchers and journalists with thousands of private files, who cannot upload them to cloud AI (spec section 3).
- **Owner:** GitHub user `drunkenEngineer`. May be new to Rust: explain changes in plain words.

## 2. Current Status

Overall health: **good**. On 2026-10-06, on Windows 11: 260 Rust tests and 69 interface tests pass; format, lint, type check and the privacy check are clean; the retrieval evaluation meets all 11 thresholds. CI has never been seen to run (see section 8).

| Area | Status |
| --- | --- |
| Engine: scan, exclusions, hidden/system/cloud-only files, encodings, cutting passages | `[DONE]` |
| PDF worker with PDFium, Windows job-object limits, protocol v1 | `[DONE]` |
| Embeddings (Granite 97M, ONNX Runtime), combined search (rank fusion) | `[DONE]` |
| File and folder name search; modified dates; quoted phrases; folder, kind and date filters | `[DONE]` |
| Coverage: problems recorded with reasons, retry, parking after two failures | `[DONE]` |
| Offline folders, cloud-only files, unreadable sub-folders | `[DONE]` |
| Pause/resume, resource modes, low-disk pause | `[DONE]` |
| Pause on battery, resume when plugged in (IDX-9, a 0.2 SHOULD, done early) | `[DONE]` |
| Moving the index to a folder the user chooses (APP-7, 0.2, done early) | `[DONE]`, with a warning that uninstalling will not remove it there (ADR-23) |
| Rebuild on demand, quick check at start, damaged-index recovery, newer-index refusal | `[DONE]` |
| Safe mode after two unclean ends in a row (spec section 19) | `[DONE]` |
| Hostile-input tests (REL-2, start of Phase 3): worker answers, decoding, cutting, patterns, queries, 150 damaged PDFs, a decompression bomb, deep nesting, a false page count | `[DONE]`; fuzzing tools (cargo-fuzz) `[TODO]` |
| Desktop app: first launch, Search, Library, Settings (exclusions, limits, appearance, data, diagnostics, About) | `[DONE]` |
| Local logs, diagnostics report, crash reports | `[DONE]` |
| Keyboard use and focus (A11Y-1); screen-reader announcements (A11Y-2, automated part) | `[DONE]` |
| Colour contrast to WCAG 2.2 AA, checked automatically; contrast themes and reduced motion respected (A11Y-3) | `[DONE]`: `apps/desktop/ui/src/contrast.test.ts`; text scaling to 200% not yet checked by hand |
| Conformance suites for extractors and models (MNT-2) | `[DONE]` |
| Founding decisions ADR-1 to ADR-13 as files in `docs/adr/` (ARC-1) | `[DONE]` |
| Deleted content cannot be read back from the index file (PRIV-5, ADR-22) | `[DONE]` |
| Threat model published with its status per threat and the guarded-area checklist (SEC-3) | `[DONE]`: `docs/threat-model.md` |
| Release checklist, rollback steps and rollback drill (REL-5, written part) | `[DONE]`: `docs/contributing/release-process.md`; the drill itself `[TODO]`, by hand before 0.1 |
| One version number everywhere (spec section 18) | `[DONE]`: `apps/desktop/src-tauri/tests/version.rs` |
| Developer setup script (INF-3) | `[DONE]`: `scripts/setup.sh`, run in full on this PC; `--check` runs in CI (results not yet seen) |
| "Add a file format" guide (DOC-3) | `[DONE]`: `docs/contributing/adding-a-file-format.md`; its acceptance test (an outside contributor follows it) needs a contributor |
| Architecture overview for new contributors (DOC-1) | `[DONE]`: `docs/architecture/overview.md`; its acceptance test (a new reader can name the three guarded areas) needs a reader |
| Draft user guide, network statement and Store privacy policy (DOC-2, a Phase 2 deliverable) | `[IN PROGRESS]`: drafts in `docs/user/`; needs a review by a non-technical tester, a firewall check run by hand, and the owner's contact address |
| Licence notices (cargo-about), shown in About and shipped | `[DONE]` |
| NSIS per-user installer (GitHub build) | `[DONE]`, install/update-uninstall/uninstall tested by hand once |
| MSIX package (Store build) | `[IN PROGRESS]`: builds and its files are tested; never installed |
| Speed and memory against section 14 | `[DONE]` measured on the owner's laptop; two targets missed (section 8) |
| CI on Windows, Linux, macOS | `[DONE]`: green on all three since `edef380` (8 October 2026), with the updater build, the signing test and cargo-deny; the repository is public, so CI minutes are free |
| Updater in the GitHub build (APP-2, APP-1's update choice; ADR-24) | `[DONE]`, not yet tried against a real release; a beta channel (REL-3) `[TODO]` |
| Dependency licence and advisory checks (MNT-3) | `[DONE]`: cargo-deny (approved by the owner, 8 October 2026), `deny.toml`, in CI and the local checks; two unmaintained compile-time macros excepted, with reasons |
| Store registration, name reservation, Store identity in the manifest | `[BLOCKED]`: owner's steps |
| Screen-reader check by hand with Narrator and NVDA (A11Y-2) | `[TODO]` |
| Measurements on a reference laptop (4 cores, about 2020, 8 GB) | `[TODO]` |
| Meaning search ready within 1.5 s of start | `[DECISION NEEDED]` (section 8) |
| Embedding at 20 passages a second | `[DECISION NEEDED]` (section 8) |
| OCR, Office formats, question answering | `[TODO]`, later versions by the spec (0.2 to 0.4) |

Currently being worked on: nothing half-done. The last session ended with everything committed and tested.

## 3. Architecture

Rust workspace (engine and use cases) plus a Tauri 2 desktop shell with a React interface. No server, no accounts, no network at run time.

```text
React interface (apps/desktop/ui)
   │  Tauri commands, typed by ts-rs (contract.rs → ui/src/contract/*.ts); ids, never paths
   ▼
Desktop shell (apps/desktop/src-tauri): AppState, background Indexer thread, settings, logs
   │
   ▼
catchword-service: index_folder, embed_missing, search / search_within, Model, Cutter
   │                 │                            │
   ▼                 ▼                            ▼
catchword-engine   catchword-embed              catchword-store
scan, exclude,     Embed trait; Embedder =      SQLite: FTS5 (passages, paths),
encoding, chunk,   ONNX Runtime + Granite       sqlite-vec vectors, problems
fuse, extract ───► catchword-worker (separate process, PDFium), under a Windows job object
```

- **Frontend:** `apps/desktop/ui/src`. Screens `Search.tsx`, `Library.tsx`, `Settings.tsx`, `Welcome.tsx` (first launch); `App.tsx` (rail, status chip, keyboard shortcuts Ctrl+1/2/3, F5); `Confirm.tsx` (inline confirmation); `engine.ts` (the only module that calls Tauri); `mock.ts` (made-up engine for `npm run dev:mock` and tests); `strings.ts` (all interface text); `appearance.ts`; `styles.css`.
- **Backend (shell):** `apps/desktop/src-tauri/src`:
  - `commands.rs`: `AppState` and every Tauri command;
  - `contract.rs`: every type crossing to the interface, plus a drift test;
  - `indexing.rs`: the `Indexer` thread (runs, pause, low disk, pace, resource-mode threads);
  - `settings.rs`: settings, saved atomically;
  - `log.rs`: logs and crash reports;
  - `session.rs`: whether earlier runs ended cleanly (safe mode);
  - `diagnostics.rs`: the diagnostics report;
  - `disk.rs`: free space, synced-folder detection;
  - `open.rs`: opening files through ShellExecuteW;
  - `views.rs`: store results to contract types;
  - `lib.rs`: setup.
- **Commands (allow-list):** `status search add_folder remove_folder index_now retry_failed settings exclude_folder include_folder set_patterns finish_first_launch delete_all_data rebuild_index check_index notices set_appearance set_limits pause_indexing resume_indexing set_resource_mode set_pause_on_battery pick_index_folder move_index set_update_check check_for_update install_update set_detailed_logs diagnostics save_diagnostics preview open_file reveal_file`. A new command needs all three: the list in `apps/desktop/src-tauri/build.rs`, `allow-<name>` in `capabilities/main.json`, and registration in `lib.rs`.
- **Database:** one SQLite file, `data/index.db`; see section 14.
- **APIs:** none over a network. Worker protocol v1 over stdin/stdout: length-prefixed, versioned frames (ADR-15, `crates/engine/src/extract/protocol.rs`).
- **Authentication:** none, by design.
- **Infrastructure/deployment:** none at run time. Built locally into an NSIS installer (`scripts/package-nsis.sh`) and an MSIX (`scripts/package-msix.sh`). The repository is on GitHub (private).
- **Data flow, indexing:**
  1. scan the folder listings (no file opened);
  2. skip unchanged files (size and date);
  3. hash the file; skip content already indexed (copies);
  4. extract (text in-process, PDF in the worker);
  5. cut into passages of up to 350 tokens with 50 overlapping;
  6. store, one transaction per file; then purge what is gone.

  A second stage embeds passages without vectors, one at a time, saving every 32.
- **Data flow, search:** keyword (FTS5, half the words must match, words in more than 5% of passages ignored) plus vector (sqlite-vec, exact) plus names (FTS5 over paths), fused by reciprocal rank fusion (K = 60, 50 candidates each, 20 for names); grouped by file.
- **Data locations on Windows:**
  - `%LOCALAPPDATA%\org.catchword.desktop\` holds `config\settings.json` (with `settings.previous.json`), `data\index.db`, `logs\catchword.log` (and `.1.log`, `.2.log`, `crash-*.txt`), `session.json` (running mark, see safe mode), and `EBWebView\` (WebView2).
  - The installed program is in `%LOCALAPPDATA%\Catchword\`.

## 4. Tech Stack

| Item | Version / detail |
| --- | --- |
| Rust | 1.99.0, pinned in `rust-toolchain.toml` (with rustfmt, clippy) |
| Tauri | 2.12.1 (`tauri-build` 2.7.1); plugins: dialog 2.8.1, opener 2.7.0, single-instance 2.5.2, window-state 2.5.0 |
| SQLite | `rusqlite` 0.31, bundled; FTS5; `sqlite-vec` 0.1.9 |
| ONNX Runtime | 1.28.3 (win-x64), loaded at run time by full path through `ort` 2.0.0-rc.13 (`load-dynamic`) |
| Tokenizer | `tokenizers` 0.23.2 (no download feature, Oniguruma) |
| Embedding model | granite-embedding-97m-multilingual-r2, 8-bit (`model_quint8_avx2.onnx`), 384 dimensions, CLS pooling; baseline multilingual-e5-small for the benchmark |
| PDF | PDFium build 156.0.8076, through `pdfium-render` 0.9.4, in `catchword-worker` |
| Text encodings | `encoding_rs` 0.8.42, `chardetng` 0.1.17 |
| Other Rust | `walkdir` 2, `dunce` 1, `sha2` 0.10, `anyhow` 1, `serde_json` 1, `ts-rs` 12.0.1, `windows-sys` 0.61 |
| Interface | React 19.3, TypeScript 7.0.2, Vite 8.3.2 |
| Interface tests | Vitest 5.0.3, jsdom 30.1.1, Testing Library (react 16.3.3, dom 10.4.1) |
| Package managers | cargo; npm (`apps/desktop/ui/package-lock.json`) |
| Node.js | 22 (this PC has 22.20.0; jsdom asks for 22.22.2 or later, see section 8) |
| Build tools | MSVC C++ build tools and Windows SDK 10.0.26100 (`makeappx.exe`); Git Bash for the `.sh` scripts |
| Packaging | Tauri bundler with NSIS 3.11 (downloaded by Tauri); `makeappx` for MSIX |
| Notices | `cargo-about` 0.9.2 (`cargo install cargo-about --locked --features cli`) |
| Hosting | none (desktop app); repository at https://github.com/drunkenEngineer/catchword (private) |
| CI | GitHub Actions, `.github/workflows/ci.yml` (Ubuntu, Windows, macOS) |

## 5. Repository Structure

```text
crates/
├── engine/        # Scan (attributes.rs: hidden, system, cloud-only), exclude.rs, encoding.rs,
│                  # chunking (in lib.rs), fuse.rs (rank fusion), extract/ (Extractor trait,
│                  # TextFiles, PdfReader, worker supervisor, protocol.rs, job_windows.rs)
├── worker/        # catchword-worker: reads one PDF with PDFium, answers by protocol v1
├── embed/         # Embed trait, Embedder (ONNX Runtime), model manifests, Threads
├── store/         # Store: SQLite schema v5, keyword/vector/name search, problems, checks
├── service/       # Shared use cases: index_folder, embed_missing, search(_within), Model
├── cli/           # catchword: index, search, status (--db, --limit, --retry)
├── eval/          # catchword-eval: retrieval evaluation (check), benchmark (scale); not shipped
└── test-support/  # Test-only: PDFs written by code, WordModel stand-in, conformance suites
apps/desktop/
├── src-tauri/     # Tauri shell (section 3); tauri.conf.json; tauri.nsis.conf.json (packaging only);
│                  # nsis/hooks.nsh and nsis/English.nsh (installer); capabilities/; build.rs
├── msix/          # AppxManifest.xml (placeholder identity) and logos
└── ui/            # React interface; src/contract/ is generated, do not edit by hand
scripts/           # fetch-pdfium.sh, fetch-embedding.sh, fetch-eval.sh, check-no-network.sh, check-pinned-actions.sh,
                   # notices.mjs, package-nsis.sh, package-msix.sh, measure-app.ps1
docs/
├── specification.md   # The plan; ADR-1..13 are in its section 23
├── adr/               # ADR-14..21
├── benchmarks/        # 2026-10-03 Phase 0 benchmark; 2026-10-04 app measurements
├── packaging.md       # Installer, MSIX, notices, owner's install-test steps
├── threat-model.md    # Spec section 13 by threat: what is in place, where; guarded-area checklist
├── user/              # guide.md, network.md (what leaves the computer, how to check), privacy-policy.md (Store)
├── architecture/      # overview.md: the ten-minute tour for contributors (DOC-1)
├── contributing/      # adding-a-file-format.md (DOC-3), release-process.md (REL-5)
└── README.md          # Docs index; checklist before going public
eval/              # Evaluation set: domain/ (written docs, queries.tsv), thresholds.txt
vendor/            # Downloaded by scripts; gitignored: pdfium/, onnxruntime/, models/, eval/
about.toml         # Licences allowed for shipped Rust crates (cargo-about)
HANDOFF.md         # Original task queue and per-item history
CLAUDE.md          # Rules and commands for the coding assistant
```

## 6. Important Decisions

The founding decisions are ADR-1 to ADR-13 in `docs/specification.md` section 23 (Tauri shell and Rust engine; worker process model; one rebuildable SQLite index; exact vector search first; hybrid retrieval with rank fusion; content addressing; network policy; PDFium; OCR open; answers open; Apache-2.0; Store first then GitHub). Each also has a file in `docs/adr/` (0001 to 0013) with what has happened since; the specification stays the source. Later decisions are only files: ADR-14 to ADR-24. Do not reopen them without the owner. Further decisions made in sessions:

**Decision: Keep Granite as the model (ADR-20)**
- Decision: granite-embedding-97m-multilingual-r2, 350-token passages with 50 overlapping, fusion K = 60 with 50 candidates.
- Reason: best quality in the Phase 0 benchmark; the owner chose it.
- Alternatives considered: multilingual-e5-small, about twice as fast.
- Consequences: embedding stays under the spec's 20 passages a second (section 8).

**Decision: Text files are read in the engine (ADR-17)**
- Decision: plain text and Markdown are decoded in-process, with encoding detection (`encoding_rs`, `chardetng`), not in the worker.
- Reason: a worker per file costs about 42 ms; decoding is not format parsing.
- Alternatives considered: the worker for every file.
- Consequences: the one exception to rule 2; recorded.

**Decision: The updater, in the GitHub build only (ADR-24)**
- Decision: a Cargo feature `updater` (on for `package-nsis.sh`, off for the Store); one network module (`update_net.rs`) asking one address, the newest release's `latest.json` on GitHub; downloads only from this repository's releases; off until the user agrees; at most once a day; installs only on "Install and restart"; `requireSignedVersion` on, downgrades off.
- Reason: APP-2 and PRIV-2. The owner approved `tauri-plugin-updater` and generated the key on 8 October 2026.
- Alternatives considered: the updater in every build (the Store build must have no network code); endpoints in `tauri.conf.json` (the address is fixed in code, next to its tests).
- Consequences: each release needs the installer, its `.sig` and `latest.json`, from `scripts/sign-update.sh`, run by the owner. Pre-releases are not offered.

**Decision: The index may be moved anywhere, with a warning (ADR-23)**
- Decision: the owner's, 5 October 2026. Any folder the user picks; the index gets a folder of its own there (`Catchword index`); before the move, the user is told that uninstalling will not remove it there.
- Reason: APP-7 against PRIV-7 and the Store's uninstall: neither uninstaller can find a moved index.
- Alternatives considered: only folders the uninstaller can find (impossible for the Store); leaving APP-7 out until encryption (PRIV-6).
- Consequences: PRIV-7 holds for the usual place only, and the user is told. A drive missing at start pauses indexing (`IndexAway`) instead of starting an empty index elsewhere.

**Decision: Deleted content leaves the index file (ADR-22)**
- Decision: FTS5 secure-delete on both full-text indexes; a purge of more than 2% of the passages deletes plainly, then compacts; every purge empties the log into the file.
- Reason: PRIV-5. A byte-level test found that a purged file's words and name stayed readable; secure-delete alone made a large purge about 20 times slower (38 s for 10,000 passages).
- Alternatives considered: secure-delete alone (too slow in bulk); compaction after every purge (rewrites the whole index for one changed file).
- Consequences: a single deletion costs about 4 ms a passage more; a large purge adds a compaction (about 0.06 ms per passage left). Pending the owner's review.

**Decision: Interfaces for extractors and models, none for the store (ADR-21)**
- Decision: `Extractor` (TextFiles, PdfReader) and `Embed` (Embedder, WordModel) traits, with shared suites in `catchword_test_support::conformance`. The store's public API is its interface.
- Reason: there are or will be several extractors and models; the store has one implementation by design (ADR-3).
- Alternatives considered: an `Index` trait over the store.
- Consequences: the text path hashes a file then reads it (two reads). The owner may still ask for a store trait.

**Decision: The evaluation measures passages only**
- Decision: `Store::search_combined` (passages) is what `catchword-eval` measures; the app uses `search_with_names` (passages plus file and folder names, and filters).
- Reason: the evaluation's test files are named after their content (`ar-car-insurance.txt`), so name matches would flatter it.
- Consequences: changes to name search are not covered by the thresholds.

**Decision: Quoted phrases bind every result**
- Decision: words in quotes must appear as written in every result, including those found by meaning or name.
- Reason: quotes are an explicit request for exact text.
- Consequences: combined recall moved from 92.9 to 92.5% (39 evaluation questions quote a title); thresholds still met.

**Decision: What uninstalling removes**
- Decision: every uninstall except an update (`/UPDATE`) deletes `data\`, `logs\` and `EBWebView\`; `config\` (settings) only if the user ticks "Also delete your settings" (`apps/desktop/src-tauri/nsis/hooks.nsh`, label in `nsis/English.nsh`).
- Reason: the index holds document text (PRIV-7). A manual reinstall runs the old uninstaller without `/UPDATE`, so deleting settings by default would wipe the folder list on every manual upgrade.
- Alternatives considered: relabelling Tauri's box and inverting it (rejected for that reason).
- Consequences: when the Tauri CLI is updated, compare `nsis/English.nsh` with Tauri's own `languages/English.nsh`.

**Decision: Packaging settings live apart**
- Decision: bundle resources (worker, DLLs, model, notices) are in `tauri.nsis.conf.json`, used only by `scripts/package-nsis.sh`; `tauri.conf.json` keeps `bundle.active: false`.
- Reason: Tauri's build script checks resources on every build, which would break a fresh clone without `vendor/`.

**Decision: Privacy of logs by construction (PRIV-3)**
- Decision: log event names are fixed text, values are numbers or fixed codes; paths and error details are `Value::Private`, written only with detailed logs on and marked private.
- Reason: no document text, query or file name can reach a log by accident.

**Decision: Behaviour around unreachable data**
- Decision:
  - An unreachable folder is offline, not deleted: its files stay.
  - Files under an unreadable sub-folder stay.
  - Cloud-only files are never opened. They are recorded as `cloud-only` and re-checked every run.
  - A damaged index is set aside as `index.damaged.db` and rebuilt; the copy is deleted at the next good start.
  - An index from a newer version is never written to: the app runs paused on an empty one, offering a rebuild.
  - Safe mode: a run marks itself running in `session.json` and clears the mark on a normal exit (Tauri `RunEvent::Exit`). Two unclean ends in a row (crash, kill, power cut) and the next start pauses indexing with `PauseReason::SafeMode`; Library offers Resume and Rebuild. Verified in a release build on 2026-10-04 (one normal close, two forced kills).

**Decision: Other choices worth knowing**
- A pause the user chooses survives restarts (`settings.paused`).
- Resource modes:
  - Light: 1 thread.
  - Balanced: half the logical cores (the default).
  - Fast: all but one.

  ONNX Runtime's threads are started by our own thread manager at below-normal priority, with spinning off.
- The interface never sends a file path: commands take ids; the native dialogs run in the shell.
- Files are opened with ShellExecuteW, not the opener plugin, which uses PowerShell (threat T15). The opener plugin is still used for "show in folder".
- The program file is `Catchword.exe` (`mainBinaryName` in `tauri.conf.json`).
- Batching embeddings was measured and not built (section 9).

## 7. Current Work

- Current task: none in progress. The queue in `HANDOFF.md` (items 1 to 27) is done, except item 8's owner steps.
- Files being modified: none.
- Expected next step: whatever the owner chooses from section 10; the items marked `[DECISION NEEDED]` and `[BLOCKED]` need them. Without input, the best next steps are those under "High" in section 10 that need no decision.

## 8. Known Issues / Bugs

**Meaning search is ready 1.7 s after start (target 1.5 s)**
- Status: `[DECISION NEEDED]`
- Symptoms: searches by words work at once; meaning joins about 1.7 s after start (2.4 s cold, 3.9 s right after an install with the PC busy).
- Cause: verifying the model's SHA-256 checksums on every load (123 MB) and parsing its 25 MB tokenizer.
- Files/components involved: `crates/embed/src/lib.rs` (`Embedder::load`, `verify`).
- Attempts already made: none.
- Current workaround: none needed for use.
- Recommended next investigation: remembering a passed check by file size and date would meet the target, but relaxes ADR-18 ("checked every time it is loaded"): ask the owner.

**Embedding below 20 passages a second**
- Status: `[DECISION NEEDED]`
- Symptoms: 16.7 a second in the default mode on an idle owner's laptop (i7-13620H); about half that with other programs busy.
- Cause: the model is limited by computation (batching gives nothing, section 9).
- Recommended next investigation: only a faster model remains, which ADR-20 weighed and declined. Owner's call.

**The app and the command-line tool share a file name on Windows**
- Status: `[TODO]`, low risk
- Symptoms: `tauri build` writes the app as `target/release/Catchword.exe`; `cargo build --release -p catchword` writes the command-line tool as `catchword.exe` in the same folder. Windows ignores case, so the second overwrites the first.
- Files/components involved: `crates/cli` (package `catchword`), `apps/desktop/src-tauri/tauri.conf.json` (`mainBinaryName`), `scripts/package-*.sh`.
- Attempts already made: none. The packaging scripts run `tauri build` just before copying the app, so they package the right program.
- Recommended next investigation: rename the command-line tool's program (for example `catchword-cli`), which changes how it is run: ask the owner.

**CI: resolved 8 October 2026**
- Status: `[DONE]`
- What it was: every run had failed. The private repository's free CI minutes had run out, so GitHub refused to start the jobs ("recent account payments have failed or your spending limit needs to be increased"). The few early runs that did start found a Windows timing test too strict, already loosened, and three desktop tests that wrote paths with backslashes, which fail on Linux and macOS.
- Fix: the owner made the repository public (free minutes); the three tests now write paths as each system does (`edef380`); CI runs every test even after a failure (`--no-fail-fast`).
- Watch for: `ubuntu-latest` moves to Ubuntu 26 from 19 October 2026 (a GitHub notice in the logs).

**Node.js older than jsdom wants**
- Status: `[TODO]`
- Symptoms: npm warns that jsdom 30 wants Node 22.22.2 or later; this PC has 22.20.0. All interface tests pass anyway. `scripts/setup.sh` refuses Node below 22.12 (what Vite and Vitest need) and only notes anything below 22.22.
- Recommended next investigation: update Node on the owner's PC. CI uses `node-version: 22` (`actions/setup-node`), which takes the newest 22.

**MSIX never installed**
- Status: `[BLOCKED]` (owner's step: it needs a trusted test certificate, a security setting)
- Files: `scripts/package-msix.sh`, `apps/desktop/msix/AppxManifest.xml`, steps in `docs/packaging.md`.
- Also unknown: where Settings says the data is versus where an MSIX app's data really is (Windows redirects `AppData` for packaged apps). `UNKNOWN` until installed.

**Installer location differs from the spec**
- Status: known, accepted for now
- Symptoms: NSIS installs to `%LOCALAPPDATA%\Catchword`; the spec says `%LOCALAPPDATA%\Programs\<App>`. Tauri's installer offers no setting for it.

**Uninstall with "Also delete your settings" ticked never tested**
- Status: `[TODO]` (manual)
- Notes: that path is Tauri's own code (it then removes the whole data folder). The silent uninstall and the update-style uninstall were tested on 2026-10-04.

**A "search" event logged right after start, once**
- Status: `UNKNOWN`
- Symptoms: on 2026-10-04 at 03:44:37Z the log shows `app.started` then `search` in the same second. Unknown whether the user typed or the interface searched by itself.
- Recommended next investigation: watch for it; check `Search.tsx` effects if it recurs.

**Single-instance helper window reported visible**
- Status: `UNKNOWN` impact
- Symptoms: Windows lists a window titled `org.catchword.desktop-siw` (from `tauri-plugin-single-instance`) as visible next to the real "Catchword" window; nobody has seen it on screen.

**Not yet measured on a reference laptop**
- Status: `[TODO]`
- Notes: every figure in `docs/benchmarks/` is from the owner's laptop, faster than the spec's reference. On a slower one, combined search may miss 500 ms (the vector scan dominates). The spec's answer is quantised vectors with rescoring.

## 9. Failed Approaches

- **Batching embeddings** (several passages per ONNX call): measured 1, 2, 4, 8, 16 per call; all within a few percent. Do not build it expecting speed (`docs/benchmarks/2026-10-04-app.md`).
- **Tokenizing a whole long text in one call:** the tokenizer stops counting at 32,768 tokens, so long texts became a few giant passages (a 20 MB file gave 110 passages instead of about 11,000). Count tokens in blocks (`BLOCK_WORDS` = 2,000 in `crates/engine/src/lib.rs`).
- **Asking about a pause only between blocks:** splitting a long text into words came first and took long enough to make a pause late. Ask during splitting too.
- **Passing a query with control characters to FTS5:** it reads its query as C text, so a NUL ends it early and leaves a quote open ("unterminated string"), and the search fails. Found by the hostile-query test; queries now have control characters turned into spaces (`without_controls` in `crates/store/src/lib.rs`).
- **FTS5 secure-delete for every deletion:** it removes deleted words at once (PRIV-5), but at about 4 ms a passage: purging 10,000 passages took 38 s instead of 1.8 s. Large purges now delete plainly and compact afterwards (ADR-22).
- **Timing-only test assertions:** flaky when other programs load the machine. Assert the behaviour (for example "the half-read file is not in the index") and keep time bounds generous.
- **Committing without gating on the checks' exit status:** one commit (48efc39) went in with a failing test, repaired by abca015. Commit only inside `if <checks pass>; then git commit; fi`.
- **The opener plugin to open files:** it runs PowerShell (threat T15). Use ShellExecuteW (`open.rs`).
- **`GetDiskFreeSpaceExW` with a file path:** it fails; pass the nearest existing folder (`disk.rs`).
- **`Model::load` in release builds of tests:** it finds the model beside the program, or in `vendor/` only in debug builds. Measurement tests load from explicit `vendor/` paths.
- **cargo-about:** `cargo install cargo-about --locked` installs no binary without `--features cli`; the `no-clearly-defined` key is invalid in 0.9.2; use `--frozen`.
- **Measuring with generated files that share content:** identical content is stored once ("reused"), so a test library needs a different text per file.
- **Tooling pitfalls for an LLM on this PC:**
  - The Bash tool mangles backslashes and `\u` escapes in heredocs: write Python edit scripts with a file-writing tool instead.
  - Search anchors must match `rustfmt`'s formatting.
  - PowerShell `Remove-Item` on paths under `C:\D folder\` is blocked by the tool's safety check: use `rm` on relative paths in Bash.
  - The desktop app's "Run" button does not accept `/c/...` paths: open a `cmd.exe` window for interactive commands.
  - Driving the real app window with computer-use did not work: check the interface with `npm run dev:mock` in the built-in browser, and the real app through its log.

## 10. TODO / Roadmap

**Critical**
- Get the owner's decisions on the two missed targets (section 8).

**High**
- Try the updater end to end with two real releases (0.1.0, then 0.1.1): `scripts/sign-update.sh`, attach the files, promote, and check the offer and the install. A beta channel (REL-3) after that.
- Screen-reader pass with Narrator and NVDA (A11Y-2): result counts and progress must be read out.
- Measure on a reference laptop: `scripts/measure-app.ps1`, then the two release-build measurement tests (section 12).
- MSIX install test and Store registration: owner steps in `docs/packaging.md`; then put the Store identity in `apps/desktop/msix/AppxManifest.xml`.

**Medium**
- Quantised vector search with rescoring if combined search misses 500 ms on the reference laptop (spec section 14, scale tiers).

- Coverage-guided fuzzing of the worker protocol and the text path (cargo-fuzz needs a nightly toolchain and a new tool: the owner's call).

- When the first format besides PDF goes to the worker: decide how the worker tells formats apart (by extension with `document_kind`, or a kind field in the request, which is protocol version 2). `[DECISION NEEDED]` then, not now; see `docs/contributing/adding-a-file-format.md`, step 2.

**Low**
- Translations (1.0); strings are already in `apps/desktop/ui/src/strings.ts`.

## 11. Environment / Configuration

- No secrets and no environment variables are needed to build or run.
- Environment variables used by tests only:
  - `UPDATE_CONTRACT=1`: rewrites the generated TypeScript in `apps/desktop/ui/src/contract/` from `contract.rs`.
  - `CATCHWORD_TEST_ROLE`: internal to `crates/engine/tests/supervisor.rs`.
- Local requirements:
  - Windows 11 x64;
  - rustup (the toolchain comes from `rust-toolchain.toml`);
  - Visual Studio C++ build tools with the Windows SDK;
  - Node.js 22.22 or later with npm;
  - Git Bash for the `.sh` scripts;
  - Python is not needed by the project.
- Downloads, once, checked against pinned checksums:
  - `sh scripts/fetch-pdfium.sh`: PDFium, into `vendor/pdfium`;
  - `sh scripts/fetch-embedding.sh`: ONNX Runtime and the model, into `vendor/onnxruntime` and `vendor/models`;
  - `sh scripts/fetch-eval.sh`: XQuAD and the baseline model, for the evaluation.
- Ports: 1420, the Vite dev server, only for `tauri dev` and `npm run dev:mock`.
- Configuration files:
  - `apps/desktop/src-tauri/tauri.conf.json`: app, window, CSP, `mainBinaryName`;
  - `tauri.nsis.conf.json`: packaging;
  - `capabilities/main.json`: permissions;
  - `about.toml`: allowed licences;
  - `eval/thresholds.txt`: evaluation minimums;
  - `rust-toolchain.toml`;
  - `.gitattributes`.
- Repository: `origin` is https://github.com/drunkenEngineer/catchword (private), branch `main`. Before making it public, see `docs/README.md`. The commits so far are authored with the owner's own email address; the owner may want a GitHub noreply address first, which would mean rewriting history.
- Future: the updater's signing key will be required. Never store it in the repository.

## 12. Development Commands

All from the repository root unless said otherwise.

| Purpose | Command |
| --- | --- |
| Interface packages (once) | `cd apps/desktop/ui && npm ci` |
| Rust tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --all` |
| A fresh clone to a working build | `sh scripts/setup.sh` (tools only: `--check`) |
| Privacy check | `sh scripts/check-no-network.sh` |
| Dependency policy | `cargo deny check` (rules in `deny.toml`) |
| CI actions pinned to commit hashes | `sh scripts/check-pinned-actions.sh` |
| Interface checks | `cd apps/desktop/ui && npm run typecheck && npm test` |
| Interface with a made-up engine | `cd apps/desktop/ui && npm run dev:mock`, then http://127.0.0.1:1420 (`?first-launch` for the first-launch steps) |
| Desktop app, development | `cargo build --workspace`, then `cd apps/desktop && ./ui/node_modules/.bin/tauri dev` |
| Command-line tool | `cargo run -p catchword -- index <folder>`; `cargo run -p catchword -- search <words>`; `status`; `index <folder> --retry` |
| Contract changed | `UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract` |
| Evaluation | `cargo run --release -p catchword-eval -- check` |
| Licence notices | `node scripts/notices.mjs` |
| GitHub installer | `sh scripts/package-nsis.sh` → `target/release/bundle/nsis/Catchword_<version>_x64-setup.exe` |
| Store package | `sh scripts/package-msix.sh` → `target/package/Catchword.msix` |
| Packaged-files test | `cargo test -p catchword --test package -- --ignored` (after `package-msix.sh`) |
| Start-up and memory | `powershell -ExecutionPolicy Bypass -File scripts/measure-app.ps1 [-App <path to Catchword.exe>]` |
| Scanning measurement | `cargo test --release -p catchword-service --test measure -- --ignored --nocapture` |
| Embedding measurement | `cargo test --release -p catchword-desktop measure_embedding -- --ignored --nocapture` |

There are no migrations to run by hand: the index upgrades itself on opening (section 14). There is no seed data.

## 13. Testing

- Coverage on 2026-10-06: 260 Rust tests and 69 interface tests pass. There are four ignored tests: the packaged-files test and three measurements.
- The evaluation meets all 11 thresholds; combined recall@10 is 92.5%.
- Rust tests:
  - unit tests in each crate;
  - integration tests in `crates/*/tests/`: `cli.rs`, `service.rs`, `supervisor.rs` (stand-in workers that hang, crash, flood memory), `pdf.rs`, `model.rs`, and the conformance tests;
  - desktop tests in `apps/desktop/src-tauri/src/*` (`AppState` flows on temporary folders).
- Some tests need `vendor/` (PDFium, the model) and the worker built by `cargo test --workspace`.
- Interface tests: `apps/desktop/ui/src/*.test.tsx` with the mock engine. `safety.test.ts` fails if any source writes HTML from strings or calls Tauri outside `engine.ts`.
- Important scenarios covered:
  - privacy of logs (no query, text or path);
  - interrupted and paused runs keep nothing half-done;
  - schema upgrades from layouts 3 and 4;
  - damaged and newer indexes;
  - offline folders;
  - quoted phrases;
  - filters;
  - keyboard focus around confirmations;
  - deleted content (PRIV-5): the raw bytes of the index file and its log are searched for a purged file's text, words, name and vector, after a bulk purge, a single deletion, a change and a new cutting pipeline (`a_purged_file_leaves_no_trace_in_the_index_file`), and after opening an index made before (`words_an_older_index_kept_are_dropped_when_it_is_opened`);
  - links out of a chosen folder (SRC-6): a junction, a folder link and a file link leading outside are not followed (`links_and_junctions_out_of_a_chosen_folder_are_not_followed` in `crates/engine/src/lib.rs`). On Windows, symbolic links need Developer Mode or an administrator, so without them only the junction is tried; Linux and macOS try all three;
  - hostile input from a seeded generator, so a failure repeats exactly: 5,000 malformed worker answers and a valid one cut and corrupted at every byte, 3,000 byte strings decoded, 2,000 awkward texts cut, 5,000 name patterns (`crates/engine/tests/hostile.rs`), 3,000 queries on every kind of search (`no_query_makes_a_search_fail` in `crates/store/src/lib.rs`), and 150 PDFs cut short, corrupted or stitched wrongly, read by the real worker (`crates/worker/tests/hostile_pdfs.rs`: 73 read, 74 damaged, 3 without text, none crashed or timed out); and files made to exhaust resources (`crates/worker/tests/bombs.rs`): a PDF whose content expands to about 600 MB of zeros, written with a hand-built deflate stream, is stopped by the 512 MB memory limit in 0.33 s on Windows; 100,000 nested arrays read as no text in 38 ms; a page tree claiming a billion pages reads its one real page.
- Currently failing: none known.
- Manual verification:
  - install, update-uninstall and uninstall of the NSIS build (done once);
  - the MSIX install (`docs/packaging.md`, not done);
  - Narrator and NVDA (not done).

## 14. Database / Data

- **File:** `%LOCALAPPDATA%\org.catchword.desktop\data\index.db`, SQLite in WAL mode with `secure_delete`. Code: `crates/store/src/lib.rs`. If the user moved it (APP-7, ADR-23): `<chosen folder>\Catchword index\index.db`, recorded as `index_folder` in the settings; a copy in progress is `index.db.partial` there.
- **Deleted content (PRIV-5, ADR-22):** both FTS5 tables have FTS5's `secure-delete` option on, so deleted entries are removed at once. A purge of more than 2% of the passages (`BULK_SHARE`), or a new cutting pipeline, switches it off for its transaction and then compacts (`compact_keyword_indexes`: `optimize`, then the option back on, in one transaction). An index found with the option off is compacted at open. Every purge ends with `PRAGMA wal_checkpoint(TRUNCATE)`.
- **Schema version 5** (`SCHEMA_VERSION`):
  - Version 2 added pages, 3 vectors, 4 `problems`, 5 `names_fts`.
  - Layouts 3 and 4 are upgraded in place; older ones are cleared and refilled by the next scan.
  - A newer layout is refused without writing.
- **Tables:**
  - `meta(key, value)`: `schema_version`; `pipeline` (how passages are cut, e.g. `<model id> tokens 350/50 in blocks`; a change clears the index for a rebuild); `model` (vectors' model and dimensions; a change drops vectors); `last_scan`.
  - `contents(hash)`: SHA-256 of a file's bytes; identical files share one content.
  - `files(path PK, size, modified_secs, hash)`.
  - `passages(id, hash, ordinal, page, start_line, end_line, text)`: passages belong to a content, not a file.
  - `passages_fts`: FTS5 over `passages.text` (external content, `unicode61 remove_diacritics 2`).
  - `names_fts`: FTS5 over `files.path`, kept in step by the triggers `files_name_added`, `files_name_removed` and `files_name_changed`.
  - `passage_vectors`: sqlite-vec, 384 floats per passage.
  - `problems(path PK, size, modified_secs, reason, attempts)`: files not indexed. The reason codes are `needs-ocr`, `encrypted`, `too-large`, `cannot-open`, `damaged`, `timed-out`, `memory-limit`, `crashed`, `invalid-output`, `library-missing` and `cloud-only`. Failures are parked after 2 attempts; rule-based skips wait until the file changes; `cannot-open` and `cloud-only` are retried every run.
  - `temp.passage_words`: an fts5vocab view, per connection only.
- **Settings:** `config\settings.json`, version 2, written atomically with the previous copy kept (`apps/desktop/src-tauri/src/settings.rs`). Fields: `folders`, `excluded_folders`, `patterns`, `welcomed`, `detailed_logs`, `resource_mode`, `paused`, `theme`, `text_size`, `max_file_mb`, `max_pages`, `pause_on_battery` (default on), `index_folder` (where the index was moved, or none), `update_check` (none until asked), `last_update_check`, `next_id`. Version 1 files are upgraded.
- **Assumptions:** the index can always be rebuilt from the files. Settings are precious; the index is not.

## 15. External Services

- **At run time:** none. The app makes no network requests. `scripts/check-no-network.sh` fails if the engine, store, worker, embedding runtime or service depend on a network library (it checks `cargo tree` for reqwest, hyper, tokio, rustls and others); CI runs it.
- **At build time, downloaded by scripts, each checked against pinned checksums:**
  - PDFium binaries (ADR-14);
  - ONNX Runtime 1.28.3 from Microsoft's releases (ADR-18);
  - the Granite and e5 models from Hugging Face at pinned revisions;
  - XQuAD for the evaluation.
- **Packaging:** Tauri downloads NSIS 3.11, its `nsis_tauri_utils` plugin and Microsoft's WebView2 bootstrapper, checking their hashes. The installer embeds the bootstrapper; Windows 11 already has WebView2.
- **GitHub:** private repository and Actions CI. Free CI minutes are limited for private repositories (macOS counts ten times). The actions CI uses are pinned to commit hashes, with the version in a comment: actions/checkout v7.0.1, Swatinem/rust-cache v2.9.2, actions/cache v6.1.0, actions/setup-node v7.1.0 (moved up from v4 on 2026-10-09, once CI results could be seen), EmbarkStudios/cargo-deny-action v2.1.1. `scripts/check-pinned-actions.sh` fails on any action not pinned this way; CI runs it.
- **Future:** the updater's static manifest host (GitHub releases), to a fixed host list; the Microsoft Store.

## 16. Product / UX Context

- **The promise is "your files never leave this computer"**, not "this app never goes online". It is shown at first launch and in Settings.
- **First launch** has two steps: the promise, then folders. Indexing starts as soon as a folder is added. Then Search opens.
- **Search is home:**
  - Results are grouped by file, with each passage labelled by how it was found: words, meaning, both, or the file or folder name.
  - Each file shows when it last changed.
  - Keyboard: Ctrl+K or Ctrl+L focuses the box, the arrows move, Enter opens, Ctrl+C copies a passage with its source, Escape goes back.
- **Library:**
  - each folder's state (ready, scanning, offline);
  - two progress bars with pace and time left;
  - Pause or Resume;
  - the last scan;
  - files not indexed, grouped by reason, with Try again for failures.
- **Settings** sections:
  - Appearance;
  - Indexing (resource mode, file limits);
  - What to leave out (folders, name patterns, defaults);
  - Your data (place, size, check, rebuild, delete all);
  - Diagnostics (detailed logs, report);
  - About (licences);
  - Privacy.
- **Nothing found** lists likely causes with counts (scans without text, passwords, cloud-only, over the limits, unreadable), says if filters narrowed the search and offers to search everything, and links to Library. The causes come from each file's reason `code` in the `NotIndexed` contract type.
- **Keyboard (spec section 8 table):** every shortcut in the table works: Ctrl+K/L, Up/Down, Left/Right (fold a file's passages to its best one, or unfold; clicking the file's header does the same), Enter, Ctrl+Enter, Ctrl+C, Ctrl+Shift+C, F6/Shift+F6 (panes marked `data-pane`, cycled by `apps/desktop/ui/src/panes.ts`), F5, Ctrl+1/2/3, Esc.
- **While indexing (spec section 8, states):** Search says how far it has got (`indexingNotice` in `Search.tsx`): "Reading your files: N of M" for an index without a finished scan (new, or rebuilt after damage or on request: this is the "Index repairing" state), "Checking your files for changes" otherwise, and the share searchable by meaning in the second stage. The word-stage counts are per folder, as in Library. A search still running after 300 ms shows "Searching…".
- **Keyword results first (spec section 8, "Loading"):** Search asks for results by words and names alone (`search` with `wordsOnly`, `search_words` in the service: no model), then for the full search. The words-only results are shown only if the full ones have not followed within 150 ms (`WORDS_FIRST_MS`), so a quick search never reorders under the user; when the full results replace them, the passage the user moved to stays chosen if it is still there.
- **Start-up notices (spec section 8, "Index unreadable"):** a damaged index or settings file is reported in `Status.notice`, said once by the engine; the window keeps it in a banner above every destination until the user presses OK. Before 2026-10-04 it travelled in `Status.problem`, which the next refresh replaced, so it was shown only to someone already in Library.
- **Moving the index (APP-7, ADR-23):** Settings, Data: "Move the index…" opens the system's folder dialog (in the shell: the interface never sends a path); a confirmation then warns that uninstalling will not remove the index there, that search and indexing wait while its drive is not connected, and whether a cloud service copies the folder. "Move it back to its usual place" appears once moved. The move is a checked copy (`copy_index` in `commands.rs`); a drive missing at start gives a notice and `PauseReason::IndexAway`, and Resume opens the index once the drive is back.
- **On battery (IDX-9):** every 10 s the shell asks Windows whether it runs on battery (`apps/desktop/src-tauri/src/power.rs`). Going on battery pauses indexing (`PauseReason::Battery`); plugging in resumes it. It acts only on the change (`power::step`), so a user who presses Resume on battery is not overruled until the next unplug, and it never lifts a pause for another reason. Settings has the switch, on by default. The battery pause is not saved: the next start looks again.
- **The model is released when idle (section 14, risk R13):** after 10 minutes without a search or indexing (`RELEASE_MODEL_AFTER`), checked every 10 s by the shell's background loop in `lib.rs`. It gives back about 140 MB (`docs/benchmarks/2026-10-04-app.md`). The next full search loads it again and waits for it, about 2 s; the words-first results show meanwhile. A new indexing run loads it too. Status still says meaning is ready; diagnostics say "released while idle".
- **Filters (SEA-6):** folder, kind (PDF, or text and Markdown) and "Changed": any time, the past week, month or year (`Changed` in the contract; `modified_since` in the store's `Filter`). The date is the file's modified time as last scanned. Of identical copies, the one the filter allows is shown. "Search everything" clears all three when nothing was found.
- **Contrast (A11Y-3):** every text colour is at least 4.5 to 1 against what it sits on, and field edges and focus rings at least 3 to 1, in both themes; `contrast.test.ts` reads `styles.css` and checks it, and that the dark colours, written twice, stay the same. Fields to type or choose in use `--control-border` (search box, filters, limits, names to leave out); buttons keep the softer `--border`, their label showing what they are. Windows contrast themes replace every colour (`forced-colors`), and reduced motion turns off transitions.
- **Copying (RES-3):** Copy passage (Ctrl+C) puts the passage and its source on the clipboard; Copy path (Ctrl+Shift+C) the file's whole path, with control characters removed (`FileHit.path`).
- **A result whose file moved:** `open_file` and `reveal_file` return `FileAction::Missing` instead of opening anything; Search shows a notice naming the file, with Scan now (spec section 8, "File moved or deleted").
- **Confirmations are inline:** the safe choice ("Keep") has the focus, and Escape picks it.
- **Must not change without the owner:**
  - document text is shown only as plain text;
  - the interface never sends paths;
  - no telemetry;
  - quoted phrases bind all results;
  - a user's pause survives restarts.
- **Default exclusions** (`crates/engine/src/exclude.rs`): system files, development folders such as `node_modules`, and files that often hold passwords or keys (`*.kdbx`, `*.pem`, `id_rsa*`, `*passwords*`, `*recovery*codes*` and others). Hidden and system files are always skipped.

## 17. Handover Notes

- Read `CLAUDE.md`: the rules there are binding, especially asking before any new dependency and keeping commits free of AI attribution.
- `apps/desktop/ui/src/contract/*.ts` is generated: change `contract.rs`, then run `UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract`. A drift test fails otherwise.
- `bundle.active` is `false` in `tauri.conf.json` on purpose (section 6).
- Things that look wrong but are intentional:
  - `cannot-open` and `cloud-only` problems are retried every run;
  - the evaluation does not use name search;
  - text files are read twice (hash, then extract);
  - the store has no trait.
- `crates/test-support` depends on `catchword-engine` and `catchword-embed`, and those crates use it as a dev-dependency. That cycle is allowed by Cargo for integration tests only, so the conformance suites run from `tests/` folders, not unit tests.
- **Fragile areas:**
  - The worker supervisor (`crates/engine/src/extract/mod.rs`, `job_windows.rs`): it handles time, memory and crashes. Its tests in `supervisor.rs` play misbehaving workers.
  - The Indexer's locks (`indexing.rs`): the order is control, then snapshot. In `commands.rs`, take one `AppState` lock per statement: a guard lives to the end of its statement, so two in one expression would deadlock (caught in review, never shipped).
  - The index file cannot be deleted while a connection is open. `delete_all_data` and `rebuild_index` first swap the reader for an in-memory store.
- On the owner's PC:
  - the installed app (`%LOCALAPPDATA%\Catchword`) and `target\try-app\` share one data folder;
  - the owner's settings have `paused: true`, so nothing is indexed until Resume.
- `HANDOFF.md` is the older queue document. Keep its history, but put new status here.

## 18. Session Log

**2026-10-03**
- Task: `HANDOFF.md` items 1 to 5: build on Windows; PDF extraction; embeddings and combined search; evaluation and benchmark; the first desktop slice.
- Changes:
  - Rust 1.99.0 pinned; `dunce` for plain paths; CI installs the pinned toolchain.
  - The PDFium worker under a job object.
  - ONNX Runtime and Granite; rank fusion.
  - The evaluation set and thresholds.
  - The Tauri shell with Search, Library and Settings; ts-rs contract; CSP and the command allow-list.
- Files affected: most of `crates/`, `apps/desktop/`, `eval/`, `docs/adr/0014-0020`, `docs/benchmarks/`.
- Decisions: keep Granite (ADR-20); history rewritten once to remove AI attribution.
- Problems encountered: keyword search slow at scale, fixed by ignoring very common words.
- Current state at the end: queue items 6 to 9 planned.

**2026-10-04**
- Task: items 6 to 23 in `HANDOFF.md`; licence notices and the NSIS installer; repository on GitHub.
- Changes:
  - Coverage, retry and parking.
  - Settings, exclusions, first launch; logs and diagnostics; the MSIX spike.
  - Cloud-only and offline handling; pause, resource modes and low disk.
  - Name search and dates; rebuild and recovery; encodings; pace and last scan.
  - Announcements, synced-folder warning, crash reports.
  - cargo-about notices; the NSIS installer, installed and tested on the owner's PC.
  - The long-text token-counting fix and a quicker pause.
  - `Catchword.exe` naming; appearance; keyboard focus.
  - Conformance suites (ADR-21); measurements; file limits; phrases; filters.
- Files affected: across the repository. New: `HANDOVER.md`, `docs/adr/0021-*`, `docs/packaging.md`, `docs/benchmarks/2026-10-04-app.md`, `scripts/notices.mjs`, `scripts/package-*.sh`, `scripts/measure-app.ps1`, `about.toml`, `apps/desktop/src-tauri/nsis/`.
- Decisions: those in section 6 (uninstall behaviour, evaluation scope, phrases, store without a trait, batching not built).
- Problems encountered:
  - the tokenizer limit bug;
  - a commit made with a failing test (repaired);
  - CI results unseen (gh not logged in).
- Current state: everything committed; all checks pass.
- Next step: CI results; the owner's decisions; the "High" items in section 10.
- Later the same day: `HANDOVER.md` written; 11 commits pushed; the two low-disk tests limited to Windows (free space is read on Windows only, so CI on Linux and macOS would fail them); safe mode added and verified in a release build; hostile-input tests added, which found that a NUL in a query made search fail (fixed); 150 damaged PDFs and three resource-exhaustion files through the real worker; "nothing found" causes with counts; a notice when a result's file has moved; ADR-1 to ADR-13 written as files (ARC-1); CI actions pinned to commit hashes, with a check; a test that links and junctions out of a chosen folder are not followed (SRC-6 had none); deleted content leaves the index file (PRIV-5, ADR-22): a byte-level test found that a purged file's words and name stayed in the keyword indexes, now fixed; Copy path (RES-3), with Ctrl+Shift+C as the spec's keyboard table gives it; the table's last two missing shortcuts, F6 between panes and Left/Right to fold a file's passages; Search's indexing notice with counts, and "Searching…" after 300 ms; start-up notices (damaged index or settings) now stay on screen until closed; before, the next status refresh dropped them; the threat model published (SEC-3), with the CI-actions comment corrected from T8 to T7; the app's manifest declares it runs as the user (`asInvoker`), with a test on the built program; keyword results first when the full search is slow; indexing pauses on battery and carries on when plugged in (IDX-9); APP-7 found to conflict with PRIV-7 and the Store's uninstall, recorded for the owner. All pushed.


**2026-10-05**
- Task: the owner's decision on APP-7 (warn at the move), then moving the index.
- Changes: the index can be moved to a folder the user chooses, and back; the warning; a missing drive at start is waited for; Delete all data removes a moved index; ADR-23; threat model and packaging notes updated.
- Files affected: `apps/desktop/src-tauri/src/{commands,indexing,settings,contract,lib}.rs`, `build.rs`, `capabilities/main.json`, `apps/desktop/ui/src/{Settings,Library,engine,mock,strings}.ts(x)` and tests, `docs/adr/0023-*`, `docs/threat-model.md`, `docs/packaging.md`.
- Then: a date filter for search (SEA-6): any time, the past week, month or year. The app rebuilt and installed for the owner to try. The model released after 10 idle minutes, loaded again when needed. Draft user guide, network statement and Store privacy policy (DOC-2) in `docs/user/`; the app's network use checked by sampling (none in 20 s from start); Settings' stale "arrive in a later version" line replaced by the network statement; README brought up to date. Architecture overview (DOC-1). A wrong figure corrected: the evaluation has 2,544 queries, not 3,570. The "add a file format" guide (DOC-3); it found that the worker reads every request as a PDF, so the first new worker format needs a decision on how it tells formats apart. The setup script (INF-3), run in full; its first run failed because a `npm run dev:mock` server left running since 3 October held a file in `node_modules`, so that server was stopped.
- Decisions: ADR-23 (the owner's).
- Problems encountered: GitHub could not be reached for a while (connection timed out), so commits waited to be pushed.
- Current state: everything committed; all checks pass.
- Next step: the next item in section 10 that needs no decision.


**2026-10-06**
- Task: the next steps that need no decision.
- Changes: WCAG AA colour contrast, with an automated check (two failures fixed: the dark theme's found-word highlight, and field edges in both themes); the release checklist, rollback steps and rollback drill (`docs/contributing/release-process.md`); a test that the version is the same in `Cargo.toml`, `tauri.conf.json` and the interface's `package.json` and lock file; the CHANGELOG's Unreleased section brought up to date.
- Current state: everything committed and pushed; all checks pass.
- Next step: the owner's decisions and hand checks (section 10); the rollback drill once there are two releases to practise with.


**2026-10-08**
- Task: the repository went public; CI; the updater; then cargo-deny.
- Changes: CI's history explained (the private repository's free minutes had run out); three tests made to pass on Linux and macOS; CI runs every test even after a failure; the updater in the GitHub build (ADR-24): `updates.rs`, `update_net.rs`, the first-launch question, a one-time banner for installs that skipped it, an offer banner, Settings' Updates section, `scripts/sign-update.sh` and its test, notices with `--updater`, the privacy check extended to the Store build.
- Decisions: the owner's: make the repository public; `tauri-plugin-updater`; the update key (public key in `tauri.conf.json`, private key with the owner, never in the repository).
- Current state: committed and pushed; CI runs on all three systems.
- Then: cargo-deny (MNT-3, approved by the owner): `deny.toml` checks advisories, licences (the same list as `about.toml`, kept equal by a test) and sources for all three systems, with the updater; all our crates marked `publish = false`; two unmaintained compile-time macros excepted (`paste` through tokenizers, `proc-macro-error` through Tauri's Linux GTK libraries).
- CI: green on all three systems from `edef380` on, including the updater and cargo-deny.
- Next step: the owner's remaining decisions (section 10).

---

## LLM Operating Instructions

1. Read `HANDOVER.md` completely before making substantial changes.
2. Inspect the actual repository and verify the handover information against the code.
3. Do not blindly trust the handover file if the repository contradicts it.
4. Identify the current task and continue from the latest known state.
5. Before making architectural changes, check the "Important Decisions" section.
6. Check "Known Issues" and "Failed Approaches" before debugging.
7. After completing meaningful work, update `HANDOVER.md`.
8. Update the "Current Work", "Current Status", "TODO / Roadmap" and "Session Log" sections when relevant.
9. Never expose or write secrets into `HANDOVER.md`.
10. Never fabricate project status, test results, implementation details or decisions.
11. If you discover that an existing handover entry is outdated, correct it rather than preserving incorrect information.
12. Keep the file optimized for another LLM to quickly understand the project and continue development.

**Most important principle:** `HANDOVER.md` should answer, for someone with no other context, what Catchword is, where it stands, why it is built the way it is, and what to do next. (The owner's sentence was cut off at this point; this is its evident intent.)
