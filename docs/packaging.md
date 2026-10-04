# Packaging for Windows

Catchword ships two ways (section 10 of the specification): an MSIX package for the Microsoft Store (REL-1), and a per-user installer for GitHub, built with NSIS (REL-2). Both hold the same files: the app, its PDF worker, PDFium, ONNX Runtime, the embedding model, and the licences of all of them.

Both need, once: `sh scripts/fetch-pdfium.sh`, `sh scripts/fetch-embedding.sh`, and cargo-about for the licence notices:

```bash
cargo install cargo-about --locked --features cli
```

## Licence notices

`node scripts/notices.mjs` writes `target/notices/THIRD-PARTY-NOTICES.txt`: every Rust library in the app and the PDF worker (found by cargo-about, from the libraries' own licence files), the JavaScript packages in the interface, PDFium and what it includes, ONNX Runtime and its notices, and the model. Both packaging scripts run it, and the app shows the file in Settings, About. A Rust library under a licence not listed in `about.toml` stops the script: a new licence is a decision. The less common licences in use: MPL-2.0 (`option-ext`, and `cssparser`, `selectors` and `dtoa-short` through Tauri), BSD-3-Clause (`encoding_rs`, `brotli`), ISC (`libloading`) and Zlib (`foldhash`); all allow shipping with credit, which the file gives.

## The GitHub installer (NSIS)

```bash
sh scripts/package-nsis.sh
```

This makes `target/release/bundle/nsis/Catchword_<version>_x64-setup.exe`. The first run downloads, through Tauri, NSIS and the WebView2 bootstrapper, checked against Tauri's pinned hashes. The packaging settings are in `apps/desktop/src-tauri/tauri.nsis.conf.json`, apart from `tauri.conf.json`, so ordinary builds do not need the bundled files.

- It installs for the current user, into `%LOCALAPPDATA%\Catchword`, without administrator rights (SEC-4). (The specification says `%LOCALAPPDATA%\Programs`; Tauri's installer does not offer that place.)
- Uninstalling removes the index and the logs (PRIV-7), which hold text and names from the user's documents. Settings stay unless the user ticks "Also delete your settings" on the uninstall page. An update (`/UPDATE`, as the updater will run it) keeps everything (REL-5). See `apps/desktop/src-tauri/nsis/hooks.nsh`.
- The uninstall page's text comes from `nsis/English.nsh`, a copy of Tauri's with that one line changed. When the Tauri CLI is updated, compare the two.
- It is unsigned, so Windows warns before running it, until SignPath signs it (Q6).

To check it on your own PC: run the installer, start Catchword, index a folder, then uninstall it from Settings, Apps. `%LOCALAPPDATA%\org.catchword.desktop` should then hold `config` only, or nothing if you ticked the box.

## The Store package (MSIX)

The packaging spike (REL-1): it proves that Catchword can ship as an MSIX package.

## What the package holds

| File | What it is |
| --- | --- |
| `Catchword.exe` | The desktop app (`catchword-desktop`, release build) |
| `catchword-worker.exe` | The PDF worker, started by the app for each PDF |
| `pdfium.dll` | PDFium, loaded by the worker from its own folder |
| `onnxruntime.dll` | ONNX Runtime, loaded by the app from its own folder |
| `models/granite-embedding-97m-multilingual-r2/` | The embedding model and its tokenizer, checked against pinned checksums when loaded |
| `Assets/` | Start menu and Store logos |
| `LICENSE`, `NOTICE`, `THIRD-PARTY-NOTICES.txt` | Catchword's licence, and the licences of everything it bundles |
| `AppxManifest.xml` | The package manifest, from `apps/desktop/msix/AppxManifest.xml` |

The app finds every file by full path in its own folder, never through the system search path (ADR-14, ADR-18).

### Build it

From Git Bash:

```bash
sh scripts/package-msix.sh
```

This makes `target/package/Catchword/` (the files) and `target/package/Catchword.msix` (the package, unsigned). It downloads nothing: `makeappx.exe` comes with the Windows SDK, which the Visual Studio C++ build tools install. The specification points to Microsoft's winapp tool for this. That tool calls the same SDK programs, so it is not needed.

To check that the packaged files work together without installing anything:

```bash
cargo test -p catchword --test package -- --ignored
```

This copies the package's files into a temporary folder with the command-line tool beside them, then indexes a PDF and finds it by meaning. Only the packaged worker, PDFium, ONNX Runtime and model are used.

### Install it on your own PC for a test

Windows installs an MSIX only if it is signed by a certificate the PC trusts. For a local test, sign it with a test certificate of your own. These steps change your PC's certificate settings, so **you** run them, all in one PowerShell window opened **as administrator**, in the repository folder. Remove the certificate when you are done (step 5).

1. Make a test certificate. Its subject must match `Publisher` in the manifest:

   ```powershell
   $cert = New-SelfSignedCertificate -Type Custom -Subject "CN=Catchword Development" -KeyUsage DigitalSignature -FriendlyName "Catchword test signing" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
   ```

2. Sign the package with it:

   ```powershell
   & "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe" sign /fd SHA256 /sha1 $cert.Thumbprint target\package\Catchword.msix
   ```

3. Trust the certificate on this PC, then install:

   ```powershell
   Export-Certificate -Cert $cert -FilePath "$env:TEMP\catchword-test.cer"
   Import-Certificate -FilePath "$env:TEMP\catchword-test.cer" -CertStoreLocation Cert:\LocalMachine\TrustedPeople
   Add-AppxPackage target\package\Catchword.msix
   ```

4. Start Catchword from the Start menu and check:
   - The first-launch steps appear.
   - Add a folder holding a PDF with text. The PDF becomes searchable: the worker runs inside the package.
   - Library shows "Searchable by meaning" filling up: ONNX Runtime and the model load.
   - Settings shows where the index is. Windows redirects a packaged app's writes to its own folder, so the real place is under `%LOCALAPPDATA%\Packages\Catchword.Development_…\LocalCache\Local\`. Please note what Settings shows and where the files really are; if they differ, Settings must learn to show the real place.
   - Uninstall Catchword (Start menu, right-click, Uninstall). Its index should be gone with it (PRIV-7).

5. Remove the test certificate:

   ```powershell
   Get-ChildItem Cert:\LocalMachine\TrustedPeople, Cert:\CurrentUser\My | Where-Object Subject -eq "CN=Catchword Development" | Remove-Item
   ```

## Not done yet

- **Store identity.** `Name` and `Publisher` in the manifest are placeholders. They come from Partner Center once the owner registers as a developer and reserves the name (REL-1). The Store signs the package, so no certificate is needed for the Store build.
- **Install, upgrade and uninstall tests** (REL-2) run by hand so far, as above; nothing automates them yet.
- **The updater** does not exist yet, so there is nothing to compile out of the Store build.
