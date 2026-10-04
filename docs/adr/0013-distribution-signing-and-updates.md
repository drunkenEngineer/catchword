# ADR-13: Distribution, signing and updates

Status: accepted, a founding decision.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Unsigned builds are blocked. Who may get a signing certificate depends on the country. Updates must be trustworthy.

## Options

1. An installer with an updater in the app.
2. Microsoft Store distribution, as an MSIX package.
3. Both.

## Trade-offs

- An updater of our own means managing our own keys.
- The Store means packaging work, and the Store's policies.
- Two channels mean two builds to test at every release.

## Direction

- The Microsoft Store first, as an MSIX that Microsoft signs (Q6).
- A per-user installer on GitHub, with an updater that checks signatures, as the second channel. It stays unsigned until SignPath accepts the project, after 0.1.

## Revisit if

The packaging spike fails, or the Store refuses certification. An OV certificate is then the fallback.

## Since then

As of 4 October 2026:

- The per-user NSIS installer is built by `scripts/package-nsis.sh`. Install, update and uninstall were tested by hand once, on the owner's PC.
- The MSIX is built by `scripts/package-msix.sh` with a placeholder identity. Its files are tested, but it has never been installed.
- Waiting on the owner: Store registration and the name reservation; the update signing key, and so the updater.
- Both builds are unsigned. See `docs/packaging.md`.
