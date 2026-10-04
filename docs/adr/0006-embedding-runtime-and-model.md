# ADR-6: Embedding runtime and model

Status: accepted. The model was open in the specification; ADR-20 settled it on 3 October 2026.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

The model fixes the size of the vectors, the languages covered, the licence, and the cost of re-indexing.

## Options

The shortlist in section 10 of the specification.

## Trade-offs

- Multilingual models are larger, and may be slightly weaker in English.
- Larger models index more slowly.
- Some strong models have restrictive licences.

## Direction

- ONNX Runtime.
- A multilingual model (Q1), chosen by the Phase 0 benchmark from candidates with permissive licences, and recorded in the index.
- The provisional choice is granite-embedding-97m-multilingual-r2; multilingual-e5-small is the baseline it must beat.

## Revisit if

A clearly better permissive model appears. A change means re-embedding everything in the background, so at most once per major version.

## Since then

- ONNX Runtime 1.28.3 and the Granite model are pinned and checked by checksum when they are fetched and every time the model loads (ADR-18).
- The Phase 0 benchmark and the specification's own rule disagreed, and the owner kept Granite (ADR-20, 3 October 2026).
- The index records the model and the size of its vectors. Opening an index with another model drops its vectors, and they are made again.
- On 4 October 2026 the app embeds 16.7 passages a second in its default mode, short of the target of 20 (`docs/benchmarks/2026-10-04-app.md`). Whether to change the model or the target is the owner's decision.
