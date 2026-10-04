# ADR-8: Network policy and diagnostics

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Privacy is the product. Updates and model downloads still need the network. Maintainers need some feedback.

## Options

1. No network at all.
2. Updates and downloads only.
3. Telemetry the user opts into.

## Trade-offs

- Without telemetry the maintainers cannot see usage or failures.
- An update check reveals the user's IP address.

## Direction

- Network code only in the desktop shell, and only to an allow-list of addresses.
- No telemetry.
- The user chooses at first launch whether to check for updates.
- Diagnostics are exported by hand.

## Revisit if

An opt-in, fully inspectable crash report is wanted after 1.0. Telemetry by default is never revisited.

## Since then

As of 4 October 2026:

- No network library is in the engine, store, worker, embedding runtime or service. `scripts/check-no-network.sh` checks this on every CI run.
- The desktop shell has no network code yet either. The window's content security policy allows no connection except to the app itself.
- There is no telemetry. Diagnostics are a file the user saves from Settings and can read first. Logs leave out paths, names and queries unless the user turns on detailed logs.
- The update check, and the choice at first launch, are not built. The updater waits on an update signing key from the owner.
