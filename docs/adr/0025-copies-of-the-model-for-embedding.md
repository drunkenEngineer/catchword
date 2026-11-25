# ADR-25: Copies of the model embed side by side in a long meaning stage

Status: accepted by the owner, 25 November 2026. Target: embedding throughput of at least 20 passages a second (spec section 14).

## Context

Embedding missed its target: 16.7 passages a second in the default mode on the owner's laptop in October (`docs/benchmarks/2026-10-04-app.md`), 14.1 to 15.8 in the tests of 25 November. Two things had been tried:

- **More threads for one model.** Going from 8 to 15 threads gave only 7% more: one passage at a time cannot keep eight threads busy.
- **Batching** (several passages in one call). It gave nothing (section 9 of `HANDOVER.md`), and it changes the vectors of the 8-bit model, which sets its rounding scale from everything in one run.

The other way left was a faster model, which ADR-20 weighed and declined.

## Options

1. Accept the miss.
2. A faster, smaller model (declined in ADR-20: it gives worse results).
3. Several copies of the same model, each embedding its own passage at the same time, sharing the same threads.

## Measured

On the owner's laptop (16 logical cores), with eight threads in all, as the default mode allows:

| Models | Threads each | Passages a second |
| --- | ---: | ---: |
| 1 | 8 | 14.1 to 15.8 |
| 2 | 4 | 17.0 to 17.1 |
| 4 | 2 | 19.0 to 19.3 |

A temporary test embedded 120 passages of 300 words directly. Through the real code (`measure_copies_against_one_model`, 400 notes of about 250 words, loading the copies included), one model gave 15.8 a second and four copies 20.3, about 30% faster.

Memory for each copy, sampled from outside the process: the tokenizer holds about 37 MB and needs about 155 MB more while it is read; the model's runtime session holds about 120 MB. A first version loaded four full copies at once. It peaked at 1,565 MB for the test process, which would break the 1.5 GB limit for the indexing peak once the web view is added.

## Decision

Option 3, approved by the owner on 25 November 2026.

- **Only for a long stage.** Copies are made when at least 256 passages wait (`COPIES_FROM` in `crates/service/src/index.rs`). Below that, the shared model embeds alone, as before. Making a copy takes about three quarters of a second, which is not worth it for a few passages.
- **Within the mode's threads.** The mode's budget is split between the copies: at least two threads each, and four copies at most (`copies_for` in `apps/desktop/src-tauri/src/indexing.rs`).

  | Machine | Balanced (default) | Fast | Light |
  | --- | --- | --- | --- |
  | 16 cores | 4 copies on 2 threads | 4 copies on 3 threads | none |
  | 8 cores (the reference laptop) | 2 copies on 2 threads | 3 copies on 2 threads | none |
  | 4 cores | none | none | none |

  The default mode still uses at most half the logical cores, at below-normal priority, including the threads that run the copies.
- **The shared model stays free for searches.** While copies embed, the shared model is not used for indexing, so a search never waits for a passage.
- **Copies share the tokenizer.** They are made from the loaded model (the `Copier` trait in `crates/embed`), so each holds only its own runtime session. Each copy's model file is checked against its checksum before it is read, as every load is (ADR-18).
- **One copy at a time.** Copies are made one after another, asking before each whether to stop. A pause waits for one copy at most, and only one copy is ever being made at a time.
- **The same vectors.** Each passage is still embedded alone, so a copy's vector is the shared model's. Nothing needs embedding again. The embedding conformance suite (ADR-21) checks that a copy gives the same vectors.
- **Dropped at the end.** The copies go when the stage ends, so memory when idle is unchanged.
- **Failure is harmless.** A copy that cannot be made is left out; with fewer than two, the shared model embeds alone. An error while embedding ends the stage, as it did before, and the next run carries on.

With this, the test process peaked at 744 MB, against 201 MB with one model. Four copies at 2 threads embedded 20.3 passages a second, loading included.

## Consequences

- **Faster:** about 30% on the owner's laptop, and the target is met there. On a reference laptop (8 cores), the default mode gets two copies; that is still to be measured.
- **Memory while a long stage runs:** about 540 MB more for four copies. That puts the app's indexing peak at about 1.05 GB, under 1.5 GB.
- **The log says how many copies helped.** `embed.finished` records `copies`.
- **The command-line tool does not use copies.** It lets the runtime choose its threads, and has no resource modes.

## Revisit if

- The reference laptop shows that two copies are not faster than one model, or that memory runs short.
- ONNX Runtime can share one model's weights between sessions. Copies would then cost much less memory.
- The model changes. The best split between copies and threads must be measured again.
