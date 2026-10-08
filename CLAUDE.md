# Catchword: instructions for Claude Code

Read `HANDOVER.md` first: the living handover (status, architecture, decisions, known issues, failed approaches, next steps). Keep it up to date after meaningful work. `HANDOFF.md` holds the original task queue and its history.

The full plan is `docs/specification.md` (26 sections). Before any work, read sections 1, 5, 9, 11, 12, 13, 20, 21 and 26.

## Commands

- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Privacy check: `sh scripts/check-no-network.sh`
- CI actions pinned to commit hashes: `sh scripts/check-pinned-actions.sh`
- A fresh clone to a working build (tools check, downloads, `npm ci`, build): `sh scripts/setup.sh`; tools only: `sh scripts/setup.sh --check`
- PDFium, ONNX Runtime and the model (once, and after a pin changes): `sh scripts/fetch-pdfium.sh` and `sh scripts/fetch-embedding.sh`
- Try it: `cargo build --workspace` (builds the PDF worker too), then `cargo run -p catchword -- index <folder>` and `cargo run -p catchword -- search <words>`
- Interface (in `apps/desktop/ui`, once: `npm ci`): `npm run typecheck`, `npm test`; with a made-up engine in a browser: `npm run dev:mock`
- Desktop app (after `cargo build --workspace`): `cd apps/desktop && ./ui/node_modules/.bin/tauri dev`
- Command contract changed: `UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract`
- Licence notices: `node scripts/notices.mjs` (needs `cargo install cargo-about --locked --features cli`)
- Measure start-up and memory: `powershell -ExecutionPolicy Bypass -File scripts/measure-app.ps1` (see `docs/benchmarks/2026-10-04-app.md`)
- Windows packages: `sh scripts/package-nsis.sh` (GitHub installer, with the updater) and `sh scripts/package-msix.sh` (Store package), then `cargo test -p catchword --test package -- --ignored` (see `docs/packaging.md`)
- The GitHub build's updater (ADR-24): `cargo clippy -p catchword-desktop --all-targets --features updater -- -D warnings`, `cargo test -p catchword-desktop --features updater`, `sh scripts/test-sign-update.sh`. Signing a release (the owner only, with their private key): `sh scripts/sign-update.sh`

## Rules

1. No network code in `crates/engine`, `crates/store`, `crates/worker`, `crates/embed` or `crates/service`. Network code may only ever live in the desktop shell.
2. Untrusted files are parsed only in a separate worker process with time and memory limits.
3. The index is derived data: one transaction per document, and everything can be rebuilt from the files.
4. Never modify, move or delete the user's documents.
5. Document text is untrusted: show it as plain text only.
6. Format, lint and tests pass before every commit.
7. Every new behaviour has a test.
8. Ask before adding a dependency. No GPL or AGPL libraries.

## How to work

- One task at a time, in the order given in `HANDOFF.md`. Stop and report after each one.
- The owner reviews every change and may be new to Rust. Explain what you did and why, in plain words.
- If the specification and reality disagree, stop and ask.
