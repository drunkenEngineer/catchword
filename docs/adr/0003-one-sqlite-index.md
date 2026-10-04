# ADR-3: The index as rebuildable data in one SQLite file

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Text, the keyword index and the vectors must stay consistent, be easy to delete, and survive corruption.

## Options

1. One SQLite file.
2. SQLite plus separate search and vector stores.
3. LanceDB.

## Trade-offs

- SQLite has one writer, and vectors come from an extension.
- Separate stores are each stronger, but invite consistency bugs.

## Direction

One SQLite file for the index; settings stored apart.

## Revisit if

Measured targets are missed at the stated corpus size, and swapping the vector index is not enough.

## Since then

As of 4 October 2026:

- The index is one file, `data/index.db`, in the app's data folder. Text, the keyword indexes (FTS5) and the vectors (sqlite-vec) are in it. Each file is written in one transaction (rule 3).
- The layout is version 5. Versions 3 and 4 are upgraded in place. An index from a newer version of the app is left alone, and indexing pauses.
- Settings are a separate `settings.json`.
- Settings offers to check the index, rebuild it from the files, or delete all data.
- At the reference size of 250,000 passages the search targets are met on the owner's laptop: keyword search 68 ms and combined search 330 ms at the 95th percentile (`docs/benchmarks/2026-10-03-phase0.md`).
