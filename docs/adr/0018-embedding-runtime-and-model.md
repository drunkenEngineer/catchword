# ADR-18: Where ONNX Runtime and the embedding model come from

Status: accepted, 3 October 2026. Builds on ADR-6 (ONNX Runtime; the model stays open until the Phase 0 benchmark) and ADR-14 (pinned, checksum-verified downloads).

## Context

Meaning-based search needs two large pieces from outside the repository: the ONNX Runtime library (native code) and an embedding model with its tokenizer. Both are supply-chain risks (threat T8), and the model fixes the vectors stored in every index.

The `ort` binding can download ONNX Runtime by itself while compiling, from its maintainer's own server. That is a second, unpinned download path, outside our checksums.

## Direction

### ONNX Runtime

- Microsoft's official release, pinned to **1.28.3** (2 October 2026), the latest patch of the line `ort` 2.0.0-rc.13 is built for.
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
- No macOS Intel build of ONNX Runtime 1.28.3 is published, so the script supports Windows x64, Linux x64 and macOS on Apple silicon.

## Updating

1. Change the version or revision and the checksums in the script. For ONNX Runtime the digests are listed on the GitHub release; for the model they are in the Hugging Face API (`/api/models/<name>?blobs=true`).
2. Run the script and the full test suite.
3. A different model also means changing the model manifest in the code, and a full re-embed of every index.
