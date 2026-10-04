# ADR-11: Answer generation

Status: **proposed**. The runtime stays open until it is confirmed before 0.4.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Answers are an optional feature. They bring the heaviest packaging, and a real risk of wrong answers in professional use.

## Options

1. A bundled llama.cpp worker with one curated model.
2. A local server the user already runs.
3. No answers at all.

## Trade-offs

- Bundling serves people who are not technical, but multiplies the builds.
- An external server is cheap to support, but needs setting up.

## Direction

- Built in, with the model downloaded the first time answers are switched on (Q16).
- A "use my own local server" option can follow. It would be restricted to loopback addresses and off by default.
- The runtime is confirmed before 0.4.
- Never a cloud provider.

## Revisit if

- Small permissive models improve markedly.
- Operating systems make local models widely available.

## Since then

Nothing is built, as of 4 October 2026. Answers are planned for 0.4.
