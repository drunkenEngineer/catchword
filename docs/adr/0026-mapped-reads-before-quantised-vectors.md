# ADR-26: Faster searches on a slower laptop: mapped reads, not yet quantised vectors

Status: accepted, 26 November 2026, for the owner's review. The owner approved two things that day: quantised vectors, to bring combined search under its target on a slower laptop, and a study of keyword search there. Measuring showed that two smaller changes do both, with results unchanged. Quantised vectors stay the next step for larger libraries. Targets (spec section 14): keyword results under 100 ms and combined results under 500 ms, at the 95th percentile.

## Context

On a stand-in for the reference laptop (the owner's laptop held to its four efficiency cores, `docs/benchmarks/2026-11-26-stand-in.md`), with 250,000 passages, keyword search took 166 ms and combined search 777 ms at the 95th percentile.

The specification's next tier for vector search is a quantised scan with rescoring: a compact copy of every vector is scanned, and the best candidates are scored again with the full vectors. It says to quantise "only when a measured target is missed".

## What was measured

All on the efficiency cores (processor affinity `F000`), with 250,000 synthetic passages and 384-number vectors.

### Quantised copies, with a throwaway probe

The probe used the store's own tables and 40 random queries. Random vectors are the hardest case for a quantised copy, because every candidate is nearly as close as the next. Real vectors have clear near neighbours.

| Way of searching for meaning | Median | 95th percentile | True top 10 found |
| --- | ---: | ---: | ---: |
| Exact scan (sqlite-vec), as before | 487 ms | 555 ms | all |
| Exact scan, reads through a memory map | 158 ms | 169 ms | all |
| 1-bit copies, 50 candidates, rescored | 29 ms | 31 ms | 27% |
| 1-bit copies, 2,000 candidates, rescored | 38 ms | 42 ms | 84% |
| 8-bit copies, 60 candidates, rescored | 181 ms | 195 ms | 100% |
| 8-bit copies, 60 candidates, rescored, mapped | 79 ms | 87 ms | 100% |

What the probe showed:

- **Most of the time was spent reading, not comparing.** SQLite reads its file one 4 KB page at a time, with a call to the system for each. A vector search reads every vector: 384 MB at this size, about 100,000 calls. Through a memory map, SQLite reads the pages where they already lie in memory.
- **1-bit copies lose too much.** One bit for each of the 384 numbers kept only 84% of the true top 10, even with 2,000 candidates. Real vectors would do better, but by an unknown amount.
- **8-bit copies keep everything.** One byte for each number found the true top 10 every time, with only 60 candidates. They need a quarter of the full vectors' space, about 100 MB more at this size.
- **Rescoring cannot read the full vectors from sqlite-vec.** sqlite-vec keeps vectors in blocks of 1,024. Reading one vector walks the block's pages up to it: about 0.8 ms a vector, so 330 ms for 400 candidates. From a plain table, with one row a vector, it took about 0.02 ms.
- **sqlite-vec's choice of the best candidates grows with how many are wanted:** 124 ms for 400 one-bit candidates.

### Keyword search

Keyword search sorted its matches with `ORDER BY bm25(passages_fts)`. Sorted that way, SQLite works out every column of every matching passage before sorting: the highlighted snippet, the passage's row and its file. It then keeps only the first few. FTS5's own `ORDER BY rank` gives the same score, but sorts first. Then only the passages that are read get a snippet. On an index read without a map, every passage's row costs a call to the system.

### Both, with `catchword-eval scale`

Two runs each, 50 queries, searching on a connection of its own as the app does. Times are 95th percentiles.

| | Before | Sorted by rank | Mapped and sorted by rank | Target |
| --- | ---: | ---: | ---: | ---: |
| Keyword search | 133 ms | 59 ms | 47 ms | 100 ms |
| Vector search | 493 to 500 ms | 492 ms | 155 to 160 ms | |
| Combined search | 630 to 638 ms | 558 ms | 205 to 210 ms | 500 ms |

Mapped reads with the old sort gave 42 to 43 ms for keyword search. With a map, the sort makes no difference that can be measured. On the owner's laptop (its performance cores), mapped and sorted by rank: keyword 24 ms, vector 80 ms, combined 107 ms.

The evaluation set (ADR-19) gives the same scores, to the last digit, with either sort.

## Options

1. **Mapped reads, and keyword matches sorted by rank.** One setting and one changed query. Results stay exact, and nothing in the index changes.
2. **8-bit copies with rescoring, and mapped reads.** Faster still for meaning. But it needs a new table, a migration of every index, 100 MB more at the reference size, and a scan of our own beside sqlite-vec's.
3. **1-bit copies.** Fastest, but it loses results.

## Decision

Option 1.

- **The index in its usual place is read through a memory map** (`Store::map_reads`, used by `open_reader` in `apps/desktop/src-tauri/src/commands.rs`). The map covers up to SQLite's limit of just under 2 GB; the rest of a larger index is read as before.
- **A moved index is read as before** (ADR-23). It may sit on a drive that can be unplugged. A read through a memory map from a drive that has gone stops the whole program, where an ordinary read returns an error. On Windows, SQLite catches such faults only for its small `-shm` file, not for the index itself.
- **Only reading changes.** SQLite still writes with ordinary calls, so each document's transaction and its safety are as before.
- **Only the connection that searches maps its reads.** The indexing connection and the command-line tool read as before.
- **SQLite drops the map and makes it again when another connection has written,** so searching sees what indexing has just added. A test checks it (`a_mapped_index_still_reads_what_another_connection_writes_later`).
- **Keyword matches are sorted by FTS5's rank,** which is bm25 with its default weights. Nothing sets other weights.

## Consequences

- **On the stand-in, keyword and combined search now meet their targets** in the usual place, with room to spare: 47 and 210 ms.
- **A moved index on a slower laptop still misses the combined target:** 558 ms on the stand-in. Keyword search meets its target there (59 ms).
- **The working set grows by the pages a search has read.** They are pages of the index file, shared, and the system takes them back when memory is short. The app's private memory is unchanged. Without the map, the system keeps the same pages in its file cache anyway.
- **Quantised vectors wait for a larger library.** When they come, they should be 8-bit copies, with the full vectors kept in a plain table for rescoring and the best candidates chosen in our own code, not sqlite-vec's.

## Revisit if

- A real reference laptop still misses either target.
- Libraries beyond the reference size are supported (the specification's second tier, about 1.25 million passages). Quantise then, as above.
- Moved indexes must meet the combined target on slower laptops. 8-bit copies would do it without a map.
- SQLite learns to catch faults in its memory map on every system. A moved index could then be mapped too.
