# ADR-23: Moving the index to a folder the user chooses

Status: accepted, 5 October 2026, by the owner. Requirement APP-7 (a SHOULD for 0.2, done early).

## Context

APP-7 lets the user keep the index wherever they choose. Section 13 gives the reason: the index copies the text of the documents, so documents kept on an encrypted drive are weakened if the index sits on an unencrypted one.

Two other parts of the specification pull against it:

- PRIV-7: uninstalling removes the index by default.
- Section 15: a Store uninstall removes all app data.

Neither uninstaller can find an index outside the app's own folders:

- The GitHub installer (NSIS) removes the index from its usual place only (`hooks.nsh`).
- Windows removes only the Store app's own container.

A moved index would be left behind: a full-text copy of private documents.

## Options

1. Allow only folders the uninstaller can find. That cannot work for the Store build.
2. Allow any folder, and warn at the move that uninstalling will not remove it there.
3. Leave APP-7 out until the index can be encrypted (PRIV-6).

## Direction

Option 2, chosen by the owner on 5 October 2026.

- **The index gets a folder of its own,** `Catchword index`, inside the folder the user picks. Nothing else in that folder is ever touched, and the folder of its own is removed when the index leaves it, if it is empty.
- **The warning comes before the move.** It says that uninstalling will not remove the index there, and that Delete all data, or deleting the folder by hand, will. It also says that search and indexing wait while that drive is not connected. If a cloud service copies the folder, the warning says so too (PRIV-4).
- **The move is a checked copy.** The index is closed, copied, read back with SQLite's quick check, and only then put in place. The settings then record the new place, and the old copy is deleted, since it holds the documents' text. If anything fails, the index stays where it was.
- **A drive that is not connected at start** is waited for, not replaced. The app opens with an empty stand-in and indexing paused (`PauseReason::IndexAway`), and a notice explains. Resume opens the index once the drive is back. Moving it back to its usual place makes a new index there, filled from the files.
- **Delete all data removes the index wherever it is,** and the next index starts in the usual place.

## Consequences

- PRIV-7 holds only for the usual place. That exception is stated to the user at the moment of the move, and in `docs/packaging.md`.
- The move is done while the user waits. A large index on a slow drive takes some time; the interface says "Moving the index…" meanwhile.

## Revisit if

- The index is encrypted (PRIV-6). A moved copy left behind would then be unreadable, which removes most of the risk.
- A way appears for the uninstallers to find the moved index.
