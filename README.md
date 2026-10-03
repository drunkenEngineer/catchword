# Catchword

> **Pre-alpha.** This is the very first slice. It is not ready for real use.

Catchword finds passages in your own documents, by meaning as well as by exact words, and shows you the file and the place. **Your files never leave this computer.** There is no account and no telemetry.

## What works today

A command-line tool that indexes text and Markdown files and searches them by keyword:

```
cargo run --release -p catchword -- index  path/to/folder
cargo run --release -p catchword -- search tax refund
cargo run --release -p catchword -- status
```

- Identical copies of a file are stored once.
- Moved, changed and deleted files are picked up on the next `index` run.
- Hidden folders are skipped, and links are never followed out of the folder you chose.

## What comes next

1. Text extraction from PDF files, in a separate worker process.
2. Meaning-based search with a local embedding model.
3. The desktop app for Windows, distributed through the Microsoft Store.

Scanned documents need OCR, which is planned for a later version. Until then they are counted and reported, not searched.

## Privacy

The engine contains no network code, and a check in CI enforces that. The desktop app will make at most two kinds of request, both visible and switchable: an update check and a model download.

## Help test it

A tester sign-up form will be linked here. We are looking for people with large document folders, on Windows, including scans and non-English files.

## Contributing, security, licence

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md). Catchword is licensed under [Apache-2.0](LICENSE).
