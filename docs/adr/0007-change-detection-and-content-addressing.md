# ADR-7: Change detection and content addressing

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Files move, are copied, and change while the app is closed. Drives disconnect. Change notifications can be lost.

## Options

1. Notifications only.
2. Scans only.
3. Scans plus notifications.
4. The NTFS change journal.

## Trade-offs

- Scans take time on large folder trees.
- Hashing costs a full read of each file.
- Content addressing adds a level of indirection.

## Direction

The scan is the source of truth. Notifications speed it up from 0.2. Computed data is keyed by a hash of the content.

## Revisit if

Scans of real libraries take more than a few minutes.

## Since then

As of 4 October 2026:

- Every indexing run scans the folders. A file whose size and modified time are unchanged is skipped. Any other file is hashed, and if its content is already indexed (a copy, or a move) its passages are reused, not read again.
- A scan of an unchanged library of 10,000 files takes 0.4 s on the owner's laptop (`docs/benchmarks/2026-10-04-app.md`).
- Notifications (0.2) and the NTFS change journal are not built.
