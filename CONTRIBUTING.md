# Contributing to Catchword

Thank you for helping. The project is small and run by one maintainer, so please keep changes focused.

## Set up

1. Install Rust (stable) from https://rustup.rs.
2. Clone the repository.
3. In the project folder, run `rustup toolchain install`. It installs the exact Rust version pinned in `rust-toolchain.toml`.
4. Run `sh scripts/fetch-pdfium.sh` (on Windows, from Git Bash). It downloads the pinned PDFium library into `vendor/pdfium/` and refuses it if its checksum does not match.
5. Run `sh scripts/fetch-embedding.sh` the same way. It downloads the pinned ONNX Runtime library and the embedding model (about 200 MB) into `vendor/`, with the same checks.
6. Run `sh scripts/fetch-eval.sh` the same way. It downloads the evaluation data and the baseline model (about 140 MB), used by the retrieval evaluation and its tests.
7. Install Node.js 22 (22.22 or later) and, in `apps/desktop/ui`, run `npm ci`.
8. Run `cargo test --workspace`, and in `apps/desktop/ui` run `npm run typecheck` and `npm test`. Everything should pass.
9. To run the desktop app: `cargo build --workspace`, then from `apps/desktop` run `./ui/node_modules/.bin/tauri dev`. To work on the interface alone, run `npm run dev:mock` in `apps/desktop/ui` and open http://127.0.0.1:1420: a made-up engine stands in.
10. To build the Windows installer or the Store package, and the licence notices, see `docs/packaging.md`.

## Before you open a pull request

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `sh scripts/check-no-network.sh`
- In `apps/desktop/ui`: `npm run typecheck` and `npm test`
- If you changed a type in `apps/desktop/src-tauri/src/contract.rs`: `UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract`, and commit the regenerated TypeScript

## Rules that are not negotiable

- **No network code in the engine, the store, the extraction worker, the embedding runtime or the service.** Network access belongs only in the desktop shell.
- **No new dependency without a reason** stated in the pull request. GPL and AGPL libraries cannot be used.
- **Test files must be redistributable** and contain no real personal data.
- **You must be able to explain the code you submit,** whether or not a tool helped you write it.

## Conventions

- Pull request titles follow Conventional Commits, for example `feat(store): add phrase search`.
- Sign off your commits (`git commit -s`) to certify the Developer Certificate of Origin.
- Never attach private documents to an issue. Describe the problem instead.
