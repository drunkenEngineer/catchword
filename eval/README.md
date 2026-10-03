# Retrieval evaluation

A fixed set of questions with known answers that measures search quality, as the spec's section 15 asks. ADR-19 explains its design; ADR-20 records what it decided.

## What is in it

- **XQuAD** (Google DeepMind, CC BY-SA 4.0), not committed. `sh scripts/fetch-eval.sh` downloads it with pinned checksums.
  - 48 Wikipedia articles, each indexed in one language: English, German or Arabic.
  - Every question is asked in the document's own language and again in another.
  - Names and numbers found in a single paragraph are added as exact lookups.
- **`domain/`**, written for Catchword (Apache-2.0): short letters, invoices and contracts in English, French and Arabic, the kind of document Catchword is for.
  - `domain/docs/` holds the documents. Each file name starts with its language code.
  - `domain/queries.tsv` holds one query per line, with five columns separated by tabs: kind (`descriptive`, `exact` or `cross-language`), the query's language, the document, the answer, and the query. The answer must appear, word for word, on exactly one line of the document.
- **`thresholds.txt`:** the minimum scores the app's settings must keep.

A passage counts as a correct result when it comes from the right document, covers the answer's line, and contains the whole answer.

## Running it

After `sh scripts/fetch-embedding.sh` and `sh scripts/fetch-eval.sh`:

```
cargo run -p catchword-eval -- check       # the app's settings against thresholds.txt
cargo run -p catchword-eval -- run --model e5 --tokens 200
cargo run -p catchword-eval -- benchmark --out report.md
cargo run -p catchword-eval -- scale       # search speed on 250,000 synthetic passages
```

The scores are recall@10, MRR@10 and nDCG@10, each for words only, meaning only and both combined. They are broken down by kind of query, document language, script and source.
