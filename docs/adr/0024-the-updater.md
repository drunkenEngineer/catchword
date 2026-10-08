# ADR-24: The updater in the GitHub build

Status: accepted, 8 October 2026. The owner generated the update key, and agreed to the updater plugin as a dependency. Requirements APP-1 (the first-launch choice) and APP-2 (updates in the GitHub build); REL-3 in part; threats T5 and T6.

## Context

The spec asks the download from GitHub to check for updates, verify their signature, and install them with the user's consent, and lets the user turn this off (APP-2). The Store build is updated by Windows and must have no network code at all (ADR-8, ADR-13). The network code must live in one module of the desktop shell, and reach only a fixed list of hosts (PRIV-2). The update key must stay outside CI, with the owner (T5, T6).

## Direction

### Two builds, from one code base

- The updater is behind a Cargo feature, `updater`, in the desktop shell. `scripts/package-nsis.sh` turns it on for the GitHub installer; `scripts/package-msix.sh` does not.
- Without the feature, the desktop shell has no web client at all. `scripts/check-no-network.sh` checks this on every change, alongside the network-free engine.

### One module, one address

- **The network code.** All of it is in `apps/desktop/src-tauri/src/update_net.rs`, compiled only with the feature.
- **The rules, in every build.** They are in `updates.rs`, compiled into every build and tested there:
  - **The one address asked:** `https://github.com/drunkenEngineer/catchword/releases/latest/download/latest.json`, the manifest of the newest release. The request carries no identifier and no version: GitHub sees an IP address, and the plugin's own name in the request.
  - **Where updates may come from:** only from `https://github.com/drunkenEngineer/catchword/releases/download/<tag>/<file>`. Anything else is refused before downloading. GitHub then serves the file from its own servers.

### Consent, and at most once a day

- **Off until the user agrees.** At first launch the GitHub build asks. An install that skipped that question asks once, in a banner. Settings can change the answer at any time.
- **At most once a day.** After a successful check, the next comes a day later. A failed one, offline for example, is tried again an hour later.
- **Install only when asked.** A newer version is offered in a banner, with its notes as plain text. It is installed only when the user chooses "Install and restart".

### Signed for its version, never older

- **The key.** The owner generated it with Tauri on 8 October 2026. The private key stays on their computer, behind a password; it is never in the repository or in CI. The public key is in `tauri.conf.json`.
- **Checked before use.** Every download is checked against that public key before it runs.
- **Signed for its version** (`requireSignedVersion`). The plugin's documentation explains why: the manifest itself is not signed, so without this, a tampered manifest could pair a newer version number with an older, validly signed installer.
- **Never older.** Downgrades are not allowed (`allowDowngrades` stays off).
- **Signing a release.** The owner runs `scripts/sign-update.sh` after `scripts/package-nsis.sh`. It signs the installer for its version and writes `latest.json`. `scripts/test-sign-update.sh` tests it with a throwaway key.

## Consequences

- **Releases.** Each GitHub release must carry the installer, its `.sig` and `latest.json`.
- **Only the stable channel.** The newest release that is not a pre-release is the stable channel, and pre-releases are not seen by the updater. A beta channel (REL-3) is not built.
- **Licence notices.** The GitHub installer's notices list the updater's libraries (`node scripts/notices.mjs --updater`); the Store package's do not.
- **CI.** It lints and tests the desktop shell with the feature too.

## Revisit if

- A beta channel is wanted. A second, fixed manifest address would then be needed, for example a file in the repository.
- The key is lost or exposed. Generate a new one, put its public key in a release signed with the old key if it still can be, and say so in the release notes. Otherwise users must reinstall from the website.
- SignPath accepts the project. The installer is then Authenticode-signed as well; the update signature stays.
