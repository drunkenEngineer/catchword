# Releasing Catchword

The checklist for one release, as things stand on 6 October 2026. Section 18 of the [specification](../specification.md) describes the full pipeline to come. Steps marked **not built yet** wait for it, and are done by hand or skipped meanwhile.

## Version numbers

Catchword follows Semantic Versioning. Before 1.0, a minor version may need the index rebuilt. Four things have version numbers of their own:

| What | Where | Raise it when |
| --- | --- | --- |
| The app | `Cargo.toml` (`[workspace.package]`), `apps/desktop/src-tauri/tauri.conf.json` and `apps/desktop/ui/package.json` | Every release |
| The index layout | `SCHEMA_VERSION` in `crates/store/src/lib.rs` | Its tables change |
| The settings file | `VERSION` in `apps/desktop/src-tauri/src/settings.rs` | Its format changes in a way older versions cannot read |
| The worker protocol | `VERSION` in `crates/engine/src/extract/protocol.rs` | Its messages change (ADR-15) |

To set the app's version:

1. Change it in the three files.
2. Run `npm install` in `apps/desktop/ui`, so `package-lock.json` follows.

`cargo test -p catchword-desktop --test version` fails if any of them disagree. The Store package takes its version from `Cargo.toml`.

## The checklist

### 1. Before building

- [ ] `main` passes every check:
  - `cargo fmt --all -- --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace`;
  - `sh scripts/check-no-network.sh` and `sh scripts/check-pinned-actions.sh`;
  - `cargo deny check`: no known security problem in any dependency;
  - in `apps/desktop/ui`: `npm run typecheck` and `npm test`.

  CI runs the same checks on Windows, Linux and macOS.
- [ ] The evaluation passes: `cargo run -p catchword-eval -- check`.
- [ ] The version is set (above), and the version test passes.
- [ ] `CHANGELOG.md` has an entry for this version, in plain words: what users will notice, and anything they must do.
- [ ] If the index layout, the settings or the worker protocol changed, their own numbers were raised. Test an upgrade from the previous release: an older index is upgraded or rebuilt, and the settings survive.
- [ ] The README's "What works today" and the [user guide](../user/guide.md) describe this version.
- [ ] If any network use changed, [What leaves your computer](../user/network.md) and the [privacy policy](../user/privacy-policy.md) say so.

### 2. Build

- [ ] `sh scripts/package-nsis.sh`: the GitHub installer, in `target/release/bundle/nsis/`, with the updater.
- [ ] `sh scripts/sign-update.sh`: signs the installer for the updater with your private key (it asks for the password), and writes `latest.json` beside it. Run it on your own computer only; the key never goes anywhere else.
- [ ] `sh scripts/package-msix.sh`: the Store package, in `target/package/`.
- [ ] `cargo test -p catchword --test package -- --ignored`: the packages hold every file the app needs, and the licence notices.
- [ ] Checksums, from the project folder:

  ```sh
  sha256sum target/release/bundle/nsis/*-setup.exe target/package/*.msix > SHA256SUMS.txt
  ```
- **Not built yet:** a bill of materials and a build provenance attestation (REL-4); signing. Microsoft signs the Store package after certification. The GitHub installer stays unsigned until SignPath accepts the project, after 0.1.

### 3. Test the packages by hand

On a clean Windows 11 machine, and a quick check on Windows 10. The steps are in [docs/packaging.md](../packaging.md).

- [ ] Install the GitHub installer. Then the first start, add a folder, search, and open a result.
- [ ] Upgrade from the previous release: the folders, the settings and the index are kept.
- [ ] Uninstall: the index and the logs are gone. The settings are kept, unless the box was ticked.
- [ ] Install the Store package locally. A PDF is read, which proves its worker runs (REL-1).
- [ ] The firewall check in [What leaves your computer](../user/network.md): with Catchword blocked, everything still works.
- [ ] The main flows by keyboard alone, and with Narrator and NVDA.
- **Not built yet:** automated end-to-end tests (TST-3).

### 4. Publish

- [ ] Tag the release on `main`:

  ```sh
  git tag -a v0.1.0 -m "Catchword 0.1.0"
  git push origin v0.1.0
  ```
- [ ] On GitHub, draft a release from the tag:
  - the changelog entry as its text;
  - the installer, its `.sig`, `latest.json` and `SHA256SUMS.txt` attached;
  - marked as a **pre-release**, which is the beta channel for now.
- [ ] Submit the MSIX to the Microsoft Store (Partner Center), as in [docs/packaging.md](../packaging.md).
- The updater finds the newest release that is not a pre-release: users of the GitHub download are offered it once it is promoted (step 5), not while it is a pre-release.
- **Not built yet:** a beta channel for the updater (REL-3); the winget manifest.

### 5. After the beta period

- [ ] After several days without a blocking report, promote the same release: on GitHub, untick "pre-release" and mark it as the latest. Published files are never replaced: a fix is always a new version.
- [ ] Keep the last three installers downloadable.

## When a release is bad

1. **Stop the spread.**
   - On GitHub, mark the previous good release as the latest again, and say in the bad release's text what is wrong and which version to use.
   - Once the updater exists, point the stable update manifest back to the last good version instead.
2. **Roll forward.** Ship a higher version with the fix. For Store installs this is the only remedy, and it waits on certification.
3. **Data is protected on the way back.**
   - An older app refuses an index made by a newer one, and offers to rebuild it (REL-5).
   - The settings keep their previous copy.
   - The index can always be rebuilt from the files.
4. **The manual way back:** the last three installers stay downloadable.

## The rollback drill (REL-5)

Practise this once before 0.1, then once a year. Write down the date and the result here.

1. Install the previous release, index a folder, and search.
2. Install the new release over it. Check that it upgrades and keeps everything.
3. Install the previous release again, over the new one. Note whether the installer allows it. If the index layout changed in between, check that the older app says the index is from a newer version and offers a rebuild, and that the folders and settings are intact.
4. Install the new release again, and check that it works.

| Date | Releases | Result |
| --- | --- | --- |
| *not yet practised* | | |

## Not built yet

These are done by hand, or skipped, until they exist:

- the release pipeline from a tag (REL-4);
- a beta channel for the updater (REL-3);
- SignPath signing;
- end-to-end tests (TST-3);
- the winget manifest.
