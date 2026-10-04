# Packaging for Windows

This is the packaging spike (REL-1 in the backlog, section 21 of the specification). It proves that Catchword can ship as an MSIX package: the app, its PDF worker, PDFium, ONNX Runtime and the embedding model, installed together.

## What the package holds

| File | What it is |
| --- | --- |
| `Catchword.exe` | The desktop app (`catchword-desktop`, release build) |
| `catchword-worker.exe` | The PDF worker, started by the app for each PDF |
| `pdfium.dll` | PDFium, loaded by the worker from its own folder |
| `onnxruntime.dll` | ONNX Runtime, loaded by the app from its own folder |
| `models/granite-embedding-97m-multilingual-r2/` | The embedding model and its tokenizer, checked against pinned checksums when loaded |
| `Assets/` | Start menu and Store logos |
| `LICENSE`, `NOTICE`, `licenses/` | Catchword's licence, and the licences of PDFium, ONNX Runtime and the model |
| `AppxManifest.xml` | The package manifest, from `apps/desktop/msix/AppxManifest.xml` |

The app finds every file by full path in its own folder, never through the system search path (ADR-14, ADR-18).

## Build it

From Git Bash, after `sh scripts/fetch-pdfium.sh` and `sh scripts/fetch-embedding.sh`:

```bash
sh scripts/package-msix.sh
```

This makes `target/package/Catchword/` (the files) and `target/package/Catchword.msix` (the package, unsigned). It downloads nothing: `makeappx.exe` comes with the Windows SDK, which the Visual Studio C++ build tools install. The specification points to Microsoft's winapp tool for this. That tool calls the same SDK programs, so it is not needed.

To check that the packaged files work together without installing anything:

```bash
cargo test -p catchword --test package -- --ignored
```

This copies the package's files into a temporary folder with the command-line tool beside them, then indexes a PDF and finds it by meaning. Only the packaged worker, PDFium, ONNX Runtime and model are used.

## Install it on your own PC for a test

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
- **Notices for the Rust libraries.** Their licences must ship too, and appear in About. This needs a generator such as cargo-about, a new tool, so it waits for the owner's OK. One of them, `option-ext`, is under MPL-2.0, which allows this but must be credited; `encoding_rs` carries a BSD-3-Clause part for its data, which must be credited too.
- **The GitHub installer** (REL-2) is Tauri's NSIS installer. Tauri's bundler downloads NSIS to build it, so it also waits for an OK.
- **The updater** does not exist yet, so there is nothing to compile out of the Store build.
