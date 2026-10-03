# ADR-20: Embedding model, passage size and keyword fusion after the Phase 0 benchmark

Status: **accepted**, 3 October 2026. The benchmark and the spec's own decision rule disagreed on the model, so the owner decided: keep Granite. Backlog item ARC-5; the evidence is in `docs/benchmarks/2026-10-03-phase0.md`.

## Context

The spec leaves the embedding model open until a benchmark (ADR-6, section 26), and says the evaluation set decides passage size and fusion settings (section 15). Its rule for the model: the provisional choice, Granite, stands "unless Granite is more than 2 points worse on recall@10, or below 20 passages a second where the baseline is not".

## Decided and in effect

### Keyword search: half the words, without very common ones

- A passage matches if it contains at least half of the query's distinct words.
- Words found in more than 5% of passages are left out of the query; if every word is that common, the rarest is kept.
- It replaces "every word must appear".

Result: combined recall@10 rose from 91.5 to 92.9 and MRR@10 from 73.0 to 77.8, with cross-language search unchanged. Keyword search at 250,000 passages takes 68 ms (95th percentile), against a target of 100 ms.

### Fusion: reciprocal rank fusion, 50 candidates

- Reciprocal rank fusion with K = 60 (ADR-5).
- Each search contributes 50 results before they are combined.
- 10 and 20 candidates were measured; the differences were within what the small corpus can show.

### Store writes: constant cost per file

- Storing a file no longer scans the whole index.
- Writing 250,000 passages took 956 s before and 55 s after.

### Thresholds

- `eval/thresholds.txt` holds the scores of the current settings minus two points.
- CI runs `catchword-eval check` on Linux, and fails below them.

## Model and passage size

### The model: keep Granite

- **On quality,** Granite wins clearly: 11 to 16 points better on combined recall@10, and 27 to 36 points better on cross-language meaning search (use case UC9).
- **On speed,** the spec's rule, read literally, says to switch to e5 at 350 and 500 tokens. There Granite embeds 19.7 and 15.3 passages a second, e5 39.4 and 27.0.
- **Why the speed clause misleads here:** passages a second depends on passage size. Granite embeds the whole set in about 10 s at every size, against about 5 s for e5. The honest comparison is that Granite takes about twice as long to index.

**Decision (owner, 3 October 2026): keep Granite.** Indexing runs in the background, and keyword search works meanwhile (assumption A6). A 2-point quality margin is the spec's own threshold for switching, and Granite is ahead by more than 10. Twice the indexing time is a smaller cost than losing cross-language search. The speed target should be re-checked on the reference laptop.

Fallback: e5, if the reference laptop takes far longer than the spec's 3.5 hours for the reference corpus. In that case, also try Granite at 200 tokens, which meets the literal 20-passages-a-second rule.

### Passage size: keep 350 tokens for now

- 200, 350 and 500 tokens are within about one point of each other.
- 500 scores best (combined recall@10 93.9, nDCG@10 81.6), but part of that lead comes from the test: a longer passage more often contains the whole answer.
- In its favour: 500 also stores a quarter fewer vectors, so vector search is faster.
- Against it: a result then points the reader at a longer stretch of text.

Decision: keep 350 until the evaluation set has a larger corpus, then measure again. If vector search proves too slow on the reference laptop, move to 500 first.

## Revisit if

- The reference laptop misses the indexing or query targets.
- A larger evaluation corpus separates the passage sizes.
- A new model beats Granite on this set.
