# Catchword

> **Pre-alpha.** This is the very first slice. It is not ready for real use.

Catchword finds passages in your own documents, by meaning as well as by exact words, and shows you the file and the place. **Your files never leave this computer.** There is no account and no telemetry.

## What works today

A first desktop app for Windows, and a command-line tool. Both index text, Markdown and PDF files and search them by their words and by their meaning. The desktop app: a two-step first launch, add a folder, watch it being indexed, search, preview a passage, open the file or show it in its folder; in Settings, leave out folders and names, see where the index is, and delete all data. To run it, see CONTRIBUTING. The command-line tool:

```
sh scripts/setup.sh
cargo run -p catchword -- index eval/domain/docs
cargo run -p catchword -- search "tax refund"
cargo run -p catchword -- status
```

`scripts/setup.sh` checks that Rust and Node.js are installed, then downloads PDFium, ONNX Runtime, the embedding model and the evaluation data, about 340 MB in all, and checks each file against a pinned checksum. Finally it builds everything. On Windows, run it from Git Bash. `eval/domain/docs` holds sample letters in English, French and Arabic; point `index` at any folder of your own instead.

- Search combines matches by words and by meaning, and says how each result was found. A query in one language finds passages in another. Words in quotes must appear exactly as written, and a search can keep to one folder, to PDFs or text files, or to files changed in the past week, month or year. File and folder names count too: "plumber invoice" finds `Plumber/invoice-march.pdf`. Each result shows when its file last changed.
- Indexing is in two stages: everything is searchable by words first, then by meaning as passages are embedded on your computer. An interrupted run carries on where it stopped.
- In the desktop app, indexing can be paused and resumed, and runs at low priority in one of three modes: Light (one core), Balanced (half the processor, the default) or Fast. It pauses by itself when less than 1 GB of disk space is free, and while the computer runs on battery. After ten idle minutes the meaning model is released, to give back about 140 MB of memory.
- The index can be moved to a folder you choose, for example on an encrypted drive. Deleted files leave nothing readable in it.
- PDFs are read in a separate worker process with time and memory limits. Results show the page.
- Every file that was not indexed is listed with the reason: a scan with no text, a password, a size limit, or damage. The index remembers it, so the file is not read again until it changes. A file whose reading failed gets a second try, then waits until you ask for a retry (the Library's Try again button, or `catchword index <folder> --retry`).
- Identical copies of a file are stored once.
- The index is checked at every start. A damaged one is rebuilt from your files by itself, and Settings can check it fully or rebuild it on demand. Your folders and settings are kept apart from it, so they survive.
- Moved, changed and deleted files are picked up on the next `index` run.
- Files kept only in the cloud (OneDrive and the like) are listed, never opened, so nothing is downloaded. A folder on an unplugged drive is shown as offline, and its files stay searchable.
- Text files are read in whatever encoding they were saved in: UTF-8, UTF-16, or older ones such as Windows-1252 or Windows-1256 (Arabic).
- Hidden and system files are skipped, and links are never followed out of the folder you chose.
- Some names are left out by default: system files, development folders such as `node_modules`, and files that often hold passwords or keys (`*.kdbx`, `*.pem`, `id_rsa*`, `*passwords*` and others). In the desktop app you can change the list and leave out folders; the command-line tool uses the default list.

Meaning search is slow to build: on a 2023 laptop about 17 passages a second, so a few hours for a large library. Search quality is measured on 2,544 judged queries in English, German, French and Arabic; see `eval/` and `docs/benchmarks/`.

## What comes next

1. The update check.
2. The Microsoft Store package and the GitHub installer. Both build (`docs/packaging.md`); neither is published yet.

Scanned documents need OCR, which is planned for a later version. Until then they are listed as skipped, not searched.

## Privacy

This version makes no network requests at all. The engine contains no network code, and a check in CI enforces that. Later, the download from GitHub will offer an update check, which you can turn off. [What leaves your computer](docs/user/network.md) gives the details, and how to check them yourself with the Windows firewall.

The [user guide](docs/user/guide.md) explains how to use the app (a draft).

The desktop app keeps small local logs (at most about 3 MB). They never hold document text, searches or file names; file names are recorded only while you turn on detailed logs in Settings. If the app crashes, it writes a crash report beside the logs; nothing is sent. To report a problem, Settings makes a diagnostics report that you read before saving, and share only if you choose to.

## Help test it

A tester sign-up form will be linked here. We are looking for people with large document folders, on Windows, including scans and non-English files.

## Contributing, security, licence

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md). Catchword is licensed under [Apache-2.0](LICENSE).
