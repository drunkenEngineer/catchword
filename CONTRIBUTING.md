# Contributing to Catchword

Thank you for helping. The project is small and run by one maintainer, so please keep changes focused.

Start with [How Catchword is built](docs/architecture/overview.md), a ten-minute tour of the parts, the data flow and the three guarded areas. To teach Catchword a new kind of file, the main way to contribute, follow [Adding a file format](docs/contributing/adding-a-file-format.md).

## Set up

1. Install Rust from https://rustup.rs, and Node.js 22 (22.22.2 or later) from https://nodejs.org. On Windows, also install Git for Windows, for Git Bash.
2. Clone the repository.
3. In the project folder, run `sh scripts/setup.sh` (on Windows, from Git Bash). It:
   - checks that the tools are installed, and says what is missing;
   - installs the exact Rust version pinned in `rust-toolchain.toml`;
   - downloads PDFium, ONNX Runtime, the embedding model, and the evaluation data with its baseline model, about 340 MB in all, into `vendor/`. It refuses any file whose checksum does not match;
   - installs the interface's packages (`npm ci` in `apps/desktop/ui`);
   - builds everything.

   It can be run again at any time: what is already there is kept. `sh scripts/setup.sh --check` only checks the tools.
4. Try it on the sample letters: `cargo run -p catchword -- index eval/domain/docs`, then `cargo run -p catchword -- search "tax refund"`.
5. Run `cargo test --workspace`, and in `apps/desktop/ui` run `npm run typecheck` and `npm test`. Everything should pass.
6. To run the desktop app: `cargo build --workspace`, then from `apps/desktop` run `./ui/node_modules/.bin/tauri dev`. To work on the interface alone, run `npm run dev:mock` in `apps/desktop/ui` and open http://127.0.0.1:1420: a made-up engine stands in.
7. To build the Windows installer or the Store package, and the licence notices, see `docs/packaging.md`. To make a release, follow [the release process](docs/contributing/release-process.md).

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
