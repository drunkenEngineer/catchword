# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/) and Semantic Versioning.

## Unreleased

Nothing has been released yet. This is what the first release, 0.1, will hold so far.

### Added

- A desktop app for Windows, and a command-line tool, that index text, Markdown and PDF files and search them by their words and by their meaning, across languages. Nothing leaves the computer.
- Search: results grouped by file, with the passage, its page or lines, how it was found and when the file last changed. Quoted phrases. Filters by folder, kind of file and date. A preview, with Open, Show in folder, Copy passage and Copy path. Results by words appear first when the full search is slow.
- Indexing in two stages, words first and then meaning. It can be paused and resumed, carries on after an interruption, and pauses by itself on low disk space and on battery.
- Library: folders and their state, progress with time left, and every file that was not indexed, with the reason and a way to try again.
- Settings: what to leave out, resource use, file limits, appearance, moving the index to another folder, checking, rebuilding and deleting it, and a diagnostics report to read before saving.
- PDFs are read in a separate process with time and memory limits. Damaged and hostile files end as a reason, never as a crash of the app.
- A damaged index is rebuilt by itself; after two unexpected closes in a row, indexing waits (safe mode).
- Full keyboard use, screen-reader announcements, WCAG AA colour contrast, and support for Windows contrast themes and reduced motion.
- A per-user installer for GitHub, and a package for the Microsoft Store, neither published yet.
- Documentation: a user guide, what leaves your computer and how to check it, an architecture overview, a guide to adding a file format, and this release process.
