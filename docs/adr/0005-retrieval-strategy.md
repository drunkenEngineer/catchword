# ADR-5: Retrieval strategy

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Users search both by description and by exact identifier.

## Options

1. Vector search only.
2. Keyword search only.
3. Both, with rank fusion.
4. Both, plus a reranker.

## Trade-offs

- Fusion costs two queries.
- A reranker adds a model and processor time to every search.

## Direction

Keyword and vector search, fused by reciprocal rank, in 0.1. A reranker only on evidence.

## Revisit if

Evaluation shows that a reranker gives a clear gain inside the time budget.

## Since then

As of 4 October 2026:

- The settings came from the Phase 0 benchmark (ADR-20): reciprocal rank fusion with K = 60 and 50 candidates from each list.
- The app fuses a third list, file and folder names, with 20 candidates. This was decided in a working session and is recorded in `HANDOVER.md`, section 6.
- There is no reranker.
- The evaluation meets all 11 of its thresholds; combined recall@10 is 92.5%.
