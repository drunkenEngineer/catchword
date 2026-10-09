# Threat model

The threat model is section 13 of [`specification.md`](specification.md), which stays the source. This page tracks what is in place for each threat, and where. It also holds the review checklist for the guarded areas.

Last checked against the code: 4 October 2026. Update this page in the same change as any mitigation.

## In brief

- **Assets.** The user's documents, the index (a full-text copy of them), the update channel, and the credibility of the privacy promise.
- **Adversaries in scope:**
  - a malicious file that lands in an indexed folder, such as a saved email attachment;
  - a network attacker on the update path;
  - a compromised dependency or build pipeline;
  - a web page trying to reach the app locally.
- **Out of scope:**
  - malware already running as the user, which can read the documents directly;
  - an attacker with administrator rights, or at an unlocked session.

## Threats and what is in place

| # | Threat | In place | Not yet |
| --- | --- | --- | --- |
| T1 | A malicious file exploits a parser | PDFs are parsed only in `catchword-worker`, one process per file (ADR-2). On Windows a job object limits it to 512 MB, one process and a time limit, and kills it with its job (`crates/engine/src/extract/job_windows.rs`, ADR-16). Before it reads anything, the worker lowers itself to low integrity: it can still read the document, but cannot write to the user's files, settings or other programs (SEC-6, `crates/engine/src/extract/integrity.rs`). The worker has no network library (`scripts/check-no-network.sh`). PDFium is pinned (ADR-14). Seeded hostile-input tests: 150 damaged PDFs and generated worker answers (`crates/worker/tests/hostile_pdfs.rs`, `crates/engine/tests/hostile.rs`) | A tighter sandbox, such as an AppContainer. Coverage-guided fuzzing (SEC-4; cargo-fuzz is the owner's call). The memory limit on Linux and macOS (1.0) |
| T2 | Resource exhaustion: zip bomb, giant or deeply nested file | File size and PDF page limits, set in Settings (SRC-7). The worker's time and memory limits. A file that fails twice is parked until it changes, never retried in a loop. Tested with a 600 MB decompression bomb, 100,000 nested arrays and a false page count (`crates/worker/tests/bombs.rs`) | A limit on decompressed bytes for formats other than PDF (0.2, with Office files) |
| T3 | Markup in document text or a file name reaches the interface | Document text and names are only ever placed as text (`apps/desktop/ui/src/safety.test.ts`, and "never turns document text into markup" in `Search.test.tsx`). A strict content security policy: no remote content, inline script or eval (`tauri.conf.json`). Only the listed commands exist (`build.rs`, `capabilities/main.json`). Commands that touch a file take an id from the index, not a path: `open_file`, `reveal_file`, `preview`, `remove_folder`, `include_folder`. New folders come from the system's folder picker, opened by the shell | |
| T4 | A compromised worker returns hostile output | Every answer is checked for size, encoding and structure (ADR-15). 5,000 malformed answers, and a valid one cut and corrupted at every byte, are tested (`crates/engine/tests/hostile.rs`). The worker has no access to the index: it depends only on the engine and PDFium | |
| T5 | The update channel is compromised | The GitHub build's updater (ADR-24) checks every download against the owner's public key before it runs. Each update must be signed for the version it claims (`requireSignedVersion`), and no older version installs. It asks one fixed address and downloads only from this repository's releases (`apps/desktop/src-tauri/src/updates.rs`). The Store build has no updater | Store signing. Authenticode signing through SignPath after 0.1. A release job with manual approval (REL-4) |
| T6 | The update signing key is lost | The key was generated on 8 October 2026 and stays on the owner's computer, behind a password, never in the repository or CI. What to do if it is lost or exposed is in ADR-24 | The owner's offline backup, to be confirmed. A yearly restore test |
| T7 | Dependency or build supply-chain attack | `Cargo.lock` and `package-lock.json` are committed. cargo-deny checks every Rust dependency, on every CI run, for known vulnerabilities, unmaintained or yanked crates, licences and sources: crates.io only (`deny.toml`, MNT-3). Licences are checked again when the notices are made (`about.toml`), and a test keeps the two lists the same. CI actions are pinned by commit hash (`scripts/check-pinned-actions.sh`). The CI token can only read (`permissions: contents: read`), and CI uses no secrets | Advisory checks for the interface's npm packages. Provenance attestation and a bill of materials per release (REL-4) |
| T8 | A tampered model file or prebuilt native library | PDFium, ONNX Runtime and the models are pinned by checksum in the fetch scripts (ADR-14, ADR-18). The model's checksums are checked again every time it loads | |
| T9 | A web page or local process attacks a local port | No listening port exists. The window talks to the engine only through Tauri's own channel | |
| T10 | Someone else reads the index | The index is in the user's own, non-roaming profile (`%LOCALAPPDATA%`). Settings warns if the data folder is inside OneDrive, Dropbox, Google Drive or iCloud Drive. Uninstalling, and "Delete all data", remove it. Deleted files leave nothing readable in the index file (PRIV-5, ADR-22). The user may move the index elsewhere, for example onto an encrypted drive (APP-7). Before the move, they are warned that uninstalling will not remove it there, and whether a cloud service copies that folder (ADR-23) | Optional encryption (PRIV-6, by 1.0) |
| T11 | Private data in logs, crash reports or bug reports | Logs hold no document text or queries. Paths and names appear only with detailed logs switched on (`apps/desktop/src-tauri/src/log.rs`). The diagnostics report is shown before it is saved. The issue templates warn against attaching private files | |
| T12 | Links or junctions lead the scanner out of a chosen folder | Links are not followed, and chosen folders are resolved to real paths (`scan` and `resolve_folder` in `crates/engine/src/lib.rs`). Tested with a junction, a folder link and a file link (`links_and_junctions_out_of_a_chosen_folder_are_not_followed`) | |
| T13 | Secrets swept into the index | Default exclusions for key files, password-manager files and password exports (`DEFAULT_PATTERNS` in `crates/engine/src/exclude.rs`), which the user can edit in Settings | |
| T14 | Prompt injection inside a document | Not applicable until answers arrive (0.4) | All of it, with answers |
| T15 | Opening a result launches something harmful | The path comes from the index, by id, and only indexed files have results. Files are opened with the Windows shell function directly, never through a command line (`apps/desktop/src-tauri/src/open.rs`). A result whose file has moved is reported, not opened | |
| T16 | Privilege escalation | The installer is per user (`installMode: currentUser`). There is no service. The app's manifest declares that it runs as the user who started it (`asInvoker` in `apps/desktop/src-tauri/windows-app.manifest`), checked in the built program by `apps/desktop/src-tauri/tests/manifest.rs` | |
| T17 | A library is planted in the install folder | PDFium and ONNX Runtime are loaded by full path from known folders only (`load_pdfium` in `crates/worker/src/main.rs`, ADR-18). Store installs are read-only | A per-machine installer for managed PCs (after 1.0) |

