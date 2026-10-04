# ADR-21: Component interfaces and their conformance suites

Status: accepted, 4 October 2026, pending the owner's review.

## Context

MNT-2 asks that extractors, the index store and the embedding runtime each sit behind an interface with a shared conformance test suite, so that a component can be replaced, or a second one added, and be held to the same terms as the first.

When this was written there were two extractors (text files read in the engine, ADR-17; PDFs read by the worker, ADR-16), one embedding model in the app (Granite, ADR-20) and one in the benchmark (e5-small), and one index store (SQLite, ADR-3).

## Options

1. A Rust trait for each of the three, with a suite every implementation runs.
2. Traits where there is, or soon will be, more than one implementation; for the store, its public API as the interface and its tests as the suite.

## Trade-offs

- Extractors: a second kind of file (Word, in 0.2) is certain, so a trait pays for itself now. It also made the service read every kind of file one way.
- Embedding: a trait lets tests use a stand-in model that needs no 100 MB download, and a model change (ADR-20) is held to the same contract.
- Store: ADR-3 chose one SQLite file on purpose; nothing plans a second store. A trait over its twenty-odd methods would add a layer, and generic code in the service and the desktop app, for no replacement anyone has in view.

## Direction

Option 2.

- `catchword_engine::extract::Extractor`, implemented by `TextFiles` and `PdfReader`.
- `catchword_embed::Embed`, implemented by `Embedder` (ONNX Runtime) and, for tests, `catchword_test_support::WordModel`.
- The suites are in `catchword_test_support::conformance`: `extractor` and `embedder`. Each implementation's own tests call them: `crates/engine/tests/conformance.rs`, `crates/worker/tests/conformance.rs`, `crates/embed/tests/model.rs` and `crates/test-support/tests/word_model.rs`.
- The store's interface is the public API of `catchword_store::Store`, and its unit tests are its suite. If a second store is ever wanted, the first step is to turn that API into a trait and those tests into a suite, as above.

## Consequences

- A new extractor or model is not done until its suite passes.
- The text path now hashes a file, then reads it: two reads where there was one. Text files are small, and the second read comes from the system's cache.
- The owner may still ask for a store trait; this record says what it would take.
