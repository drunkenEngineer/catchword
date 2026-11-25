# ADR-18: Where ONNX Runtime and the embedding model come from

Status: accepted, 3 October 2026; ONNX Runtime's source and version changed on 25 November 2026 (see the update at the end). Builds on ADR-6 (ONNX Runtime; the model stays open until the Phase 0 benchmark) and ADR-14 (pinned, checksum-verified downloads).

## Context

Meaning-based search needs two large pieces from outside the repository: the ONNX Runtime library (native code) and an embedding model with its tokenizer. Both are supply-chain risks (threat T8), and the model fixes the vectors stored in every index.

The `ort` binding can download ONNX Runtime by itself while compiling, from its maintainer's own server. That is a second, unpinned download path, outside our checksums.

## Direction

### ONNX Runtime

- Microsoft's official release, pinned to **1.28.3** (2 October 2026), the latest patch of the line `ort` 2.0.0-rc.13 is built for. Since 25 November 2026: **1.31.1**, from Microsoft's NuGet package (update below).
- `ort`'s build-time download is switched off. The engine loads the library by full path (`load-dynamic`).
- Loading by full path matters on Windows, which ships an older `onnxruntime.dll` of its own (threat T17).
- Licence: MIT. Third-party notices are kept with the download.

### The model

- `ibm-granite/granite-embedding-97m-multilingual-r2`, the spec's provisional choice, in its published 8-bit ONNX form (`onnx/model_quint8_avx2.onnx`, 98 MB), plus its `tokenizer.json` (25 MB).
- Pinned to Hugging Face revision `835ad14087e140460703cf0fae09f97d469d65c2`.
- Licence: Apache-2.0.
- What the model card and its configuration give us:
  - vectors of 384 numbers, compared by cosine similarity;
  - the vector of the first token is the embedding (CLS pooling);
  - no query or passage prefix;
  - up to 32,768 tokens of context.

### How the files arrive

- `scripts/fetch-embedding.sh` downloads both into `vendor/` (not committed) and refuses any file whose SHA-256 differs from the value in the script. CI runs it and caches the result.
- The engine checks the model's checksums again each time it loads the model, and refuses a mismatch (section 12, "model checksum mismatch").

## Findings from the first build (3 October 2026)

Measured on the owner's PC: Intel i7-13620H, 6 performance and 4 efficiency cores, 16 GB of RAM.

- **One passage at a time.** The 8-bit model sets its rounding scale from everything in one run. So a passage run together with others, or padded to their length, gets a different vector:
  - similarity 0.98 next to an equal-length passage;
  - 0.84 when padded next to a long one.

  Running passages together was no faster (13.2 against 12.8 passages a second), so each passage and query is embedded on its own. A test guards this.
- **Speed: about 13 passages a second** for 350-token passages. Thread-count and graph-optimisation settings made no improvement. That is about 5 hours for the spec's 250,000-passage reference library. Keyword search works meanwhile, as the spec requires. The Phase 0 benchmark must weigh this: the spec's rule moves to the baseline model below 20 passages a second, if the baseline is faster.
- **Loading takes about 1.9 seconds:** the checksum of the model about 0.2 s, reading the 25 MB tokenizer about 1 s, the rest starting ONNX Runtime. Debug builds now optimise third-party libraries; without that, loading took 5 seconds.

## Not covered yet

- The model choice is still provisional. The Phase 0 benchmark (task 4, ARC-5) compares it with multilingual-e5-small, and may also change the passage size.
- The 8-bit file is quantised for CPUs with AVX2. Speed on Windows on ARM, which runs x64 code through emulation, is not yet measured.
- No macOS Intel build of ONNX Runtime is published (neither 1.28.3 nor 1.31.1), so the script supports Windows x64, Linux x64 and macOS on Apple silicon. The NuGet package also holds Windows on ARM and Linux ARM libraries, for when those are supported.

## Updating

1. Change the version or revision and the checksums in the script.
   - ONNX Runtime: download the NuGet package (`https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/<version>/microsoft.ml.onnxruntime.<version>.nupkg`), check it with `dotnet nuget verify --all <file>`: the author signature must be Microsoft Corporation's. Then pin its SHA-256.
   - The model: the digests are in the Hugging Face API (`/api/models/<name>?blobs=true`).
2. Run the script, the full test suite, the evaluation (`cargo run --release -p catchword-eval -- check`) and the embedding measurement (`cargo test --release -p catchword-desktop measure_embedding -- --ignored --nocapture`), and compare them with the version before, back to back.
3. A different model also means changing the model manifest in the code, and a full re-embed of every index. A different ONNX Runtime does not: the index records the model, not the runtime.

## Update, 25 November 2026: ONNX Runtime 1.31.1 from NuGet

Accepted by the owner on 25 November 2026.

**What happened.** Microsoft's `microsoft/onnxruntime` repository disappeared from GitHub around 17 to 18 November 2026, with its release downloads and no redirect, and Microsoft gave no reason. The pinned 1.28.3 archives returned 404, so a fresh clone could not be set up and CI failed once its cache no longer held a copy. 1.28.3 had been published only on GitHub: NuGet and PyPI went from 1.28.0 to 1.29.0.

**The choice.** Microsoft's NuGet package `Microsoft.ML.OnnxRuntime`, which holds the native library for each system, at **1.31.1**: the newest version published before the repository disappeared (28 October 2026), with three versions of fixes over 1.28. 1.32.0 (13 November) was passed over as too close to the unexplained removal; 1.28.0 as older than what we had.

**How it was checked:**
- `dotnet nuget verify --all`: an author signature by Microsoft Corporation (DigiCert code-signing certificate, timestamped 23 October 2026) and nuget.org's repository countersignature (28 October 2026). The package's SHA-256 is pinned in `scripts/fetch-embedding.sh`.
- `ort` 2.0.0-rc.13, as we build it, asks for C API level 17, so any runtime from 1.17 loads; a newer one only logs a note.
- `LICENSE` is unchanged (MIT). `ThirdPartyNotices.txt` adds Mbed TLS, licensed Apache-2.0 or GPL-2.0-or-later and used under Apache-2.0; it ships in our notices as before.
- The Windows library imports no network library (no `ws2_32`, `winhttp` or `wininet`), and contains no network function names, as plain or wide text, and no Mbed TLS code: the same as 1.28.3. Its web addresses are the same list as 1.28.3's: links in error messages and Microsoft's certificate addresses. It adds an import of `SHELL32`.
- Measured back to back on the owner's PC against 1.28.3:

  | | 1.28.3 | 1.31.1 |
  | --- | --- | --- |
  | Evaluation, combined recall@10 | 0.925 | 0.925 |
  | Evaluation, combined MRR@10 | 0.776 | 0.775 |
  | Evaluation, meaning recall@10 | 0.888 | 0.887 |
  | Evaluation, cross-language recall@10 | 0.869 | 0.871 |
  | Passages a second: Light, Balanced, Fast | 5.3, 15.4, 16.6 | 5.8, 16.1, 16.0 |

  All 11 thresholds are met; no score moved by more than 0.002, and speed is within run-to-run variation.

**Consequences.**
- Vectors differ very slightly between the two runtimes, which the evaluation shows to be negligible. Existing indexes keep their vectors; new passages are embedded with 1.31.1.
- The download is one 160 MB package for all systems, instead of a smaller archive per system. CI caches what is taken from it.
- The script now needs `unzip` on every system (`scripts/setup.sh` checks for it).
- The verified 1.28.3 Windows library was kept on the owner's PC in `vendor/onnxruntime-1.28.3-backup/`, for a comparison or a way back.
