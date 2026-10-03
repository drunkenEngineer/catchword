# Contributing to Catchword

Thank you for helping. The project is small and run by one maintainer, so please keep changes focused.

## Set up

1. Install Rust (stable) from https://rustup.rs.
2. Clone the repository.
3. Run `cargo test --workspace`. Everything should pass.

## Before you open a pull request

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `sh scripts/check-no-network.sh`

## Rules that are not negotiable

- **No network code in the engine or the store.** Network access belongs only in the desktop shell.
- **No new dependency without a reason** stated in the pull request. GPL and AGPL libraries cannot be used.
- **Test files must be redistributable** and contain no real personal data.
- **You must be able to explain the code you submit,** whether or not a tool helped you write it.

## Conventions

- Pull request titles follow Conventional Commits, for example `feat(store): add phrase search`.
- Sign off your commits (`git commit -s`) to certify the Developer Certificate of Origin.
- Never attach private documents to an issue. Describe the problem instead.
