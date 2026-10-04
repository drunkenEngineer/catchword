# ADR-12: Licence

Status: accepted, 3 October 2026 (Q4).

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Trust and adoption both depend on the licence, and it is hard to change later.

## Options and trade-offs

See section 17 of the specification.

## Direction

Apache-2.0. As a result, GPL and AGPL libraries are excluded.

## Revisit if

In practice never, once there are outside contributions.

## Since then

As of 4 October 2026:

- `LICENSE` and `NOTICE` are at the root of the repository.
- The licences that shipped Rust libraries may have are listed in `about.toml`. Any library under another licence stops the notices step (`scripts/notices.mjs`), so a new licence is always a decision, never an accident.
- The notices are shipped with the app and shown in Settings, under About.
