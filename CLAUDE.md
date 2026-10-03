# Catchword: instructions for Claude Code

Read `HANDOFF.md` first. It says what exists, which decisions are fixed, and what to do next.

The full plan is `docs/specification.md` (26 sections). Before any work, read sections 1, 5, 9, 11, 12, 13, 20, 21 and 26.

## Commands

- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Privacy check: `sh scripts/check-no-network.sh`
- PDFium (once, and after the pin changes): `sh scripts/fetch-pdfium.sh`
- Try it: `cargo build --workspace` (builds the PDF worker too), then `cargo run -p catchword -- index <folder>` and `cargo run -p catchword -- search <words>`

## Rules

1. No network code in `crates/engine`, `crates/store` or `crates/worker`.
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
