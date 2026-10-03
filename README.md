# Catchword

> **Pre-alpha.** This is the very first slice. It is not ready for real use.

Catchword finds passages in your own documents, by meaning as well as by exact words, and shows you the file and the place. **Your files never leave this computer.** There is no account and no telemetry.

## What works today

A first desktop app for Windows, and a command-line tool. Both index text, Markdown and PDF files and search them by their words and by their meaning. The desktop app: add a folder, watch it being indexed, search, preview a passage, open the file or show it in its folder. To run it, see CONTRIBUTING. The command-line tool:

```
sh scripts/fetch-pdfium.sh
sh scripts/fetch-embedding.sh
cargo build --workspace
cargo run -p catchword -- index  path/to/folder
cargo run -p catchword -- search tax refund
cargo run -p catchword -- status
```

On Windows, run the two scripts from Git Bash. They download the PDFium library, ONNX Runtime and the embedding model (about 200 MB together) and check each file against a pinned checksum.

- Search combines matches by words and by meaning, and says how each result was found. A query in one language finds passages in another.
- Indexing is in two stages: everything is searchable by words first, then by meaning as passages are embedded on your computer. An interrupted run carries on where it stopped.
- PDFs are read in a separate worker process with time and memory limits. Results show the page.
- Every file that was not indexed is listed with the reason: a scan with no text, a password, a size limit, or damage.
- Identical copies of a file are stored once.
- Moved, changed and deleted files are picked up on the next `index` run.
- Hidden folders are skipped, and links are never followed out of the folder you chose.

Meaning search is slow to build: on a 2023 laptop about 20 passages a second, so a few hours for a large library. Search quality is measured on 2,544 judged queries in English, German, French and Arabic; see `eval/` and `docs/benchmarks/`.

## What comes next

1. The rest of the desktop app: first-launch flow, settings, exclusions, and a retry for files that failed.
2. An installer, and the Microsoft Store package.

Scanned documents need OCR, which is planned for a later version. Until then they are listed as skipped, not searched.

## Privacy

The engine contains no network code, and a check in CI enforces that. The desktop app will make at most two kinds of request, both visible and switchable: an update check and a model download.

## Help test it

A tester sign-up form will be linked here. We are looking for people with large document folders, on Windows, including scans and non-English files.

## Contributing, security, licence

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md). Catchword is licensed under [Apache-2.0](LICENSE).
