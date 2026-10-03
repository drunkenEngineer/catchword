# ADR-19: The retrieval evaluation set, version 1

Status: accepted, 3 October 2026. Backlog item TST-1; section 15 of the specification.

## Context

Search quality must be measured, not guessed: the evaluation set decides the embedding model, the passage size and the fusion settings, and from now on it gates changes to search. The spec asks for at least 100 queries of three kinds (descriptive, exact, cross-language), redistributable documents in the target languages, and one right-to-left language (Arabic, Q1).

## Options

1. A public retrieval benchmark only.
2. Documents and queries written for Catchword only.
3. Both: a public set for scale and independence, a small written set for the product's own kind of documents.

## Trade-offs

- A public set gives many judged queries that nobody here wrote, so the judgements are not shaped by the code. But it is not a user's corpus: Wikipedia text, not letters and invoices.
- A written set looks like the product's documents and can hold what public sets lack: French, invoice numbers, notice periods. But it is small, and its author also wrote the code.

## Direction

Option 3.

### Public part: XQuAD

- **Source:** XQuAD (Google DeepMind, CC BY-SA 4.0): 240 Wikipedia paragraphs from 48 articles, with 1,190 questions, professionally translated in parallel. English, German and Arabic are used.
- **Corpus:** each article is a document in **one** language only: 16 English, 16 German, 16 Arabic. Otherwise a query could find the same article in its own language and never test crossing languages.
- **Descriptive queries:** every question, asked in its document's language.
- **Cross-language queries:** every question again, asked in another language. English questions find the German and Arabic documents; Arabic questions find the English ones.
- **Exact queries:** names and numbers drawn automatically from the paragraphs. Each is a word with a capital letter or a digit, at least five characters long, found in exactly one paragraph of the corpus.
- **The data is not committed:** `scripts/fetch-eval.sh` downloads it with pinned checksums, and the evaluation tool builds the set from it the same way every time.

### Written part: `eval/domain/`

- Short letters, invoices and contracts in English, French and Arabic, written for Catchword (Apache-2.0, committed).
- Queries of all three kinds, including an English query that must find a French contract (use case UC9) and invoice numbers (UC3).

### What counts as a correct result

- A passage is relevant when it comes from the right document, covers the line that holds the answer, and contains the whole answer text.
- The same rule works at every passage size. An answer split across two passages is in neither, and counts as a miss.

### Scores

- **recall@10:** the share of queries with a relevant passage in the top 10.
- **MRR@10:** the average of 1 / (rank of the first relevant passage), with 0 when there is none in the top 10.
- **nDCG@10:** binary relevance, against the best possible order of that query's relevant passages.
- Each score is reported for words only, meaning only and both combined. It is broken down by kind of query, by document language and by source.
- Thresholds are set from the first baseline (ADR-20). A change that lowers recall@10 by more than two points needs a written justification.

## Limits

- Wikipedia questions were written by people who could see the paragraph, so they share many words with it. That favours keyword search more than a user's half-remembered description would.
- The written part is small and judged by its author.
- Neither is a real user's library. The "search quality report" issue form remains the way to learn from real ones.

## Revisit if

- Users' reports show kinds of query the set lacks.
- A licensed set closer to letters, contracts and scans becomes available.
- Version 1 becomes too easy to tell candidates apart. Then add distractor documents, or a harder public set.
