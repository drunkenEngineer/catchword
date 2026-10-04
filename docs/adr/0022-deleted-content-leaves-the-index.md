# ADR-22: Deleted content leaves the index file

Status: accepted, 4 October 2026, pending the owner's review. Requirement PRIV-5 ("deleted content cannot be recovered from the index file", a SHOULD for 0.1).

## Context

The index copies text out of the user's documents (spec section 13). When a document is deleted, or its folder removed, its text must not stay readable in the index file. Someone who later gets a copy of that file, from a backup for example, should not be able to read it.

The store already had SQLite's `secure_delete` switched on, which overwrites freed pages with zeros. A test that looks at the raw bytes of the index file after a purge found three things on 4 October 2026:

- **The text and the vector were gone,** once the log was copied into the file.
- **The words and the file name were still there.** SQLite's full-text indexes (FTS5) do not remove a deleted entry. They add a note that it is deleted, and the entry itself stays until its part of the index is merged with another, which may be never. From the words and their positions, much of the text can be rebuilt.
- **The log file kept the old pages** (`index.db-wal`) until the app closed.

## Options

1. FTS5's own "secure-delete" option, which removes each entry in place.
2. Delete as before, then compact each full-text index ("optimize"), which rewrites it without the deleted entries.
3. Both: in place for small deletions, compaction for large ones.

## Trade-offs

Measured on the owner's laptop, release build:

- **In place is slow in bulk.** It costs about 4 ms a passage. Purging 10,000 passages took 38 s, against 1.8 s before.
- **Compaction costs the whole index, every time.** It takes about 0.06 ms per passage that is left: 2.9 s for 50,000. That is cheap for a large purge, but wasteful for a single changed file in a large index.
- So in place wins below about 1 in 60 of the passages, and compaction above that.

## Direction

Option 3.

- **The full-text indexes delete in place** (FTS5 secure-delete is switched on for both).
- **A purge that removes more than 2% of the passages works in bulk.** It switches in-place deletion off for its transaction, deletes, and commits, then compacts both full-text indexes and switches it back on. Changing how passages are cut also deletes everything, so it works the same way.
- **The switch is the record that compaction is still owed.** Each index is compacted and switched back on in one transaction. If the app stops between the purge and the compaction, the next start finds the option off and compacts then. An index made before this change is compacted once on its first start, which drops what it still held.
- **After every purge, the log is emptied into the index file** (a "truncate" checkpoint), where freed pages are overwritten.

## Measured with this in place

A library of 100,000 passages in 10,000 files (`measure_deleting`, an ignored test in `crates/store/src/lib.rs`):

| Deletion | Time |
| --- | ---: |
| One file deleted, then the scan's purge | 0.64 s |
| One file changed (its old text deleted in place) | 0.16 s |
| Half the files deleted, then compacted | 14 s |
| Everything deleted (a new way of cutting), then compacted | 11 s |

The test `a_purged_file_leaves_no_trace_in_the_index_file` checks the file's bytes for the text, two of its words, the file's name and its vector after each kind of deletion: in bulk, alone, a changed file, and a new way of cutting. It checks once while the index is open and once after it closes.

## Limits

- **A changed file's old text stays in the log until the scan ends.** The checkpoint comes with the purge at the end of each folder's scan, not after every file.
- **A search running at that moment can delay emptying the log.** It is then emptied at a later checkpoint, or when the app closes.
- **SQLite cannot reach copies outside the file.** It does not control what the disk keeps: an SSD may hold old blocks, and Windows may hold shadow copies or backups. Encrypting the index (PRIV-6, a COULD) is the answer to that.

## Revisit if

- The reference laptop makes compaction much slower than measured here. Then the 2% threshold moves.
- SQLite adds a cheaper way for full-text indexes to drop deleted entries.
