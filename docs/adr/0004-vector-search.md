# ADR-4: Vector search behind an interface, exact first

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Hundreds of thousands of passages, with constant inserts and deletes. sqlite-vec is not yet at version 1.0; SQLite's own Vec1 is emerging.

## Options

1. sqlite-vec, exact or quantised.
2. Vec1.
3. An HNSW library.

## Trade-offs

- Exact search is simple and always correct, but its cost grows with the number of passages.
- Approximate search scales, but adds training or rebuilds, and loses some recall.

## Direction

sqlite-vec, exact in 0.1 and quantised when needed, plus a conformance suite for the interface.

## Revisit if

- The 95th-percentile query exceeds 500 ms at the supported size.
- Vec1 reaches 1.0 with online updates.

## Since then

As of 4 October 2026:

- Vectors are 384 numbers, searched exactly with sqlite-vec.
- At 250,000 passages, vector search takes 265 ms and combined search 330 ms at the 95th percentile, on the owner's laptop. A slower laptop may miss the 500 ms target. Quantised vectors should be measured on the reference laptop before 0.1 (`docs/benchmarks/2026-10-03-phase0.md`).
- The interface is, for now, the public API of the store, and the store's tests are its suite (ADR-21). A separate trait for the vector index waits until a second one is wanted. ADR-21 awaits the owner's review.
