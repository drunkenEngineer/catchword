# Catchword

> **Pre-alpha.** This is the very first slice. It is not ready for real use.

Catchword finds passages in your own documents, by meaning as well as by exact words, and shows you the file and the place. **Your files never leave this computer.** There is no account and no telemetry.

## What works today

A command-line tool that indexes text, Markdown and PDF files and searches them by their words and by their meaning:

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

Meaning search is slow to build: on a 2023 laptop about 13 passages a second, so several hours for a large library. The model choice and passage size are still provisional.

## What comes next

1. An evaluation set and benchmark, to choose the model and passage size by measurement.
2. The desktop app for Windows, distributed through the Microsoft Store.

Scanned documents need OCR, which is planned for a later version. Until then they are listed as skipped, not searched.

## Privacy

The engine contains no network code, and a check in CI enforces that. The desktop app will make at most two kinds of request, both visible and switchable: an update check and a model download.

## Help test it

A tester sign-up form will be linked here. We are looking for people with large document folders, on Windows, including scans and non-English files.

## Contributing, security, licence

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md). Catchword is licensed under [Apache-2.0](LICENSE).