## Guarded areas

Three parts of the code decide the security posture. A change to any of them needs the maintainer's review (`.github/CODEOWNERS`) and the checklist below in its pull request.

| Area | Where |
| --- | --- |
| The commands the interface can call | `apps/desktop/src-tauri/src/commands.rs`, `build.rs`, `capabilities/main.json`, `tauri.conf.json` (the content security policy) |
| The worker protocol | `crates/worker/src/main.rs`, `crates/engine/src/extract/` (ADR-15) |
| The network module | `apps/desktop/src-tauri/src/update_net.rs`, in the GitHub build only, with its rules in `updates.rs` (ADR-24). Network code may only ever live in the desktop shell, to a fixed host list (`CLAUDE.md` rule 1, PRIV-2) |

### Review checklist

For a change to the commands:

- [ ] A new command is in all three places: `build.rs`, `capabilities/main.json` and `lib.rs`.
- [ ] It takes ids from the index, not paths or file names from the interface.
- [ ] What it returns that came from a document or a file name is shown as plain text only.
- [ ] The content security policy is not loosened.

For a change to the worker protocol:

- [ ] Every field of an answer is checked for size, encoding and structure before use.
- [ ] A malformed or hostile answer ends as a reason for that file, never as a failure of the engine. The hostile-answer tests cover the new fields.
- [ ] The worker still has no access to the index, and no network library.
- [ ] The protocol version changes if old and new no longer understand each other.

For anything touching the network:

- [ ] It is in the desktop shell, and nowhere else (`sh scripts/check-no-network.sh` passes).
- [ ] It reaches only the fixed host list, and sends nothing about the user's documents, queries or file names (PRIV-1).
- [ ] What it downloads is verified by signature or checksum before use (SEC-5).

## Deliberately not done

See section 13 of the specification: no authentication or secrets, no index encryption in the first version, no certificate pinning.
