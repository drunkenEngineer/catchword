# Architecture Decision Records

One file per decision, named `NNNN-short-title.md`, with: context, options, trade-offs, direction, and what would make us revisit it.

ADR-1 to ADR-13 are the founding decisions in section 23 of `docs/specification.md`. Each has a file here too, with what has happened since; the specification stays the source. New decisions continue from ADR-14.

| ADR | Decision |
| --- | --- |
| [0001](0001-desktop-shell-and-engine-language.md) | Desktop shell and engine language: Tauri 2 and Rust |
| [0002](0002-process-model.md) | Process model: one app process, extraction in workers, no service, no port |
| [0003](0003-one-sqlite-index.md) | The index as rebuildable data in one SQLite file |
| [0004](0004-vector-search.md) | Vector search behind an interface, exact first |
| [0005](0005-retrieval-strategy.md) | Retrieval: keyword and vector search, fused by rank |
| [0006](0006-embedding-runtime-and-model.md) | Embedding runtime and model |
| [0007](0007-change-detection-and-content-addressing.md) | Change detection and content addressing |
| [0008](0008-network-policy-and-diagnostics.md) | Network policy and diagnostics |
| [0009](0009-pdf-extraction-engine.md) | PDF extraction engine: PDFium |
| [0010](0010-ocr-engine.md) | OCR engine (proposed) |
| [0011](0011-answer-generation.md) | Answer generation (proposed) |
| [0012](0012-licence.md) | Licence: Apache-2.0 |
| [0013](0013-distribution-signing-and-updates.md) | Distribution, signing and updates |
| [0014](0014-pdfium-binaries.md) | Where the PDFium library comes from |
| [0015](0015-worker-protocol.md) | Worker protocol, version 1 |
| [0016](0016-worker-limits.md) | How the extraction worker is limited |
| [0017](0017-text-files-in-the-engine.md) | Plain text and Markdown are read by the engine, for now |
| [0018](0018-embedding-runtime-and-model.md) | Where ONNX Runtime and the embedding model come from |
| [0019](0019-evaluation-set.md) | The retrieval evaluation set, version 1 |
| [0020](0020-model-passage-size-and-fusion.md) | Embedding model, passage size and keyword fusion after the Phase 0 benchmark |
| [0021](0021-component-interfaces-and-conformance.md) | Component interfaces and their conformance suites |
| [0022](0022-deleted-content-leaves-the-index.md) | Deleted content leaves the index file |
| [0023](0023-moving-the-index.md) | Moving the index to a folder the user chooses |
