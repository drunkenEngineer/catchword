# Private Document Search: Software Product & Architecture Specification

Oct 3, 2026 · @Sadir Abdelmounam

## 1. Executive Summary

The idea is worth building. The recommendation is a search-first Windows desktop app on Tauri 2 with a Rust engine, one rebuildable SQLite index and file parsing in an isolated process. A public 0.1, signed through the Microsoft Store, is an estimated 19 to 28 part-time weeks away. At the agreed 10 hours a week, plan on the upper end.

### What is being built

A free, open-source app that finds passages in a person's own documents by meaning and by exact words, shows the file and page, and sends no document data anywhere. The primary user is a non-technical professional who cannot upload client files.

### The architecture in five lines

- **Shell and engine.** A Tauri 2 shell around a Rust engine, with a thin TypeScript interface.
- **Storage.** One SQLite file holds text, keyword index and vectors. It is derived data and can always be rebuilt. Settings live apart from it.
- **Containment.** Files are parsed in a separate, limited worker process, because parsing untrusted files is the main security risk.
- **Search.** Keyword and meaning search are combined from the first release. A file is keyword-searchable minutes before it is meaning-searchable.
- **Privacy by structure.** No server, no open port, no telemetry. Network code exists only in the shell, for update checks and model downloads.

### Where this specification disagrees with the idea

1. **Timeline.** Four to six weeks buys a prototype. A public 0.1 is 19 to 28 part-time weeks and 1.0 is 12 to 18 months. Both are estimates.
2. **The headline example needs OCR.** A scanned letter is not searchable until 0.3. Version 0.1 must detect scans and say so.
3. **"Nothing ever leaves the machine"** becomes a precise, tested statement. No document, query or usage data ever leaves. Update checks and model downloads are visible and switchable.
4. **Two additions to 0.1.** A coverage view listing every skipped file with its reason, and an update path from day one: the Store, or a signature-verified updater in the GitHub build.
5. **Three choices wait for measurement.** The embedding model, the OCR engine, and whether a reranker ships at all.
6. **Email leaves 1.0** unless it is narrowly defined.

### The competitive position

Local meaning-based search already exists: inside Windows on Copilot+ PCs, in Hyperlink, and in AnythingLLM. This product has no technical moat. It wins, if it does, on focus (search, not chat), zero setup on ordinary hardware, and privacy a user can verify.

### What decides success

- **Retrieval quality.** It is unproven until the Phase 0 benchmark on real documents. Everything else is secondary to that result.
- **One maintainer.** The plan assumes one part-time developer. Strict scope and a second person with release rights before 1.0 are the mitigations.

### Decisions made and still open

Most are made (section 26): a multilingual model, Apache-2.0, an individual publisher distributing through the Microsoft Store first, and a public repository. Three remain with the owner: the two-week Rust test, the product name (Catchword is recommended), and the ten testers. Section 26 gives each a solution and a fallback.

## 2. Product Definition

The product is a search engine for a person's own documents that matches meaning as well as words and runs entirely on their computer. "Private Document Search" is a working title; the name is an open question.

### Problem

People cannot find documents whose name and wording they have forgotten. Built-in search matches file names and exact words. Cloud AI finds by meaning but requires uploading contracts, medical records and client files, which many professionals may not do.

### Value proposition

Describe what you are looking for and get the passage, the file and the page in under a second, with no upload, no account and no setup.

### Existing solutions

| Alternative | What it offers | Gap for our primary user |
| --- | --- | --- |
| [Improved Windows search](https://www.microsoft.com/en-us/windows/learning-center/find-files-fast-with-improved-search) | Local meaning-based search built into Windows 11 for common document and image types | Only on Copilot+ PCs with an NPU; optimized for six languages |
| [Hyperlink by Nexa AI](https://www.producthunt.com/products/hyperlink-by-nexa-ai) | Free on-device assistant for Windows and macOS that indexes folders and returns cited answers | Closest in promise. Answer-first, not search-first; whether its source is open was not verified |
| [AnythingLLM Desktop](https://anythingllm.com/alternatives/gpt4all) | MIT-licensed, actively released; document workspaces with citations, OCR, Office formats, agents, 40+ model providers | A general AI workbench: the user picks models and manages workspaces, and privacy depends on configuration |
| GPT4All LocalDocs | MIT-licensed app that made "chat with a folder" popular | Last release February 2025 according to [AnythingLLM's comparison](https://anythingllm.com/alternatives/gpt4all), a competitor's account |
| Keyword desktop search (Everything, Recoll, DocFetcher, dtSearch) | Fast and mature exact-word search | No matching by meaning |
| Cloud assistants (Microsoft 365 Copilot, NotebookLM) | Best answer quality | Require upload, which the premise rules out |

Linked rows were checked on 3 October 2026. Unlinked rows come from general knowledge and were not re-checked.

### What makes this meaningfully different

1. **Search-first.** Ranked passages with file and page, fast. Generated answers are an optional layer, never the main interface.
2. **Zero setup on ordinary hardware.** One installer with the model inside. Works on a CPU-only laptop with 8 GB of RAM.
3. **Verifiable privacy.** Open source, no account, no telemetry. Network code lives in one small module, and the app works with the network disabled.
4. **Honest coverage.** Every file that was skipped is listed with the reason.

None of these is a technical moat. Every competitor can use the same models and libraries. The advantage is focus and trust, and it must be earned through retrieval quality and reliability.

### Where the original idea needs correcting

| Claim in the idea | Problem | Corrected statement |
| --- | --- | --- |
| "Search every file on their computer" | The feature list says the user picks folders. Whole-disk indexing is a different scale. | Search the folders you choose, up to a stated corpus size |
| "Nothing ever leaves the machine" | Update checks and model downloads are network calls | No document content, query or usage data ever leaves. The only network calls are update checks and model downloads, both visible and switchable |
| "The letter about my tax refund" in `scan_0042.pdf` | A scan has no text. OCR is planned for 0.3, so the headline example fails until then | 0.1 detects scans and says so; scans become searchable in 0.3 |
| "Auto-update" | Names index refresh, but users will read it as app updates | Two separate features: index refresh and app updates |
| "4 to 6 weeks to 0.1" | Realistic for a working prototype, not for a signed, installable public release | See section 20 |

### Major assumptions

- **A1.** One user per machine. No shared or server-hosted index.
- **A2.** Files live on local or external drives. Network shares and cloud-only files are best-effort.
- **A3.** Reference corpus is 10,000 documents (about 250,000 passages) for 0.1 and 50,000 documents by 1.0.
- **A4.** One maintainer at about 10 hours a week, willing to work in Rust.
- **A5.** Free forever, with no revenue requirement. Running costs can be zero, because the Microsoft Store signs packages for free.
- **A6.** Users accept an initial indexing run that takes hours, provided search works meanwhile.
- **A7.** Windows 11 on x64 is the first platform.
- **A8.** Users' documents are in more than one language.

A3 and A8 are confirmed in section 26. A4 is tested by a two-week Rust trial at the start of Phase 0.

## 3. Target Users

The primary user is a non-technical professional who holds confidential documents and cannot upload them. Every design choice below is tested against that person: one installer, no model picking, no command line.

| Persona | Corpus (assumed, to validate) | Needs most | Blocked today by |
| --- | --- | --- | --- |
| **Primary: solo or small-firm professional** (lawyer, accountant, consultant) | Thousands to tens of thousands of client files; PDF and Office; many scans | Find a clause, letter or invoice by description; proof of source (file and page) | Confidentiality duties rule out cloud AI; built-in search only matches names and exact words |
| Researcher or student | Papers in PDF, notes in Markdown and text | Find the passage that made a claim; often several languages | Reference managers search metadata, not meaning |
| Journalist | Mixed formats, scans, sensitive source material | Source protection; fully offline operation | Cannot risk any upload; often works on isolated machines |
| Freelancer or household archivist | Years of contracts, invoices, tax letters, phone scans | Find "that letter" without remembering its name | Files named `scan_0042.pdf`; older, CPU-only hardware |
| Secondary: IT administrator at a small firm | Deploys to 5 to 50 machines | Silent install, no telemetry, policy control | Not served before 1.0; see section 24 |

**Not the target.** Developers searching source code, teams that need a shared index on a server, and people who want a general chatbot. Serving them would pull the design toward a different product.

**Consequence for scope.** The primary persona's files are heavily scanned. That makes OCR more central than the original build order assumes; see sections 5 and 26.

## 4. Core Use Cases

Nine use cases define the product. Eight are served by the first release and one arrives in 0.4.

| # | Use case | Example | First supported |
| --- | --- | --- | --- |
| UC1 | Find a document by describing it | "the letter about my tax refund" | 0.1 for PDFs with a text layer; 0.3 for scans |
| UC2 | Find the passage inside a long document | "termination notice period" across 40 contracts | 0.1 |
| UC3 | Exact lookup | An invoice number, a surname, a quoted phrase | 0.1 |
| UC4 | Verify and cite | See the passage, file and page; open the file; copy the passage with its source | 0.1 |
| UC5 | Stay current without effort | A file saved today is searchable today | 0.1 on launch and on demand; 0.2 live |
| UC6 | Understand coverage | "Why can't I find it?" answered by a list of skipped files and reasons | 0.1 |
| UC7 | Remove data | Stop indexing a folder and purge its text from the index | 0.1 |
| UC8 | Ask a question, get a cited answer | "What notice period did we agree with Acme?" | 0.4 |
| UC9 | Search across languages | Query in English, find a French contract | 0.1, with the multilingual model; quality confirmed by the Phase 0 benchmark |

UC3 is why meaning-based search alone is not enough. Embedding models are weak at identifiers and rare names, so keyword and meaning search must be combined from the first release.

UC6 is not in the original idea and is added deliberately. A search tool that silently skips files loses trust the first time a known document fails to appear.

## 5. MVP Scope

Version 0.1 must prove one thing: hybrid search over a person's own PDFs and text files returns the right passage on an ordinary Windows laptop, with nothing leaving it.

### In scope for 0.1

- **Platform.** Windows 11 on x64. Distributed through the Microsoft Store, which signs the package. A per-user installer on GitHub is unsigned until SignPath signing is granted. Runs as a standard user.
- **Sources.** The user picks folders and exclusions. A scan runs on launch and on demand.
- **Formats.** PDF with a text layer, plain text, Markdown.
- **Search.** Keyword and meaning search combined. Results grouped by file, each with passage, file name and location (page for PDFs).
- **Verify.** Preview of the passage in context, open the file, reveal it in Explorer, copy the passage with its source.
- **Indexing.** Two stages: keyword-searchable within minutes, meaning-searchable as embedding completes. Pause, resume, and survive a restart.
- **Coverage.** A status view with progress, counts, and every skipped file with its reason.
- **Removal.** Removing a folder purges its text. One action deletes all index data.
- **Updates.** Store installs update through Windows. The GitHub build has its own updater, verified by signature, that asks before installing.
- **Diagnostics.** Local logs and a redacted diagnostics export. No telemetry.

### Explicitly out of 0.1

| Excluded | Why | Planned |
| --- | --- | --- |
| OCR for scans and images | Large dependency and tuning effort | 0.3; ordering is an open question |
| Word, Excel, PowerPoint | Three more parsers and their test fixtures | 0.2 |
| Live file watching | The launch scan already guarantees correctness | 0.2 |
| Question answering with a local model | Depends on good retrieval first; heavy packaging | 0.4 |
| Reranking model | Must earn its latency in measured evaluation | 0.2 at the earliest |
| macOS and Linux builds | Signing and packaging cost per platform | 1.0; core stays portable from day one |
| Email | "Email support" is undefined: files, archives or live mailboxes | After 1.0 unless narrowed |
| Legacy `.doc`, `.xls`, `.ppt` | Binary formats with poor library support | Unscheduled |
| GPU or NPU acceleration | Per-vendor packaging; CPU is the guaranteed path | After 0.2, if indexing time is the top complaint |
| Index encryption at rest | OS disk encryption covers the main threat | Candidate for 1.0 |
| Global hotkey, tray quick search, search history | Polish, not proof | 0.2 |
| Translated interface | Strings are externalised in 0.1; translations follow | 1.0 |
| Any cloud model or remote API | Breaks the core promise | Never |
| Telemetry or automatic crash upload | Breaks the core promise | Never |
| Shared or multi-user index | A different product | Never in this app |

### Changes to the original build order

1. **The launch scan moves into 0.1.** The first index and later change detection are the same code path. Only live watching waits for 0.2.
2. **The coverage view is added to 0.1.** Without it, users cannot tell a bad search from an unindexed file.
3. **An update path ships with the first public build: the Store, or a signature-verified updater in the GitHub build.** Software that parses untrusted files needs a way to deliver fixes.
4. **OCR may need to precede Office formats.** It moves ahead if scans exceed about a quarter of a typical tester's library (Q9).
5. **Email leaves 1.0** unless it is defined as "index `.eml` and `.mbox` files found in chosen folders".

### MVP exit criteria

- Retrieval quality meets the thresholds set after the Phase 0 baseline (section 15).
- 95% of queries return in under 500 ms at 250,000 passages on the reference laptop (section 14).
- Ten outside testers index their own folders without help.
- A test proves no network connection occurs with the update check off.

## 6. Functional Requirements

Thirty of the 54 requirements below are MUST for 0.1. Priority is relative to the MVP; the Release column says when a deferred item is planned.

### Sources and scanning

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| SRC-1 | Add and remove folders to index through a folder picker | MUST | 0.1 |
| SRC-2 | Exclude sub-folders and file patterns; ship a default exclusion list covering system, hidden, development and credential files | MUST | 0.1 |
| SRC-3 | Scan chosen folders on launch and on demand; detect added, changed, moved and deleted files | MUST | 0.1 |
| SRC-4 | Skip cloud-only placeholder files without triggering a download | MUST | 0.1 |
| SRC-5 | Mark a folder offline when its drive is unreachable and keep its index entries | MUST | 0.1 |
| SRC-6 | Never follow links or junctions that lead outside a chosen folder | MUST | 0.1 |
| SRC-7 | Apply configurable size and page limits per file | SHOULD | 0.1 |
| SRC-8 | Watch chosen folders and index changes within one minute | WON'T (MVP) | 0.2 |

### Extraction

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| EXT-1 | Extract text and page numbers from PDFs that have a text layer | MUST | 0.1 |
| EXT-2 | Extract plain text and Markdown, detecting the character encoding | MUST | 0.1 |
| EXT-3 | Detect PDFs without a text layer and report them as "needs OCR" | MUST | 0.1 |
| EXT-4 | Detect encrypted or password-protected files and report them as skipped | MUST | 0.1 |
| EXT-5 | Contain failures: a file that crashes or hangs extraction is recorded as failed and indexing continues | MUST | 0.1 |
| EXT-6 | Extract `.docx`, `.xlsx` and `.pptx` with heading, sheet and slide locations | WON'T (MVP) | 0.2 |
| EXT-7 | Recognise text in scanned PDFs and images (OCR) | WON'T (MVP) | 0.3 |
| EXT-8 | Extract `.rtf`, `.html`, `.epub`, `.odt` and `.csv` | COULD | 0.2 or later |
| EXT-9 | Index `.eml` and `.mbox` files found in chosen folders | COULD | After 1.0 |

### Indexing

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| IDX-1 | Split text into passages sized to the embedding model's limit, keeping page and position | MUST | 0.1 |
| IDX-2 | Make text keyword-searchable as soon as it is extracted, before embedding | MUST | 0.1 |
| IDX-3 | Compute embeddings locally with a model shipped in the installer | MUST | 0.1 |
| IDX-4 | Resume after a restart or crash without redoing completed work | MUST | 0.1 |
| IDX-5 | Pause and resume indexing; choose a resource mode | MUST | 0.1 |
| IDX-6 | Purge all text and vectors when a file is deleted or a folder is removed | MUST | 0.1 |
| IDX-7 | Rebuild the index on demand, and automatically after unrecoverable corruption | MUST | 0.1 |
| IDX-8 | Reuse extracted text and embeddings for identical, moved or renamed files | SHOULD | 0.1 |
| IDX-9 | Pause automatically on battery power | SHOULD | 0.2 |

### Search

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| SEA-1 | Return ranked results from keyword and meaning search combined, including matches on file and folder names | MUST | 0.1 |
| SEA-2 | Show the passage, file name, folder, location and modified date for each result, with right-to-left text in its natural direction | MUST | 0.1 |
| SEA-3 | Group passages by file so one long document cannot flood the list | MUST | 0.1 |
| SEA-4 | Search while indexing runs, and state that results are partial | MUST | 0.1 |
| SEA-5 | Support quoted exact phrases | SHOULD | 0.1 |
| SEA-6 | Filter by folder and file type; by modified date later | SHOULD | 0.1 |
| SEA-7 | Show whether a result matched by keyword, by meaning or both | COULD | 0.2 |
| SEA-8 | Rerank the top results with a dedicated model, if evaluation justifies it | COULD | 0.2 |
| SEA-9 | Keep recent searches locally, clearable and switchable | COULD | 0.2 |

### Results and verification

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| RES-1 | Preview the passage with its surrounding text | MUST | 0.1 |
| RES-2 | Open the file in its default application; reveal it in Explorer | MUST | 0.1 |
| RES-3 | Copy the passage with its source (file name and location); copy the path | SHOULD | 0.1 |
| RES-4 | Open a PDF at the matching page | COULD | Depends on the viewer; unscheduled |

### Coverage

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| COV-1 | Show progress for both indexing stages, with counts and estimated time | MUST | 0.1 |
| COV-2 | List skipped and failed files with a reason and a retry action | MUST | 0.1 |
| COV-3 | Show index size on disk and where the data is stored | SHOULD | 0.1 |

### Answers

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| ANS-1 | Answer a question from retrieved passages with a local model, citing each passage used | WON'T (MVP) | 0.4 |
| ANS-2 | Say so plainly when the documents do not contain the answer | WON'T (MVP) | 0.4 |
| ANS-3 | Download the answer model only on request, after checking available memory, showing size, licence and checksum | WON'T (MVP) | 0.4 |

### Application

| ID | Requirement | Priority | Release |
| --- | --- | --- | --- |
| APP-1 | First-launch flow: privacy statement, folder choice, and in the GitHub build the update-check choice | MUST | 0.1 |
| APP-2 | GitHub build: check for updates, verify the signature, install with consent; can be switched off. Store build: updated by Windows | MUST | 0.1 |
| APP-3 | Settings for folders, exclusions, resources, updates, data location and appearance | MUST | 0.1 |
| APP-4 | Delete all index data from inside the app | MUST | 0.1 |
| APP-5 | Export a redacted diagnostics bundle the user can read before sharing | SHOULD | 0.1 |
| APP-6 | Keep indexing in the background, with a tray icon, when the window is closed | SHOULD | 0.2 |
| APP-7 | Move the data directory to a location the user chooses | SHOULD | 0.2 |
| APP-8 | Global hotkey to summon the search window | COULD | 0.2 |
| APP-9 | Managed settings for firm-wide deployment; portable mode | COULD | After 1.0 |

**On "page".** Only paginated formats have a page. The location is a page for PDF, a slide for PowerPoint, a sheet for Excel, a heading for Word and Markdown, and a line for plain text. Word files have no stable page numbers without rendering them.

## 7. Non-Functional Requirements

The non-functional requirements that shape the architecture most are privacy, containment of bad files, and resumable indexing. Performance numbers live in section 14 and are referenced here, not repeated.

| Area | ID | Requirement | Priority |
| --- | --- | --- | --- |
| Performance | PERF-1 | Meet the startup, query and indexing targets in section 14 on the reference laptop | MUST |
| Performance | PERF-2 | The interface never waits on indexing, search or file access; long work runs off the interface thread | MUST |
| Performance | PERF-3 | Benchmarks run on a schedule; a regression over 15% blocks a release | SHOULD |
| Reliability | REL-1 | A crash, power loss or forced quit never damages settings and needs at most an automatic resume or rebuild | MUST |
| Reliability | REL-2 | A malformed file cannot crash or hang the app | MUST |
| Reliability | REL-3 | After a completed scan, no result points to a deleted file | MUST |
| Reliability | REL-4 | The app never modifies, moves or deletes user documents | MUST |
| Reliability | REL-5 | Updates preserve settings and index; an older app refuses a newer index cleanly | MUST |
| Security | SEC-1 | File parsing runs in a separate process with time and memory limits | MUST |
| Security | SEC-2 | The app opens no listening network port | MUST |
| Security | SEC-3 | The interface treats all document text as untrusted and renders it as plain text | MUST |
| Security | SEC-4 | Runs without administrator rights and never requests elevation | MUST |
| Security | SEC-5 | Updates and downloaded models are verified by signature or checksum before use | MUST |
| Security | SEC-6 | The parsing process runs with reduced operating-system privileges | SHOULD |
| Privacy | PRIV-1 | No document content, query, file name or usage data is ever sent anywhere | MUST |
| Privacy | PRIV-2 | Network access is limited to update checks and model downloads, to a fixed host list, from one module | MUST |
| Privacy | PRIV-3 | Logs contain no document text and no queries; file paths appear only in opt-in debug logging | MUST |
| Privacy | PRIV-4 | Index data lives in the non-roaming user profile; the app warns if it is moved into a synced folder | MUST |
| Privacy | PRIV-5 | Deleted content cannot be recovered from the index file | SHOULD |
| Privacy | PRIV-6 | The index is encrypted at rest with a key protected by the user's Windows account | COULD |
| Privacy | PRIV-7 | Uninstalling removes the index by default | MUST |
| Accessibility | A11Y-1 | Every function works by keyboard alone, with visible focus | MUST |
| Accessibility | A11Y-2 | Works with Narrator and NVDA; result counts and progress are announced | MUST |
| Accessibility | A11Y-3 | Meets WCAG 2.2 level AA; respects Windows contrast themes, text scaling and reduced motion | SHOULD |
| Accessibility | A11Y-4 | Interface strings are externalised and the layout supports right-to-left languages | SHOULD |
| Maintainability | MNT-1 | The engine builds and passes its tests without the desktop shell | MUST |
| Maintainability | MNT-2 | Extractors, index store and embedding runtime each sit behind an interface with a shared conformance test suite | MUST |
| Maintainability | MNT-3 | Every dependency passes automated licence and advisory checks | MUST |
| Maintainability | MNT-4 | Architecture decisions are recorded as ADRs in the repository | SHOULD |
| Maintainability | MNT-5 | A new contributor builds and runs from a clean clone in under 30 minutes | SHOULD |
| Portability | PORT-1 | Windows 11 on x64 is supported and tested | MUST |
| Portability | PORT-2 | A native Windows 11 ARM64 build is supported and tested; until then the x64 build runs there under emulation | SHOULD |
| Portability | PORT-3 | Windows 10 22H2 works on a best-effort basis: a smoke check before each release, no promise | COULD |
| Portability | PORT-4 | Operating-system-specific code is confined to adapter modules | MUST |
| Portability | PORT-5 | Engine tests run on Linux and macOS in CI from the first commit | SHOULD |
| Observability | OBS-1 | Rotating, size-capped local logs with levels | MUST |
| Observability | OBS-2 | Index health is visible in the app: queue, throughput, failures, last scan | MUST |
| Observability | OBS-3 | Crash reports are written locally; nothing is uploaded automatically | MUST |
| Offline | OFF-1 | Every feature except update checks and model downloads works with no network | MUST |
| Offline | OFF-2 | A network failure never interrupts the user; update checks fail quietly and retry later | MUST |
| Resources | RSC-1 | Stay within the memory, processor and disk budgets in section 14 | MUST |
| Resources | RSC-2 | Background indexing runs at below-normal priority and yields to foreground apps | MUST |
| Resources | RSC-3 | Stop indexing and warn when free disk space falls below a threshold | MUST |
| Resources | RSC-4 | Installer under 200 MB including the embedding model | SHOULD |

Authentication and authorization are deliberately absent. This is a single-user local app that relies on the Windows account; adding a login would add risk without protecting anything.

## 8. UX Architecture

The app is one window with three destinations, and the search box is always the starting point. It should behave like a native Windows tool: keyboard-first, immediate and quiet.

### Principles

- **Search is home.** No dashboard and no chat screen.
- **Useful within a minute.** Search works on whatever is indexed so far.
- **Always show the source.** No result appears without its file and location.
- **Explain absence.** When nothing is found, say what is not indexed and why.
- **Quiet success.** Success is a brief inline confirmation, never a dialog.

### Interface hierarchy

- **Main window**
  - Title bar with an index status chip that opens Library
  - Left rail: Search, Library, Settings
- **Search**
  - Search box, with a Search or Ask switch from 0.4
  - Filter bar: folder, file type, date
  - Results list: one row per file, matching passages nested beneath
  - Preview pane: the passage in context, with Open, Reveal and Copy
  - Status line: result count, elapsed time, partial-index notice
- **Library**
  - Folders, each with a state: ready, scanning, offline
  - Progress for both stages: keyword-ready and meaning-ready
  - Needs attention: skipped and failed files grouped by reason, with retry
  - Index facts: passages, size on disk, last scan
- **Settings**
  - Folders and exclusions
  - Indexing: resource mode, file limits, battery behaviour
  - Privacy and network: update check, network activity statement, delete all data
  - Appearance: theme, text size, language
  - About: version, licences, diagnostics export
- **First-launch wizard**, two or three steps, shown once

### Main flows

1. **First launch.** Privacy promise in plain words: "your files never leave this computer", not "this app never goes online". Choose folders. In the GitHub build, choose whether to check for updates. Indexing starts and the Search screen opens. Target: first search within 60 seconds of install.
2. **Search and verify.** Type, arrow through results, read the preview, press Enter to open the file.
3. **"Why can't I find it?"** The no-results state lists likely causes with counts, for example "212 scanned PDFs are not searchable yet", and links to Library.
4. **Change sources.** Add a folder and indexing begins. Remove a folder and its text is purged after one confirmation.
5. **Ask (0.4).** An answer card streams above the results with numbered citations. Selecting a citation highlights its passage. It is a single question and answer, not a conversation.

### States

| Where | State | What the user sees |
| --- | --- | --- |
| Search | Empty, no folders | One prompt and one button: add a folder |
| Search | Empty, ready | Focused search box, three example queries, one-line index summary |
| Search | Indexing | Results work; notice "Indexed 1,240 of 8,300 files. Results may be incomplete." |
| Search | Loading | Keyword results first; placeholders only if a query passes 300 ms |
| Search | No results | Likely causes with counts; actions to clear filters or open Library |
| Search | Index repairing | "Search returns when the rebuild reaches the keyword stage", with progress |
| Library | Working | Two progress bars, throughput, estimated time, Pause |
| Library | Paused | The reason (you, battery, low disk) and Resume |
| Library | Folder offline | "Drive not connected. Its files stay searchable but cannot be opened." |
| Library | Up to date | Time of last scan and totals |
| Open file | File moved or deleted | Explanation and an offer to rescan the folder |
| App | Update available | Non-blocking notice with version and notes; installs on restart |
| App | Index unreadable | Automatic rebuild starts; settings and folder list are intact |
| Any | Success | A brief inline confirmation such as "Copied" |

### Keyboard

| Action | Shortcut |
| --- | --- |
| Focus the search box | Ctrl+K or Ctrl+L |
| Move through results | Up, Down |
| Expand or collapse a file's passages | Right, Left |
| Open the file | Enter |
| Reveal in Explorer | Ctrl+Enter |
| Copy passage with source | Ctrl+C |
| Copy file path | Ctrl+Shift+C |
| Move between panes | F6 |
| Rescan now | F5 |
| Search, Library, Settings | Ctrl+1, Ctrl+2, Ctrl+3 |
| Clear the query | Esc |
| Summon from anywhere (0.2) | Configurable global hotkey |

### Accessibility

- The results list is one tab stop with arrow-key navigation. Each item announces file name, location and passage.
- Result counts and indexing progress are announced politely and rate-limited.
- Status never relies on colour alone; each state has an icon and text.
- The app honours Windows contrast themes, text scaling to 200% and reduced motion.
- Focus returns to the search box when a dialog closes.
- Each release gets a manual pass with Narrator and NVDA in addition to automated checks.

### Desktop manners

The interface runs in a web view but must not feel like a web page. That means remembered window size and position, a single running instance, native file dialogs and context menus, the system font and theme, no loading screens between views, and no controls that appear only on hover.

## 9. Technical Architecture

The app is a Rust engine inside a thin desktop shell: a few local processes, one database file, no server and no open port.

### Process model

| Process | Runs | Trust | Why it is separate |
| --- | --- | --- | --- |
| Shell and engine | Window, tray, command handlers, scheduler, database, search, embedding | Trusted | The single owner of the index and the only writer |
| Web view | The interface only | Limited: it displays untrusted text | Has no file or network access; reaches the shell through a fixed command list |
| Extraction worker | Parses one file at a time | Untrusted input, so untrusted output | A bad file can be killed without harming the app |
| Answer worker (0.4) | The local language model | Trusted code, large memory | Starts on demand and exits when idle to return memory |

There is no Windows service. Indexing runs while the app runs, in the window or from the tray. A service would need administrator rights and would widen the attack surface for little gain.

### Layers

| Layer | Responsibility | May depend on |
| --- | --- | --- |
| Interface (TypeScript) | Screens, view state, accessibility | The command contract only |
| Shell (Tauri, Rust) | Window, tray, dialogs, updater, model download, command handlers | Engine API |
| Engine: application | Use cases: manage sources, scan, schedule indexing, search, report coverage | Domain, ports |
| Engine: domain | File states, chunking rules, rank fusion, exclusion matching. Pure logic, no input or output | Nothing |
| Engine: ports | Interfaces for extractor, index store (keyword and vector search under one transaction), embedding runtime and file system | Domain |
| Adapters | SQLite store, PDF library binding, ONNX Runtime binding, Windows file APIs, worker supervisor | Ports |

### Seven rules that hold the design together

1. **Dependencies point inward.** The engine never references the shell or the interface.
2. **Network code lives only in the shell's update and download module.** A CI check fails if an engine component links a network library.
3. **Untrusted bytes are parsed only in workers.** The engine validates everything a worker returns.
4. **The index is derived data.** Everything in it can be rebuilt from the user's files. Settings are stored apart from it.
5. **One writer.** A single indexing coordinator owns all writes. Searches read concurrently.
6. **Every long operation is a resumable job.** Jobs are recorded in the database and are safe to repeat.
7. **The interface speaks in IDs.** It never sends a file path to be read or opened. Folders are chosen in a native dialog that the shell opens.

### Cross-cutting concerns

- **Concurrency.** A bounded pipeline with back-pressure: scan, extract, chunk, keyword index, embed, vector index. A user's query always pre-empts background embedding.
- **State.** The database is the source of truth for index state. The interface holds only view state and subscribes to engine events for progress.
- **Inter-process communication.** Interface to shell: Tauri commands and events, with TypeScript types generated from the Rust definitions. Shell to worker: length-prefixed, versioned messages over standard input and output.
- **Configuration.** One versioned, validated settings file. A bad file is backed up and defaults are used, with a visible notice.
- **Caching.** Extracted text is kept, keyed by content hash, so re-chunking or re-embedding never re-parses a file. The model stays loaded while the app is in use.
- **Errors.** A per-file failure is data, stored with a reason code and shown in Library. User-facing errors carry a plain message and an action.
- **Logging.** Structured, local, rotated. No remote error reporting.
- **Authentication and external APIs.** None, by design.

## 10. Technology Decisions

Fourteen technology choices are made and three are deliberately left open until a measurement can decide them.

| Area | Choice | Reason | Alternatives considered | Trade-offs |
| --- | --- | --- | --- | --- |
| Desktop shell | Tauri 2 | Shares Rust with the engine; uses the system web view, so the installer stays small; ships a [signed updater](https://v2.tauri.app/release/updater/); its command allow-list suits the threat model | Electron: more contributors and identical rendering everywhere, but far larger, and the heavy work would still need native code. Native WinUI or Qt: best feel, not portable or C++-heavy. Python with a web interface: fastest prototype, fragile to package | Rust raises the contributor bar; rendering differs between operating systems' web views |
| Engine language | Rust | Memory safety where untrusted files are parsed; one binary; mature bindings for SQLite, ONNX Runtime and PDFium | Python: richest document and ML libraries, hard to ship to non-technical users. C#: strong on Windows, weaker cross-platform desktop story. Go and TypeScript: weaker ML bindings | Slower early development; fewer document parsers than Python or Java |
| Interface | TypeScript, React, Vite | Largest contributor pool; mature accessible component primitives | Svelte, Solid, Vue: all viable and lighter | Heavier than Svelte. Low stakes, because the interface is thin and replaceable |
| Interface state | Component state plus one small store; engine state fetched and pushed over IPC | The database is already the source of truth | A global Redux-style store | Needs discipline not to mirror engine state in the interface |
| Storage | One SQLite file in WAL mode | Text, metadata, keyword index and vectors commit in one transaction; one file to back up or delete; proven durability | LanceDB: built-in vector and hybrid search, younger storage engine. Tantivy plus a vector library: stronger parts, three stores to keep consistent. Embedded Qdrant or Chroma: server-shaped | Single writer; vector search depends on an extension |
| Keyword index | SQLite FTS5 with BM25 ranking | Built in and transactional with everything else | Tantivy: faster, richer tokenisers | Basic tokenisation; stemming only for English; Arabic-script text needs normalising before indexing; Chinese and Japanese need the trigram tokeniser |
| Vector index | sqlite-vec: exact search in 0.1, quantised rescoring when corpus size requires it | No training step; handles inserts and deletes online; lives in the same file. Its [rescore index](https://github.com/asg017/sqlite-vec/pull/276) merged in March 2026 | [SQLite Vec1](https://sqlite.org/vec1): the SQLite team's ANN extension, version 0.7, needs a training step. HNSW libraries: fast, separate file, awkward deletes | Query time grows linearly with corpus size; pre-1.0 with one main maintainer, so it sits behind an interface |
| Embedding runtime | ONNX Runtime through the `ort` Rust binding | Fast on CPU; the same runtime can later serve OCR and reranker models; a path to GPU and NPU | llama.cpp: one runtime for embeddings and answers, fewer encoder optimisations. Candle: pure Rust, slower on CPU. OpenVINO: fastest, Intel only | A native library to ship per platform; `ort` 2.0 is [still a release candidate](https://docs.rs/crate/ort/2.0.0-rc.13/source/README.md), so pin it |
| Embedding model | **OPEN.** Provisional choice: granite-embedding-97m-multilingual-r2 in 8-bit form, confirmed or overturned by the Phase 0 benchmark (section 26) | Changing the model later forces a full re-embed, so the choice must rest on measurement | Shortlist: [granite-embedding-97m-multilingual-r2](https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2) (Apache-2.0, 384 dimensions, 200+ languages, April 2026); multilingual-e5-small (MIT, the proven baseline). English-only models are ruled out by Q1 | Models under Gemma-style or non-commercial licences are excluded from bundling |
| PDF extraction | PDFium, inside the worker | The engine inside Chrome: heavily fuzzed, permissively licensed, tolerant of broken real-world PDFs | Pure-Rust PDF libraries: memory-safe, weaker on malformed files. MuPDF: excellent, but AGPL would dictate the project licence. Poppler: GPL | A C++ dependency with regular security fixes; must be updated promptly |
| Office extraction (0.2) | Parse the zipped XML directly | Documented formats; no heavy dependency | LibreOffice headless: covers legacy formats, very large and slow. Apache Tika: needs a Java runtime | Own parsing code to maintain; legacy binary formats stay unsupported |
| OCR (0.3) | **OPEN.** Leading option: PP-OCR models on ONNX Runtime | Reuses the runtime already shipped; Apache-2.0 models | Tesseract 5: mature, adds C++ dependencies. Windows built-in OCR: nothing to bundle, Windows-only | Decided by a bake-off on real scans; quality on photos and non-Latin scripts varies |
| Answer runtime (0.4) | **OPEN.** Leading option: llama.cpp in its own process, one curated model downloaded on request | Widest hardware coverage; crash and memory isolation | Connecting to a local server the user already runs, loopback only. ONNX Runtime's generation library | GPU builds multiply packaging work; CPU-only answers take tens of seconds |
| Change detection | Full reconciliation scan, plus OS change notifications from 0.2 | Notifications are lossy and blind while the app is closed; the scan is the source of truth | NTFS change journal: fast, needs elevated rights. Polling only: simple, slow to notice | Scanning very large trees takes minutes |
| IPC | Tauri commands and events; pipes to workers | No port and no HTTP server for a web page to attack | Localhost HTTP or gRPC: familiar, but opens a port | The worker protocol is custom and must be versioned |
| Configuration | One human-readable settings file | Easy to inspect, back up and support | Windows registry: not portable. Settings inside the index: lost on rebuild | Schema migrations to maintain |
| Logging | Structured local files, rotated | Supports diagnosis without telemetry | Hosted error reporting: rejected, it breaks the privacy promise | Maintainers see only what users choose to send |

Linked facts were checked on 3 October 2026. The quality score on the Granite model card is the vendor's own figure and must be confirmed on this project's evaluation set.

### Three places the original plan's "typical choice" is adjusted

- **"Hybrid search with reranking."** Hybrid search yes, from 0.1. Reranking adds a second model and roughly a second of latency on CPU, so it ships only if measured gains justify it.
- **"Tesseract or similar."** Tesseract is a sound fallback, but an ONNX-based engine avoids a second native dependency chain. The decision waits for real scans.
- **"Passages of a few hundred words."** Passage size must be set in the embedding model's tokens, not words. Text beyond a model's limit is silently ignored, which loses matches without any error.

## 11. System Architecture

Document text moves from the user's folders through a worker into one local database, and from there only to the screen. The single network path belongs to the shell and carries no document data.

```text
User
  |
Interface (web view)
  Search, Library and Settings screens. Shows document text as plain text.
  No file or network access.
  |  commands and events, from a fixed list (both directions)
Shell (Tauri)
  App frame, tray, dialogs. Turns commands into engine calls.
  Updater and model download: the only network code.
  |---- HTTPS ----> Update and model hosts
  |                 (signed installers and models; nothing is ever uploaded)
  |  engine API
Engine (Rust library with no network code)
  Scanner              finds new and changed files
  Indexing coordinator job queue, single writer
  Search               keyword and meaning, fused
  Embedding runtime    bundled model, runs on CPU
  |<--- pipes ---> Extraction worker (separate process, one file at a time,
  |                time and memory limits, no network code)
  |                     | reads content
  |---- lists, watches ---> Your folders (read only; never changed)
  |  one writer, many readers
Local data folder (user profile, not synced)
  Index database          one SQLite file: text, keyword index, vectors
  Settings, logs, models  kept apart from the index, so a rebuild loses nothing
```

The accent path is the only route to the network: it brings signed downloads in and sends nothing out. Everything in the shaded engine is built without network code. The Store build omits the updater, so until answers arrive in 0.4 it has no network path at all.

### How data moves when indexing

1. **Scan.** The scanner walks each chosen folder, applies exclusions, and compares name, size and modified time with the index. Cloud-only placeholders and unreachable drives are noted, never opened.
2. **Plan.** For each new or changed file the coordinator records a job and hashes the content. If the hash is already known, existing text and vectors are reused.
3. **Extract.** The coordinator hands the path to an extraction worker. The worker returns text with page and position markers, or a failure reason. A timeout or crash becomes a recorded failure.
4. **Keyword stage.** The engine validates the output, splits it into passages, and commits text and keyword index in one transaction. The file is now keyword-searchable.
5. **Meaning stage.** The embedding runtime turns passages into vectors in batches and commits them. The file is now meaning-searchable.
6. **Purge.** Deleted files and removed folders are purged the same way, in one transaction.

### How data moves for a query

1. The interface sends the query text and filters.
2. The engine runs two searches at once: keyword search over the text index, and vector search with the embedded query.
3. Reciprocal rank fusion merges the two lists. Passages are grouped by file.
4. The engine returns result IDs with passage text, file name, location and match type. The interface renders them as plain text.
5. Opening a result sends its ID. The engine resolves the path from the index and asks Windows to open it.

### Asking a question (0.4)

Retrieval runs as above. The top passages are numbered and passed to the answer worker, and the answer streams back. Before display, the engine checks that every citation refers to a passage that was actually supplied.

## 12. Data Architecture

The app holds two kinds of data and treats them differently: a few precious user choices, and a large index that can always be rebuilt from the user's files.

### Ownership and location

| Data | Kind | Location | If lost |
| --- | --- | --- | --- |
| User's documents | The user's own; never modified | Wherever they are | Not the app's to lose |
| Settings: folders, exclusions, preferences | Precious | `config` folder | Previous copy restored; otherwise the user re-chooses folders |
| Index database | Derived and sensitive | `data` folder | Rebuilt automatically |
| Models | Replaceable | `models` folder | Reinstalled, verified by checksum |
| Logs, crash reports, temporary files | Disposable | `logs` and `tmp` folders | Nothing |

All app folders sit under the non-roaming local profile (`%LOCALAPPDATA%`), never in a synced location.

### Entities and relationships

```text
Source folder   a folder the user chose, with its exclusion rules
                state: active, offline or being removed
   | one folder, many files
File entry      path, size, modified time: cheap to compare on every scan
                state and reason: indexed, skipped, failed, offline
   | many files can share one content
Content         one per unique file content, found by its hash
                extracted text with page and position markers
   | one content, many passages
Passage         a few hundred tokens of text with its location
                also listed in the keyword index
   | one passage, one vector
Vector          the passage embedding; a compact copy is added at scale
                valid only for the model recorded in the index

Source folder and File entry describe what is on disk: cheap to re-scan.
Content, Passage and Vector are computed: costly, so reused by content hash.
```

Two identical PDFs in different folders become two file entries and one content, so the second costs nothing. A renamed file keeps its content.

Supporting records:

- **Job.** A unit of pending work (extract, embed, purge) with state, attempts and last error.
- **Index metadata.** Schema version, embedding model and version, chunking version.
- **Model manifest.** Shipped with the app: dimensions, token limit, query and passage prefixes, licence, checksum.

### Lifecycle of a file

1. **Discovered** by a scan.
2. **Keyword-ready** once text is extracted and indexed.
3. **Meaning-ready** once vectors are stored.

Side states, each with a reason code shown in Library:

- **Skipped**: unsupported type, too large, encrypted, cloud-only, access denied, needs OCR.
- **Failed**: extraction crashed or timed out. After two attempts the file is parked until the extractor changes or the user retries.
- **Offline**: the folder's drive is unreachable. Entries are kept.
- **Deleted**: purged. A file counts as deleted only when its folder is reachable and the file is gone.

### Validation

- Worker output is checked for size, valid text encoding and consistent page numbers before it is stored.
- Settings are validated against a schema on load.
- The database gets a quick integrity check at startup and a full check on demand.

### Migrations

- **Three independent versions**: index schema, settings schema, and pipeline (extractor, chunker, model).
- **Schema changes** are forward-only and run in a transaction at startup. A failed migration falls back to a rebuild.
- **Pipeline changes** re-chunk or re-embed in the background from stored text. Search keeps using the old data until the new set is complete.
- **Downgrades**: an older app that meets a newer schema refuses to open it and offers a rebuild. It never writes to it.

### Backup, import and export

- The settings file is the only thing worth backing up, and the app keeps the previous copy itself.
- The index needs no backup. Backing it up to a cloud service would copy private text off the machine, and the documentation says so.
- Settings export and import is planned for 0.2. There is no index export.

### Corruption recovery

1. **Index unreadable.** Set the damaged file aside, create a fresh index, rescan. Delete the damaged file after the next successful launch, because it holds private text.
2. **Settings unreadable.** Restore the previous copy, or start from defaults with a notice.
3. **Model checksum mismatch.** Refuse to load it and offer to repair the installation.
4. **Disk full.** The transaction rolls back and indexing pauses with a clear message.

### What stays local and what leaves

| Data | Leaves the machine? |
| --- | --- |
| Document content, extracted text, vectors | Never |
| Queries and generated answers | Never |
| File names and paths | Never; included in a diagnostics export only if the user opts in |
| Usage statistics | Not collected |
| Update check | GitHub build only: a request for a static file. The host sees the IP address and the app version |
| Model download (0.4) | A request for a file. The host sees the IP address and which model |
| Diagnostics bundle | Only if the user exports it and sends it themselves |

Two privacy points are easy to miss. Vectors are as sensitive as text, because text can be partly reconstructed from them. And the index copies text out of its source: documents kept on an encrypted volume are weakened if the index sits on an unencrypted disk, which is why the data location can be moved (APP-7).

## 13. Security Architecture

The central risk is that the app must read untrusted files with complex parsers while holding access to everything the user owns. The design contains that risk in a worker process and keeps the network away from everything that touches document text.

### Threat model in brief

- **Assets.** The user's documents, the index (a full-text copy of them), the update channel, and the credibility of the privacy promise.
- **Adversaries in scope.** A malicious file that lands in an indexed folder, such as a saved email attachment. A network attacker on the update path. A compromised dependency or build pipeline. A web page trying to reach the app locally.
- **Out of scope.** Malware already running as the user, which can read the documents directly. An attacker with administrator rights or an unlocked session.

### Threats

| # | Threat | Impact | Mitigation |
| --- | --- | --- | --- |
| T1 | A malicious file exploits a parser (PDF, Office, image, OCR) | Code runs as the user, with access to all their files | Parse only in the worker. Limit its memory, time and child processes. No network code in the worker. Reduced privileges from 0.2. Keep PDFium current. Fuzz the extractors |
| T2 | Resource exhaustion: zip bomb, giant or deeply nested file | Hang, memory exhaustion, full disk | Caps on size, pages, decompressed bytes and time. Kill and park the file. Never retry in a loop |
| T3 | Document text or a file name carrying markup reaches the interface | Script in the web view could call shell commands | Render document text as plain text only. Strict content security policy: no remote content, inline script or eval. Minimal command allow-list. Commands take IDs, not paths |
| T4 | A compromised worker returns hostile output | Engine compromise or a poisoned index | Validate length, encoding and structure of every worker message. The worker has no database access |
| T5 | The update channel is compromised | Malicious code reaches every user | The updater verifies a signature against a key built into the app. Store packages are signed by Microsoft; the GitHub installer is Authenticode-signed once SignPath accepts the project. The update key is kept outside CI. The release job needs manual approval. Maintainers use hardware two-factor authentication |
| T6 | The update signing key is lost | Users can no longer be updated automatically | Encrypted offline backup, a written rotation plan, and a yearly restore test |
| T7 | Dependency or build supply-chain attack | Malicious code in a release | Committed lockfiles. Automated advisory and licence checks. CI actions pinned by commit hash. Least-privilege CI tokens. No secrets for fork pull requests. Provenance attestation and a bill of materials per release |
| T8 | A tampered model file or prebuilt native library | Wrong results, or code execution | Checksums pinned in the repository, verified at build time and at download |
| T9 | A web page or local process attacks a local port | Remote reading of the index | No listening port exists. If a loopback model provider is added, the app is the client only |
| T10 | The index is read by someone else: shared PC, stolen disk, backup or sync copy | Disclosure of private text | Per-user profile location. Never in synced folders. Rely on BitLocker for stolen disks and say so. Uninstall and "delete all data" remove it. Optional encryption by 1.0 |
| T11 | Private data in logs, crash reports or bug reports | Disclosure through support channels | No content or queries in logs. Paths only in opt-in debug mode. Diagnostics are redacted and readable before sending. Issue templates warn against attaching private files |
| T12 | Links or junctions lead the scanner outside a chosen folder | Files indexed that the user did not choose | Do not follow links out of a chosen folder. Resolve and check real paths |
| T13 | Secrets swept into the index: key files, password exports | Credentials duplicated in a second place | Default exclusions for credential and key file patterns, editable by the user |
| T14 | Prompt injection inside a document (0.4) | A misleading generated answer | The model has no tools and no network. Output is plain text with sources beside it. Citations are checked against supplied passages |
| T15 | Opening a result launches something harmful | Code execution by user action | Only indexed document types can be opened. The path comes from the index. The Windows shell API is called directly, never a command line |
| T16 | Privilege escalation | System compromise | The app never elevates, installs per user and has no service |
| T17 | A library is planted in the per-user install folder | Code runs at next start | Equivalent to same-user malware, so out of scope. Libraries are loaded by absolute path. Store installs are read-only, which removes the risk there. A per-machine installer is offered later for managed PCs |

### Deliberately not done

- **No authentication, authorization, secrets or API keys.** A single-user local app has nothing for them to protect.
- **No index encryption in the MVP.** With disk encryption on, it adds little. With it off, the documents themselves are just as exposed.
- **No certificate pinning.** Verifying the signature of what is downloaded matters more than pinning the connection.

### Guarded areas

Three parts of the code decide the security posture: the command list exposed to the interface, the worker protocol, and the network module. Changes to them require maintainer review through CODEOWNERS and a short checklist in the pull request.

## 14. Performance Strategy

Search must feel instant and indexing must stay out of the way. The first index of a large library will take hours on a laptop processor; the design accepts that and makes search useful within minutes.

### Reference conditions

- **Machine.** A four-core, eight-thread x64 laptop from about 2020, 8 GB of RAM, SSD, no usable GPU.
- **Corpus.** 10,000 documents, about 250,000 passages.

### Targets

These are proposed targets, not measurements. The Phase 0 benchmark confirms or revises each one.

| Measure | Target |
| --- | --- |
| Cold start to a usable search box | Under 2 s |
| First query after start, including model load | Under 1.5 s |
| Keyword results, 95th percentile | Under 100 ms |
| Combined results, 95th percentile | Under 500 ms |
| Scan of an unchanged 10,000-file library | Under 30 s |
| New library keyword-ready | Within 60 minutes |
| Embedding throughput | At least 20 passages per second, so about 3.5 hours for the reference corpus |
| Memory when idle with the model released, including the web view | Under 400 MB |
| Memory at indexing peak | Under 1.5 GB |
| Processor use in the default mode | At most half the logical cores, at below-normal priority |
| Index size on disk | About 4 KB per passage, so about 1 GB for the reference corpus |
| Interface responsiveness | No app-caused stall longer than 50 ms |

One outside data point supports the query target. The sqlite-vec maintainer [reports](https://github.com/asg017/sqlite-vec/pull/276) 590 ms for an exact scan of one million 1,024-dimension vectors on an Apple M4, and 101 ms with binary rescoring. The reference corpus is a quarter of that count at under half the dimensions.

### Scale tiers

| Corpus | Passages | Vector search approach |
| --- | --- | --- |
| Up to 10,000 documents | About 250,000 | Exact scan |
| Up to 50,000 documents | About 1.25 million | Quantised scan, then rescoring |
| Larger | More | An approximate index behind the same interface; not planned before 1.0 |

### Expected bottlenecks

1. **Embedding on CPU.** It dominates indexing time. Response: keyword-first staging, a quantised model, batching by length, resource modes, and later a GPU or NPU path.
2. **OCR (0.3).** Seconds per page, so a scanned archive takes far longer than embedding. Response: a third, lowest-priority stage with its own progress.
3. **Large or complex PDFs.** Response: page caps, timeouts, several workers.
4. **Antivirus scanning each opened file.** Outside the app's control. Response: read each file once, hashing and extracting in one pass.
5. **Single-writer contention.** Response: one short transaction per document.
6. **Vector scan growth.** Linear in corpus size. Response: quantisation, applying folder filters first, and the tiers above.
7. **Scanning very large folder trees.** Response: compare metadata only, prune excluded folders early, add change notifications in 0.2.
8. **Model load time and memory.** Response: load after the first paint, keep warm while in use, release when idle in the tray.
9. **Battery and heat.** Response: balanced mode by default, pause on battery in 0.2.
10. **Local answers on CPU (0.4).** Reading the retrieved passages can take tens of seconds before the first word appears. Response: a small context of four to six passages, streaming, and honest progress.

### Discipline

- A command-line benchmark on a fixed corpus exists from Phase 0 and runs on a schedule.
- No approximate index, GPU path or custom storage is built until a measured target is missed.

The original hardware guidance holds: 8 GB for search, 16 GB recommended for answers. The addition is that answers without a GPU are slow, and the interface must say so.

## 15. Testing Strategy

Most tests are fast engine tests that run without the interface. Retrieval quality is tested like correctness: a fixed set of questions with known answers gates every change to search.

### The pyramid

| Level | Volume | Covers | Runs |
| --- | --- | --- | --- |
| Unit | Most tests | Chunking, rank fusion, exclusion matching, state machines, path handling, settings validation, message parsing | Every pull request, on Windows, Linux and macOS |
| Extractor conformance | One fixture set per format | Each extractor against a shared suite: input file to expected text, locations and reason codes | Every pull request |
| Engine integration | Dozens | Scan, index and search on a temporary folder; change detection; purge; offline folder; resume after a kill; migration from every released schema | Every pull request |
| Retrieval evaluation | One suite | Judged queries scored by recall@10, MRR and nDCG@10 against thresholds | Changes to search, chunking or models; nightly |
| Interface components | Moderate | Each screen and state with a mocked engine; keyboard navigation; automated accessibility checks | Every pull request |
| End-to-end | About ten | The real app on Windows: first launch, index a fixture folder, search, open, remove a folder, update from a local test server | Main branch and release candidates |
| Installer | Three or four | Install, upgrade from the previous release and uninstall, for both the Store package and the GitHub installer | Release candidates |

### Other test types

- **Regression.** Every fixed bug adds a test. A problem file reported by a user becomes a fixture if it can be redistributed, or a synthetic reproduction if not.
- **Security.** A hostile corpus (zip bomb, oversized and deeply nested files, XML entity tricks, file names containing markup) must leave the app running and the worker killed within limits. Extractors and message parsers are fuzzed on a schedule.
- **Privacy.** With the update check off, indexing and searching in a network-blocked environment must attempt zero connections. A build check confirms the engine links no network library.
- **Performance.** The section 14 benchmarks run nightly on a fixed corpus. Hosted runners are noisy, so CI tracks the trend and alerts on a 15% regression. The release gate is measured by hand on the reference laptop.
- **Accessibility.** Automated checks in CI, plus a manual Narrator and NVDA pass per release.

### The retrieval evaluation set

- Built in Phase 0 from redistributable documents in the target languages, including one right-to-left language.
- At least 100 queries of three kinds: descriptive, exact (identifiers and names) and cross-language.
- Thresholds are set from the first baseline. A change that lowers recall@10 by more than two points needs a written justification.
- It decides the embedding model, passage size, fusion settings, quantisation and whether a reranker is worth shipping.
- Its limit: a public corpus is not a user's corpus. The "search quality report" issue form supplements it.

### What not to test

- The internals of third-party libraries such as PDFium and SQLite.
- The exact wording of generated answers. Test structure instead: citations are valid, and the model abstains when given nothing.
- Pixel-exact screenshots of whole screens.
- Exact ranking order beyond the thresholds.
- Timing on hosted runners as a pass or fail condition.

### Test data rules

- Every fixture is redistributable, free of real personal data, and has its licence recorded.
- Large corpora are fetched by script with pinned checksums, not committed.

## 16. Repository Architecture

One repository holds a Rust workspace of six components and the desktop app. The layout enforces the architecture: the compiler rejects a dependency that points the wrong way.

```text
/
├── apps/
│   └── desktop/            the installable app
│       ├── src-tauri/      Rust shell: window, tray, command handlers, updater, bundle settings
│       └── ui/             TypeScript interface: screens, components, engine client, strings
├── crates/
│   ├── engine/             domain rules, use cases, ports, scanner, pipeline, search
│   ├── store/              SQLite schema, migrations, keyword and vector index adapters
│   ├── extract/            one module per file format, conformance suite, worker protocol
│   ├── worker/             the extraction worker executable
│   ├── embed/              embedding runtime adapter, tokeniser, model manifest
│   └── cli/                headless tool: index, search, evaluate, benchmark
├── models/                 model manifest and checksums; weights fetched by script
├── eval/                   judged queries and thresholds
├── fixtures/               sample documents and the hostile-file corpus, each with its licence
├── docs/
│   ├── architecture/       overview, data flow, threat model
│   ├── adr/                one file per decision
│   ├── user/               user guide, privacy and network statement
│   └── contributing/       setup, adding a file format, release process
├── scripts/                setup and release helpers
├── .github/                workflows, issue forms, pull request template, CODEOWNERS
└── README, LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CHANGELOG, GOVERNANCE
```

### Why this shape

- **Separation of concerns.** `engine` knows nothing about Tauri, SQLite or PDFium. `store`, `extract` and `embed` implement its ports. `worker` depends only on `extract`.
- **The privacy rule is structural.** Only `apps/desktop/src-tauri` may contain network code, and CI checks it.
- **Testing.** Unit tests sit beside the code. `fixtures` and `eval` are shared by all components. The `cli` runs the whole engine without a window, which is what makes CI and benchmarks practical.
- **Feature development.** A new file format is one module in `extract`, its fixtures, and a row in the format table. Nothing else changes. This is the main path for outside contributors.
- **Ownership.** CODEOWNERS maps the three guarded areas from section 13 to maintainers.
- **Long-term maintenance.** One repository means one issue tracker and atomic changes across engine and interface.

### Kept deliberately small

- No empty placeholders. An `ocr` component appears in 0.3 and an `answer` component in 0.4, when their code exists. Phase 0 may start with fewer components and split them later: the dependency rules are fixed, the count is not.
- File watching starts inside `engine` and moves out only if it grows.
- The interface has no feature folders until it has more than a handful of screens.

## 17. Open-Source Strategy

The project is run so that a stranger can clone it, build it in half an hour, and add a file format without understanding the whole system. The licence had to be settled before the first outside contribution, and it now is: Apache-2.0.

### Licence: Apache-2.0

| Option | For | Against |
| --- | --- | --- |
| Apache-2.0 | Explicit patent grant; compatible with every planned dependency; the norm in the Rust ecosystem; comfortable for firms | Allows closed forks |
| MIT | Simplest | No patent grant |
| GPL-3.0 | Forks must stay open, which suits a product whose value is auditable privacy | Deters some corporate contributors and reuse inside other products |
| AGPL-3.0 | Strongest copyleft | Adds nothing for a desktop app with no network service |

Apache-2.0 is chosen (Q4): adoption matters more here than preventing closed forks. It rules out GPL and AGPL libraries, which the dependency check enforces. Relicensing later would need every contributor's consent. Contributions use a Developer Certificate of Origin sign-off, not a contributor licence agreement.

Model weights carry their own licences. Only permissively licensed weights are bundled, and third-party notices are generated automatically and shown in About.

### Repository files

| File | Contents |
| --- | --- |
| README | One-paragraph pitch and a screenshot; the privacy promise with a link to the network statement; status and platforms; install; quick start; supported formats; requirements; the section 11 diagram; build from source; contributing; a tester sign-up link; known accessibility gaps; the code signing policy; security; licence |
| CONTRIBUTING | Numbered setup; how to run tests; formatter and linter as the style authority; commit and pull request conventions; a walk-through for adding a file format; fixture rules; sign-off; the expectation that contributors can explain code they submit, AI-assisted or not |
| CODE\_OF\_CONDUCT | Contributor Covenant, with a contact address |
| SECURITY | Private reporting through GitHub; only the latest release supported before 1.0; acknowledgement within 7 days; scope mirroring section 13 |
| GOVERNANCE | Who decides today; how decisions are made (ADRs and proposal issues); how to become a maintainer; use of the project name by forks |
| CHANGELOG | Keep a Changelog format, written for users; states when an update will re-index |
| LICENSE and NOTICE | Project licence and third-party notices |

### Issues and pull requests

- **Issue forms.** Bug report. File not indexed or text wrong. Search quality report. Feature or format request. Questions go to Discussions; vulnerabilities go to private reporting. Every form warns against attaching private documents.
- **Pull request template.** What and why, linked issue, tests, screenshots for interface changes. Checklist: no new network call, no new dependency without a reason, docs and changelog updated, guarded areas flagged.
- **Milestones.** One per version (0.1, 0.2, 0.3, 0.4, 1.0) plus Backlog.

| Label group | Labels |
| --- | --- |
| Type | bug, feature, docs, chore, security |
| Area | ui, engine, extract, store, embed, build, release, a11y, perf |
| Priority | P0 to P3, added once issue volume justifies it |
| Status | needs-triage, needs-repro, blocked, help wanted, good first issue |
| Platform | windows, macos, linux |

### Branching, commits, versions and releases

- **Branching.** Trunk-based. `main` is always releasable. Short-lived branches, squash merges, linear history. A `release/x.y` branch exists only when an older version needs a patch.
- **Commits.** Conventional Commits on pull request titles, checked in CI.
- **Versioning.** Semantic Versioning for the app. Before 1.0, a minor version may require a re-index. Index schema, settings schema and worker protocol have their own version numbers.
- **Releases.** Two channels, beta and stable. Every release candidate goes to beta first. Releases ship when ready; security fixes ship at once.

### Documentation

- `docs/architecture`: an overview a newcomer reads in ten minutes, the data flow, the threat model. This specification seeds it.
- `docs/adr`: the decision records from section 23.
- `docs/user`: the user guide, and a "what leaves your machine" statement with steps to verify it using a firewall.
- `docs/contributing`: setup, adding a file format, the release process, the evaluation guide.

### A new contributor's first hour

1. Read the README and the ten-minute architecture overview.
2. Run one setup script, then one command that starts the app on a fixture folder.
3. Run the tests, and a search from the command-line tool.
4. Pick a "good first issue". Most are extractor fixes or new fixtures.
5. Open a pull request. CI explains every failure in plain words.

### Sustainability

- **One maintainer is the largest open-source risk.** Section 2 shows a popular app in this space going quiet. Mitigations: strict scope, a written release process, and a second person with release rights before 1.0.
- **Public from the first commit**, once the name is chosen (Q7). It builds trust, serves as the maintainer's portfolio and creates the track record SignPath looks for. The cost is early issues against unfinished work, managed with a clear pre-alpha notice.
- **Triage rules.** Reports without a reproduction close after 30 days. Feature ideas start in Discussions. A public "not planned" list names cloud models, chat and team features.
- **Funding**: no revenue is planned (Q19). With Store signing there is no fixed cost.

## 18. CI/CD Strategy

Every pull request is validated automatically in about 15 minutes. A release is built, signed and published by a pipeline in which a human approves the final step and no secret is exposed to outside contributions.

### Pull request validation

| Stage | What it checks |
| --- | --- |
| Format, lint, types | Rust and TypeScript formatting, lints and type errors |
| Engine tests | Unit and integration tests on Windows, Linux and macOS |
| Interface tests | Component tests and automated accessibility checks |
| Dependencies | Known advisories, licences, banned packages; no unintended lockfile change |
| Privacy invariant | No engine component links a network library |
| Retrieval evaluation | Runs when search, chunking or model files change; fails below thresholds |
| Build | The app compiles in release mode on Windows |
| Static analysis | Code scanning and secret scanning; new alerts block |
| Conventions | Pull request title follows the commit convention |

Every stage blocks merging. Installers, signing and end-to-end tests do not run on pull requests: they are slow, and fork pull requests must never see signing secrets.

### Main branch and scheduled jobs

- End-to-end tests on Windows after every merge.
- An unsigned nightly build for testers.
- Nightly benchmarks and the full retrieval evaluation.
- Scheduled fuzzing of extractors and message parsers.
- Automated dependency update pull requests, with PDFium and ONNX Runtime watched closely.

### Release pipeline

1. A maintainer pushes a version tag from `main`. The tag is the only source of the version number; the build fails if any artefact disagrees.
2. Build two artefacts: the Store package (MSIX, with the updater compiled out) and the GitHub installer.
3. Signing differs by channel. Microsoft signs the Store package after certification. The GitHub installer is unsigned until SignPath accepts the project; SignPath then signs it from CI, with each request approved by hand.
4. The maintainer produces the updater signatures with the separate update key, which is held outside CI on a hardware-backed device.
5. Generate checksums, a bill of materials and a build provenance attestation.
6. Run installer tests and an end-to-end smoke test against both artefacts.
7. Create a draft release with the changelog.
8. A maintainer approves in a protected environment.
9. Publish to the beta channel. After several days without a blocking report, promote the same artefacts to stable.
10. Submit the package to the Store, and update the winget manifest.

Published releases are immutable. A fix is always a new version.

### Automatic updates

- Store installs are updated by Windows and contain no update code. In the GitHub build, if enabled, the app fetches a small static manifest for its channel at most once a day.
- The request carries no identifier. Manifests are static files, so there is no update server to run.
- The download is verified against the built-in key, then installed on restart with the user's consent.

### Rollback

- **Prevention.** Stable advances only after the beta period, so most bad builds never reach it.
- **Stop the spread.** Point the stable manifest back to the last good version so no one else receives the bad one.
- **Roll forward.** Ship a higher version with the fix for those already updated. For Store installs this is the only remedy, and it waits on certification. Tauri's updater can allow downgrades, but the default stays forward-only because index schemas are.
- **Protect data.** Migrations are transactional, settings are copied before migrating, and an older app refuses a newer index.
- **Manual escape.** The last three installers stay downloadable.

### Pipeline security

- Third-party actions are pinned by commit hash.
- Each job gets the minimum token permissions.
- Secrets exist only in the release environment.
- Changes to release workflows need maintainer review, and release tags are protected.

The signing route is decided: the Store first, SignPath after 0.1. Section 19 gives the details.

## 19. Windows Distribution Strategy

Windows users get the app from the Microsoft Store, signed by Microsoft at no cost. A per-user installer on GitHub is the second channel. It needs no administrator rights and is unsigned until SignPath Foundation accepts the project after 0.1.

### Development environment

- Rust stable with the MSVC target, Visual Studio Build Tools and the Windows SDK, Node LTS with a pinned package manager.
- One setup script fetches PDFium, ONNX Runtime and the model, and verifies pinned checksums.
- Toolchain versions are pinned in the repository so local and CI builds match.

### Production builds and installer

- **Targets.** x64 first. It runs on ARM PCs under Windows' emulation, and a native ARM64 build follows 0.1. Windows 10 gets a smoke check before each release.
- **Build.** Release mode, debug symbols kept separately, no executable packers. Once signing is available, every executable and library is signed, including the worker.
- **Installer.** Two artefacts from the same binaries. The Store package is an MSIX with the updater compiled out. The GitHub installer is Tauri's NSIS installer in per-user mode: no administrator prompt, silent install, WebView2 bootstrapper included.
- **Package manager.** A winget manifest from the first public release.
- **Per-machine MSI** for managed PCs, and a **portable version**, wait until after 1.0. A portable build cannot self-update and must redirect the web view's data folder.

### Folders

| Purpose | Location |
| --- | --- |
| Program files | `%LOCALAPPDATA%\Programs\<App>` |
| Settings | `%LOCALAPPDATA%\<App>\config` |
| Index | `%LOCALAPPDATA%\<App>\data` |
| Downloaded models | `%LOCALAPPDATA%\<App>\models` |
| Logs and crash reports | `%LOCALAPPDATA%\<App>\logs`, rotated and size-capped |
| Temporary files | `%LOCALAPPDATA%\<App>\tmp`, cleared at startup |

In a Store install, Windows keeps these folders inside the package's private storage. The roaming profile is avoided on purpose. Settings contain folder paths, and roaming would copy them to a server.

### Uninstall

Uninstalling removes program files, index, models and logs by default. A checkbox lets the user keep settings and index. Upgrades never remove data. A Store uninstall always removes all app data, with no option to keep it. Leaving a full-text copy of private documents behind would break the product's promise.

### Windows permissions and file-system behaviour

- Runs as the invoking user and never requests elevation.
- Reads only what the user can read. Files that deny access are listed as skipped.
- Declares long-path awareness and handles paths over 260 characters.
- Skips cloud-only placeholders. Documents and Desktop are often OneDrive folders, and opening a placeholder would download the file.
- Only reads from user folders, so ransomware protection that blocks writes to them does not interfere.

### Code signing, SmartScreen and Defender

| Option | Cost | Availability | Fit |
| --- | --- | --- | --- |
| Microsoft Store, MSIX | Free | Worldwide | No SmartScreen warning; the Store handles updates. Chosen for 0.1. Tauri has no MSIX bundle target, so packaging is proven in a Phase 0 spike |
| Azure Artifact Signing | About $9.99 a month | Organisations in the USA, Canada, EU and UK; individuals in the USA and Canada only | Best CI integration, if eligible |
| SignPath Foundation | Free for qualifying open-source projects | By application | Planned after 0.1. Needs a released project with verifiable reputation. Certificate names the foundation; each release is approved by hand |
| OV certificate from a certificate authority | $150 to $300 a year | Worldwide | Key must live on a hardware token or cloud module |
| EV certificate | $400 or more a year | Worldwide | No longer skips SmartScreen since 2024; not worth the premium |
| Unsigned | Free | Anywhere | Strong SmartScreen block. Accepted only for the GitHub channel until SignPath signing is granted |

Source: Microsoft's [code signing options for Windows app developers](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options), updated 29 August 2026.

- **Decision (Q6).** An individual publisher distributes through the Microsoft Store first. Registration is free for individuals and Microsoft signs the package.
- **Packaging.** Tauri does not build MSIX itself. Microsoft documents the route in its [winapp guide for Tauri](https://learn.microsoft.com/en-us/windows/apps/dev-tools/winapp-cli/guides/tauri). A Phase 0 spike must prove the packaged app works with its worker process, native libraries and data folders.
- **Store build differences.** No updater, so no network code at all. Program files are read-only. Windows delivers updates and removes all data on uninstall. Installing from the Store tells Microsoft that the app was installed, and the network statement says so.
- **Store requirements.** A reserved name, a privacy policy page, an age rating, and a justification for full-trust access, since the app reads user folders.
- **GitHub channel.** The same binaries as a per-user installer, unsigned at first. Windows shows a SmartScreen warning and some managed PCs will block it. The README says so and points to the Store. Checksums and a build provenance attestation let technical users verify the download.
- **SignPath Foundation after 0.1.** Its [terms](https://signpath.org/terms) ask for an OSI licence without commercial dual-licensing, a project already released in the form to be signed, verifiable reputation, multi-factor authentication, a published code signing policy, and manual approval of every signing request.
- **Fallback.** If the Store route fails, the paid alternative is an OV certificate.
- **One identity per channel.** Reputation carries across releases only if the signer stays the same.
- **Antivirus false positives.** More likely while unsigned. Report them to Microsoft, publish checksums, avoid packers.

### Crash handling

- An engine failure is caught at the task boundary, logged with a stack trace, and shown as a recoverable error where possible.
- A worker crash is routine: the file is marked failed and indexing continues.
- A crash of the main process writes a local report with version, system and stack, and no document data. Nothing is sent.
- After two failed starts in a row the app opens in a safe mode: indexing paused, with options to rebuild the index or export diagnostics.

### Linux and macOS

Support at 1.0 is realistic. Tauri targets both, and the engine's tests run on both from the first commit.

- **What preserves the option.** Operating-system code stays in adapters: placeholder detection, process limits, opening files, known folders. Paths are stored losslessly. At least one OCR option is cross-platform.
- **macOS costs.** The Apple developer programme fee, notarisation, permission prompts for Documents, Desktop and Downloads, and a different web view engine.
- **Linux costs.** Several package formats, web view versions that vary by distribution, and file-watch limits.
- **Both.** Worker sandboxing uses a different mechanism on each system, so it sits behind an interface.

## 20. Development Roadmap

A public 0.1, installable from the Microsoft Store, is an estimated 19 to 28 weeks of part-time work across six phases. The original 4 to 6 weeks buys roughly Phase 0: decisions made and search proven from the command line, not a public release.

```text
Phase 0  Foundation                 4 to 5 weeks
         Repository, CI, the two-week Rust test, evaluation set,
         model benchmark, packaging spike
  Gate:  Rust test passed; model chosen; MSIX packaging and
         command-line search proven
Phase 1  MVP foundation             4 to 6 weeks
         A thin slice end to end: pick a folder, index text and PDF,
         search, open the file
  Gate:  first real search in the app; quality at or above the baseline
Phase 2  Core functionality         5 to 8 weeks
         The rest of the 0.1 scope: staging, coverage view, purge,
         settings, updater, installer
  Gate:  every MUST requirement implemented
Phase 3  Testing and stabilisation  3 to 4 weeks
         Hostile files, resume, performance, accessibility,
         ten outside testers
  Gate:  MVP exit criteria met; no open P0 or P1 issue
Phase 4  Release candidate          2 to 3 weeks
         Store-certified build and GitHub beta; update path tested;
         documentation complete
  Gate:  one week on beta with no blocking report
Phase 5  Public release of 0.1      1 to 2 weeks
         Version 0.1 in the Microsoft Store and on GitHub,
         announcement, issue triage

Total: 19 to 28 part-time weeks. At 10 hours a week, plan on about 28.
```

Each gate must pass before the next phase starts. The estimates are judgement, not measurement. Add a third to a half if Rust is new to the developer. The planning basis is one person at 10 hours a week (Q3), so plan on the upper end of each range: about 28 weeks.

### Phase 0: Architecture and repository foundation

- **Objective.** Make every later step safe to build on: a working repository, the open decisions closed, and proof that the stack performs.
- **Features.** None visible to users. A command-line spike that extracts a PDF, embeds it, stores it and searches it, time-boxed to two weeks as the Rust test (Q2). A second spike packages a minimal app with a worker process as MSIX and installs it locally.
- **Dependencies.** The product name (Q5), which also unlocks the Store name reservation. The other blocking questions are answered.
- **Deliverables.** Repository with CI and governance files; the first ADRs; evaluation set and fixture corpus; a benchmark report that selects the embedding model and passage size.
- **Acceptance criteria.** A clean clone builds and passes CI on three operating systems. The command-line tool indexes the fixtures and sets the first recall baseline. Benchmark numbers are recorded against section 14. The packaged test app installs from an MSIX and runs its worker process.
- **Risks.** Decisions drag on. The evaluation set is too small to be meaningful. The spike exposes a packaging problem with PDFium or ONNX Runtime on Windows.

### Phase 1: MVP foundation

- **Objective.** One thin slice working end to end in the real app.
- **Features.** Folder picker, scan, extraction worker for text and PDF, chunking, keyword and vector index, combined search, results list, open file.
- **Dependencies.** Phase 0 decisions, including the model.
- **Deliverables.** An unsigned internal build; the typed command contract; worker protocol and schema, both at version 1.
- **Acceptance criteria.** On a 500-document folder a tester finds known passages. Killing the app mid-index loses no completed work. Retrieval is at or above the baseline.
- **Risks.** Worker isolation on Windows takes longer than planned. The contract between interface and engine churns.

### Phase 2: Core functionality

- **Objective.** Complete the 0.1 scope.
- **Features.** Two-stage indexing with progress; Library view with reasons; exclusions and limits; cloud-placeholder and offline handling; purge; preview; filters and phrases; first-launch flow; settings; logs and diagnostics; Store package; GitHub installer with its updater.
- **Dependencies.** The Phase 1 slice. Store developer registration completed and the name reserved.
- **Deliverables.** A feature-complete alpha; a draft user guide; the network statement.
- **Acceptance criteria.** Every MUST in sections 6 and 7 has a passing test or a recorded manual check.
- **Risks.** Scope creep toward OCR or Office formats. Updater and installer work overruns. Store certification rules force packaging changes.

### Phase 3: Testing and stabilisation

- **Objective.** Make it trustworthy on other people's files.
- **Features.** None new. Hostile corpus, fuzzing, performance tuning, accessibility pass, clearer error messages.
- **Dependencies.** A feature-complete build and ten recruited testers.
- **Deliverables.** A test report against the exit criteria; new fixtures from tester findings; a performance report from the reference laptop.
- **Acceptance criteria.** Section 5 exit criteria met. No open P0 or P1 issue. No crash in a 24-hour indexing run.
- **Risks.** Real PDFs break extraction in ways fixtures did not. Testers are hard to recruit. A performance target is missed.

### Phase 4: Release candidate

- **Objective.** Prove the release and update machinery with real builds: a Store submission that passes certification, and a GitHub release.
- **Features.** None.
- **Dependencies.** A Store developer account and the release pipeline.
- **Deliverables.** A candidate that has passed Store certification, and the GitHub candidate on the beta channel; a tested update from one candidate to the next; complete documentation; the security policy published.
- **Acceptance criteria.** Install, update and uninstall verified on a clean Windows 11 machine for both channels, plus a smoke check on Windows 10. One week on beta with no blocking report. One rollback drill performed.
- **Risks.** Store certification delays. Antivirus false positives on the unsigned GitHub build. An updater defect found late.

### Phase 5: Public open-source release

- **Objective.** Ship 0.1 and be ready for the response.
- **Features.** None.
- **Dependencies.** An accepted release candidate.
- **Deliverables.** Version 0.1 in the Microsoft Store and on GitHub; a winget entry; the SignPath application submitted; an announcement; the 0.2 plan published; a triage routine.
- **Acceptance criteria.** A stranger installs and searches without contacting the maintainer. First issues are triaged within a week.
- **Risks.** Support load crowds out development. Users expect scans to work because of the headline example, so the limit must be stated prominently.

### After 0.1

| Version | Theme | Notes |
| --- | --- | --- |
| 0.2 | Office formats, reduced-privilege worker, live watching, tray, filters, ranking | A reranker only if evaluation justifies it. Also the SignPath-signed GitHub installer once accepted, and a native ARM64 build |
| 0.3 | OCR for scans and images | Engine chosen by bake-off; moves ahead of 0.2 if scans exceed about a quarter of testers' libraries |
| 0.4 | Cited answers from a local model | Optional download |
| 1.0 | macOS and Linux with worker sandboxing on each, accessibility conformance, translations | Email files follow after 1.0 (Q15) |

Reaching 1.0 is more likely 12 to 18 months of part-time work than "several months". That is also an estimate.

## 21. GitHub Backlog

These 44 issues form the 0.1 milestone. Later versions get their own milestones once 0.1 feedback is in.

### Infrastructure

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| INF-1 | Create repository and governance files | Set expectations from the first commit | README skeleton, licence, CONTRIBUTING, conduct, SECURITY, GOVERNANCE, issue forms, pull request template, labels, milestones | Name decided (Q5) | All files present; a test issue and pull request use the templates |
| INF-2 | Workspace skeleton | Enforce dependency rules from day one | Six components and the app shell, empty but building; toolchains pinned | INF-1 | A clean clone builds with one command on Windows, Linux and macOS |
| INF-3 | Developer setup script | Onboarding in under 30 minutes | Fetch PDFium, ONNX Runtime and the model with pinned checksums | INF-2 | A new machine reaches a running app from the README alone |

### Architecture

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| ARC-1 | Record the founding ADRs | Decisions are written, not remembered | ADR template and the records from section 23 | INF-1 | Made decisions merged as accepted; open ones as proposed |
| ARC-2 | Ports and domain model | Stable seams for adapters | Interfaces for extractor, index store, embedding runtime, file system; file state machine | INF-2 | Engine compiles against in-memory fakes; transitions unit-tested |
| ARC-3 | Typed command contract | Stop interface and engine drifting apart | Command and event definitions; generated TypeScript types; ID-only rule | ARC-2 | A changed Rust type breaks the interface build until regenerated |
| ARC-4 | Worker protocol v1 | A safe boundary for untrusted parsing | Message format, versioning, limits, validation | ARC-2 | Malformed and oversized messages are rejected in tests |
| ARC-5 | Benchmark and choose the embedding model | Close the costliest open decision | Shortlist on the evaluation set and reference laptop; passage size sweep; measured on the 8-bit model that would ship | TST-1, CORE-5 | A written report and an accepted ADR |

### Core functionality

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| CORE-1 | Folder scanner and reconciliation | Know what changed on disk | Walk, exclusions, change detection, hashing, link policy, offline detection, placeholder skip, retry of locked files, deep rescan | ARC-2, DB-1 | Tests cover add, change, move, delete, offline and placeholder |
| CORE-2 | Extraction worker and supervisor | Contain bad files | Worker executable; time and memory limits; kill and record; bounded retries | ARC-4 | The hostile corpus leaves the app running; failures carry reason codes |
| CORE-3 | PDF and text extractors | First supported formats | PDFium inside the worker; text with pages; detection of scans and encryption; text encoding detection | CORE-2 | Conformance suite passes on the fixtures |
| CORE-4 | Chunker | Passages that fit the model | Token-based passages with overlap; page and offset tracking | ARC-5 | No passage exceeds the model limit; locations round-trip |
| CORE-5 | Embedding runtime | Local vectors | Load by manifest; batching; prefixes; thread control; warm and release | ARC-2 | Vectors match reference outputs within tolerance; throughput recorded |
| CORE-6 | Indexing coordinator and job queue | Resumable, staged indexing | Pipeline with back-pressure; pause and resume; crash recovery; purge | CORE-1 to CORE-5, DB-1 | Kill-and-resume test passes; a purge leaves no rows |
| CORE-7 | Combined search | The product's core function | Keyword query, vector query, rank fusion, grouping, filters, phrases | DB-2, CORE-5 | Evaluation thresholds met; query time within target |
| CORE-8 | Command-line tool | Test and measure without a window | Index, search, evaluate, benchmark | CORE-6, CORE-7 | CI runs the evaluation through it |

### Database

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| DB-1 | Schema v1 and migration runner | Durable, upgradable storage | Entities from section 12; forward-only migrations; integrity check; rebuild path | ARC-2 | Migration tests pass; a corrupt file triggers a rebuild |
| DB-2 | Keyword and vector index adapters | Swappable search back ends | One index-store adapter with FTS5 and sqlite-vec inside; shared conformance suite | DB-1 | The adapter and an in-memory fake pass the suite |
| DB-3 | Secure deletion and compaction | Purged text is really gone | Overwrite freed pages; compact after large purges | DB-1 | A test finds no purged text in the raw file |

### Interface

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| UI-1 | App shell and navigation | The frame everything sits in | Window, rail, theme, single instance, remembered size; a mock-engine mode for interface work | ARC-3 | Three destinations reachable by mouse and keyboard |
| UI-2 | Search screen | The home screen | Search box, grouped results, preview, status line, keyboard navigation | ARC-3, CORE-7 | Every Search state in section 8 has a component test |
| UI-3 | Library screen | Coverage and control | Folders, two-stage progress, needs-attention list, retry, pause | CORE-6 | Each reason code is shown with a plain explanation |
| UI-4 | Settings screen | User control | Folders, exclusions, resource mode, updates, delete all data | CORE-1 | Changes persist and take effect without a restart |
| UI-5 | First-launch flow | First search within a minute | Privacy statement, folder choice, update-check choice | UI-1, UI-4 | A new user reaches a working search in three steps |
| UI-6 | Accessibility pass | Usable by everyone | Labels, focus order, announcements, contrast themes | UI-2 to UI-5 | Automated checks pass; manual screen-reader checklist complete |

### Security

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| SEC-1 | Lock down the web view | Untrusted text cannot act | Content security policy; command allow-list; plain-text rendering | UI-1 | A file named with markup renders inertly; unlisted commands are refused |
| SEC-2 | Network module and privacy checks | Make the promise testable | One module with a host allow-list; build check; network-blocked test | INF-2 | Both checks run on every pull request |
| SEC-3 | Publish the threat model | Shared understanding of risk | Section 13 as a document; review checklist for guarded areas | INF-1 | Linked from SECURITY and the pull request template |
| SEC-4 | Fuzzing harness | Find parser faults first | Worker protocol parser in 0.1, extractors from 0.2; scheduled runs | CORE-3, ARC-4 | Runs weekly; findings open issues automatically |

### Testing

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| TST-1 | Evaluation set v1 | Measure search quality | 100 or more judged queries on redistributable documents; thresholds | Languages decided | Baseline recorded; thresholds enforced in CI |
| TST-2 | Fixture and hostile corpora | Realistic and adversarial inputs | Per-format fixtures with licences; hostile file set | INF-2 | Every fixture has a recorded licence |
| TST-3 | End-to-end suite | Prove the assembled app | About ten flows on Windows | UI-2 to UI-5 | Runs green on the main branch |
| TST-4 | Benchmark suite | Hold the performance line | Section 14 measures through the command-line tool; nightly trend | CORE-8 | A 15% regression raises an alert |

### Documentation

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| DOC-1 | Architecture overview | A ten-minute tour | Overview, data flow, diagrams | ARC-2 | A new reader can name the three guarded areas |
| DOC-2 | User guide and network statement | Explain use and the privacy promise | Guide; what leaves the machine; how to verify; the privacy policy page the Store requires | UI-5, SEC-2 | Reviewed by one non-technical tester |
| DOC-3 | "Add a file format" guide | The main contribution path | Step-by-step walk-through with a sample extractor | CORE-3 | An outside contributor follows it successfully |

### CI/CD

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| CI-1 | Pull request pipeline | Fast, automatic validation | The stages in section 18 | INF-2 | Runs in about 15 minutes; fork pull requests see no secrets |
| CI-2 | Scheduled jobs | Slow checks off the critical path | End-to-end, benchmarks, evaluation, fuzzing, unsigned nightly build | CI-1, TST-3, TST-4 | Failures open or update an issue |
| CI-3 | Dependency policy automation | Keep the supply chain healthy | Advisory and licence checks; update pull requests | CI-1 | A dependency with a disallowed licence fails the build |

### Release

| ID | Title | Purpose | Scope | Depends on | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| REL-1 | Store registration and packaging spike | Remove the longest lead-time blocker | Register as an individual developer; reserve the name; package a minimal app with its worker as MSIX | Name decided (Q5) | The account and name are in place; the test MSIX installs locally and runs its worker |
| REL-2 | Installer | A clean install and removal | Store MSIX with the updater compiled out; per-user GitHub installer; uninstall behaviour; winget manifest | REL-1 | Install, upgrade and uninstall tests pass; an upgrade keeps the index and an uninstall removes it by default |
| REL-3 | Updater and channels | Deliver fixes safely | Update key generated and backed up offline; beta and stable manifests | REL-2 | An update from one test build to the next succeeds; a tampered file is refused |
| REL-4 | Release pipeline | Repeatable, auditable releases | Tag to draft release and Store submission: checksums, bill of materials, attestation, approval gate; SignPath signing added once granted | CI-1, REL-1 to REL-3 | A tagged test release is produced with no manual steps except update signing and approval |
| REL-5 | Release checklist and rollback drill | Be ready before it matters | Written checklist; one practised rollback; SignPath application prepared for after 0.1 | REL-4 | The drill is completed and documented |

## 22. Risks

The two risks most likely to end the project are unproven retrieval quality and dependence on a single maintainer. Neither is technical in the narrow sense, and both are addressed in Phase 0.

| # | Area | Risk | Probability | Impact | Mitigation |
| --- | --- | --- | --- | --- | --- |
| R1 | Architecture | Retrieval quality on real documents falls short | Medium | High | Evaluation set from Phase 0; combined search; model and passage size chosen by measurement |
| R2 | Architecture | Worker isolation is harder or slower than planned on Windows | Medium | Medium | Start with process limits; add privilege reduction later; prove it in the Phase 0 spike |
| R3 | Architecture | Corpora outgrow the vector index | Medium | Medium | Index behind an interface; quantised tier; Vec1 as the upgrade path; stated corpus limits |
| R4 | Architecture | Rust slows delivery or deters contributors | Medium | Medium | The two-week Rust test in Phase 0; a thin TypeScript interface for web developers; the extractor guide |
| R5 | Product | Microsoft brings built-in semantic search to ordinary PCs | Medium | High | Compete on auditability, page-level sources, coverage reporting, language breadth and other platforms |
| R6 | Product | The headline use case needs OCR, which arrives in 0.3 | High | Medium | State the limit in the app and README; count scans in Library; move OCR ahead of Office formats if scans exceed about a quarter of testers' libraries |
| R7 | Product | Hours of initial indexing drive users away | Medium | Medium | Keyword-first staging; honest time estimates; background operation |
| R8 | Product | A professional relies on a wrong generated answer (0.4) | Medium | High | Answers are optional and labelled; sources always beside them; citation checks; abstention |
| R9 | Security | A parser flaw is exploited through a crafted file | Low | High | Worker containment; prompt PDFium updates; fuzzing; a signed updater to deliver fixes |
| R10 | Security | A privacy bug sends or exposes data | Low | High | Structural network isolation; CI checks; a network-blocked test; a published statement |
| R11 | Security | The update key is compromised or lost | Low | High | Protected release environment; offline backup; rotation plan |
| R12 | Performance | Embedding is too slow on older laptops | Medium | Medium | Quantised model; resource modes; a later GPU or NPU path |
| R13 | Performance | Memory pressure on 8 GB machines | Medium | Medium | Budgets; worker caps; release the model when idle |
| R14 | Dependency | sqlite-vec or the ONNX Runtime binding stalls or breaks compatibility | Medium | Medium | Both behind interfaces with conformance suites; pinned versions; named alternatives |
| R15 | Dependency | Sourcing and updating PDFium binaries becomes a burden | Medium | Medium | Pinned checksums; scheduled update check; a documented build-from-source fallback |
| R16 | Dependency | A model's licence or availability changes | Low | Medium | Permissive licences only; version pinned in the manifest; weights mirrored with releases |
| R17 | Maintenance | The single maintainer burns out or becomes unavailable | High | High | Strict scope; written release process; a second person with release rights before 1.0 |
| R18 | Maintenance | Three platforms triple packaging and support work | High | Medium | Defer to 1.0; engine tests on all systems early; add one platform at a time |
| R19 | Maintenance | The Store route fails: MSIX packaging breaks the app, or certification is refused | Medium | High | Prove packaging in the Phase 0 spike; the paid fallback is an OV certificate; the GitHub build stays available |
| R20 | Open source | Few or no contributors arrive | High | Low | Plan as if none will; make extractors easy to contribute |
| R21 | Open source | Support and feature requests swamp development | Medium | Medium | Triage rules; a "not planned" list; Discussions for ideas |
| R22 | Open source | A closed or misleading fork trades on the name | Low | Medium | Licence choice; naming policy in GOVERNANCE |
| R23 | Open source | Users attach private documents to public issues | Medium | Medium | Warnings on every form; redacted diagnostics; prompt removal by maintainers |

### Decisions to keep flexible during the MVP

- The embedding model and passage size, until the benchmark.
- The vector index implementation, behind its interface.
- Whether a reranker ships at all.
- The OCR engine and the answer runtime.
- The timing of the SignPath application.
- The interface component library and styling approach.
- Whether the GitHub installer stays once the Store channel is established.
- The order of 0.2 and 0.3.

## 23. Architecture Decision Records

Thirteen decisions deserve a permanent record. Ten are made in substance; three stay open until evidence arrives.

### ADR-1: Desktop shell and engine language

- **Context.** Heavy local processing of untrusted files, a very small team, and an intent to go cross-platform.
- **Options.** Tauri with a Rust engine. Electron with TypeScript and native modules. A Python engine bundled beside either shell.
- **Trade-offs.** Rust gives safety and one binary but a smaller contributor pool. Electron gives reach but size and a second language for heavy work. Python gives the best libraries but unreliable packaging.
- **Direction.** Tauri 2 with a Rust engine.
- **Revisit if** the two-week Rust test at the start of Phase 0 fails (Q2), or a required format has no viable Rust or C library. The replacement would be a language the maintainer already ships in.

### ADR-2: Process model

- **Context.** Parsers crash and can be exploited. Privacy forbids open ports.
- **Options.** One process. Shell plus worker processes. A background Windows service. A local web server with a browser interface.
- **Trade-offs.** Workers add a protocol and supervision. A service needs administrator rights. A local server opens a port.
- **Direction.** Shell and engine in one process, extraction in supervised workers, no service, no port.
- **Revisit if** users need indexing while logged out, or worker start-up dominates throughput on small files.

### ADR-3: The index as rebuildable data in one SQLite file

- **Context.** Text, keyword index and vectors must stay consistent, be easy to delete, and survive corruption.
- **Options.** One SQLite file. SQLite plus separate search and vector stores. LanceDB.
- **Trade-offs.** SQLite has one writer and extension-based vectors. Separate stores are stronger individually but invite consistency bugs.
- **Direction.** One SQLite file for the index; settings stored apart.
- **Revisit if** measured targets are missed at the stated corpus size and swapping the vector index is not enough.

### ADR-4: Vector search behind an interface, exact first

- **Context.** Hundreds of thousands of passages with constant inserts and deletes. sqlite-vec is pre-1.0; SQLite's own Vec1 is emerging.
- **Options.** sqlite-vec, exact or quantised. Vec1. An HNSW library.
- **Trade-offs.** Exact search is simple and always correct but linear. Approximate search scales but adds training or rebuilds and loses some recall.
- **Direction.** sqlite-vec, exact in 0.1 and quantised when needed, plus a conformance suite for the interface.
- **Revisit if** the 95th-percentile query exceeds 500 ms at the supported size, or Vec1 reaches 1.0 with online updates.

### ADR-5: Retrieval strategy

- **Context.** Users search by description and by exact identifier.
- **Options.** Vector only. Keyword only. Both with rank fusion. Both plus a reranker.
- **Trade-offs.** Fusion costs two queries. A reranker adds a model and CPU latency.
- **Direction.** Keyword and vector search fused by reciprocal rank in 0.1. A reranker only on evidence.
- **Revisit if** evaluation shows a reranker gives a clear gain inside the latency budget.

### ADR-6: Embedding runtime and model (model OPEN)

- **Context.** The model fixes vector size, language coverage, licence and the cost of re-indexing.
- **Options.** The shortlist in section 10.
- **Trade-offs.** Multilingual models are larger and may be slightly weaker in English. Larger models index more slowly. Some strong models have restrictive licences.
- **Direction.** ONNX Runtime. A multilingual model (Q1) is chosen by the Phase 0 benchmark from permissively licensed candidates and recorded in the index. The provisional choice is granite-embedding-97m-multilingual-r2; multilingual-e5-small is the baseline it must beat.
- **Revisit if** a clearly better permissive model appears. A change means a background re-embed, so at most once per major version.

### ADR-7: Change detection and content addressing

- **Context.** Files move, duplicate and change while the app is closed. Drives disconnect. Notifications are lossy.
- **Options.** Notifications only. Scan only. Scan plus notifications. The NTFS change journal.
- **Trade-offs.** Scans take time on large trees. Hashing costs a full read. Content addressing adds a level of indirection.
- **Direction.** The scan is the source of truth, notifications speed it up from 0.2, and computed data is keyed by content hash.
- **Revisit if** scans of real libraries take more than a few minutes.

### ADR-8: Network policy and diagnostics

- **Context.** Privacy is the product. Updates and model downloads still need the network. Maintainers need some feedback.
- **Options.** No network at all. Updates and downloads only. Opt-in telemetry.
- **Trade-offs.** Without telemetry the maintainers cannot see usage or failures. An update check reveals an IP address.
- **Direction.** Network code only in the shell, to an allow-list. No telemetry. The user chooses the update check at first launch. Diagnostics are exported by hand.
- **Revisit if** an opt-in, fully inspectable crash report is wanted after 1.0. Telemetry by default is never revisited.

### ADR-9: PDF extraction engine

- **Context.** PDF is the main format and the main attack surface.
- **Options.** PDFium. Pure-Rust libraries. MuPDF. Poppler.
- **Trade-offs.** PDFium is robust but C++. Pure Rust is safer but weaker on real files. MuPDF and Poppler bring copyleft licences.
- **Direction.** PDFium inside the worker.
- **Revisit if** a pure-Rust library matches PDFium on the fixture corpus.

### ADR-10: OCR engine (OPEN)

- **Context.** Scans are central to the primary user. OCR is slow and language-dependent.
- **Options.** PP-OCR models on ONNX Runtime. Tesseract. Windows built-in OCR.
- **Trade-offs.** Runtime reuse against maturity against platform lock-in.
- **Direction.** Reserve the seam now and write the interface with its first implementation, chosen by bake-off before 0.3.
- **Revisit if** user research shows scans dominate, which pulls the decision forward.

### ADR-11: Answer generation (runtime OPEN)

- **Context.** An optional feature with the heaviest packaging and a real risk of wrong answers in professional use.
- **Options.** A bundled llama.cpp worker with one curated model. A local server the user already runs. No answers at all.
- **Trade-offs.** Bundling serves non-technical users but multiplies builds. An external server is cheap to support but demands setup.
- **Direction.** Built in, with the model downloaded the first time answers are switched on (Q16). A "use my own local server" option can follow, restricted to loopback addresses and off by default. The runtime is confirmed before 0.4. Never a cloud provider.
- **Revisit if** small permissive models improve markedly, or operating systems expose local models broadly.

### ADR-12: Licence

- **Context.** Trust and adoption both depend on it, and it is hard to change later.
- **Options and trade-offs.** See section 17.
- **Direction.** Apache-2.0, decided on 3 October 2026 (Q4). GPL and AGPL libraries are excluded as a result.
- **Revisit if** never, in practice, once outside contributions exist.

### ADR-13: Distribution, signing and updates

- **Context.** Unsigned builds are blocked. Signing eligibility varies by country. Updates must be trustworthy.
- **Options.** An installer with an in-app updater. Store distribution as MSIX. Both.
- **Trade-offs.** An own updater means owning key management. The Store means packaging work and Store policies. Two channels mean two builds to test at every release.
- **Direction.** The Microsoft Store first, as an MSIX signed by Microsoft (Q6). A per-user GitHub installer with a signature-verified updater is the second channel, unsigned until SignPath accepts the project after 0.1.
- **Revisit if** the packaging spike fails or certification is refused, in which case an OV certificate is the fallback.

## 24. Future Expansion

After 1.0 the product can grow in several directions without changing its core, provided the interfaces defined now are kept honest.

| Direction | What it adds | What the architecture must preserve |
| --- | --- | --- |
| More formats: email files, legacy Office, e-books, HTML | Wider coverage | The extractor interface and worker protocol; a location model flexible enough for messages |
| Acceleration: GPU, NPU | Faster indexing and answers | The embedding runtime interface; a model manifest that can carry per-device variants |
| Larger libraries: 100,000 documents and more | Scale | The vector index interface, so an approximate index can replace the exact one |
| Managed deployment for firms | Silent install, policy-controlled settings, per-machine installer | Layered configuration (defaults, policy, user) designed into settings from 0.1 |
| Index encryption and portable mode | Stronger protection for regulated users | A relocatable data folder; a storage adapter able to open an encrypted file |
| Richer verification | In-app page rendering with the passage highlighted | Passage offsets and page mapping stored from 0.1 |
| Assistance | File summaries, "more like this", saved searches | The engine's use-case layer; no storage change |
| Integration | A way for other local tools or AI agents to query the index with the user's approval | A deliberate, permissioned design and a new ADR. It must not quietly break the no-open-port rule |

### Directions that would change what the product is

- **Cloud model providers.** They break the core promise. A fork is the right home for that idea.
- **Connectors to cloud drives or live mailboxes.** They put network access next to document text. If ever built, they belong in a separate, clearly isolated component.
- **Shared or team indexes.** A server product with authentication and authorization, not an extension of this one.
- **Telemetry.** Not negotiable for this product.

## 25. Final Architecture Review

Reviewed as a principal engineer would, the design is sound in shape but was too trusting in two places and more elaborate than a solo project needs in several. Fourteen findings follow; each revision is already applied in the sections above.

| # | Finding | Kind | Revision |
| --- | --- | --- | --- |
| 1 | "No network code in the worker" is a property of its code in 0.1, not an enforced limit. An exploited parser could still open a connection | Security | Stated plainly here. The reduced-privilege worker moves from "by 1.0" to 0.2, when Office parsers arrive |
| 2 | An update key stored in CI lets a compromised repository ship an update | Security | Update signatures are produced outside CI with a hardware-backed key. This is also the manual approval step |
| 3 | Separate ports for text index and vector index cannot share one transaction, which undermines ADR-3 | Bad abstraction | Merged into one index-store port. The vector implementation stays swappable inside it |
| 4 | OCR and answer interfaces were specified before any implementation exists | Speculative abstraction | The seams are reserved. Each interface is written with its first implementation |
| 5 | Quantised vectors in 0.1 | Premature optimisation | Exact scan in 0.1. Quantisation only when a measured target is missed |
| 6 | The idle memory target ignored the loaded model | Contradiction | Idle now means the model is released. An in-use target is set from Phase 0 measurements |
| 7 | The 200 MB installer target silently assumes an 8-bit model | Hidden assumption | The benchmark must evaluate the quantised model that would actually ship |
| 8 | File and folder names were not searchable | Missing requirement | SEA-1 now includes them |
| 9 | Locked or half-written files, and edits that keep size and time unchanged, were not handled | Missing requirement | Locked files are retried on the next scan. A deep rescan re-hashes everything. Both added to CORE-1 |
| 10 | Fuzzing every extractor before 0.1 | Over-engineering | Only the worker protocol is fuzzed in 0.1. Extractor fuzzing joins 0.2 with the Office parsers |
| 11 | Six components, full label taxonomy and governance from day one | Over-engineering | The dependency rules are fixed; the component count is not. Phase 0 may start with fewer. Priority labels wait until issue volume justifies them |
| 12 | Interface contributors would need the whole Rust toolchain | Developer experience | The interface runs against a mock engine in a browser. Added to UI-1 |
| 13 | The folder picker could break the ID-only rule | Contradiction | The shell opens the native dialog and returns an ID. The path never passes through the interface |
| 14 | Uninstall removes data by default, but updates must not | Implementation risk | The installer default is customised and covered by the REL-2 tests |

### Reviewed and kept

- **Content addressing.** Adds one level of indirection, but retrofitting it would rewrite the core of the schema.
- **Keyword-first staging.** It is the main answer to hours of embedding, not an optimisation.
- **Two release channels.** Two static files buy a real rollback path.
- **Thirty MUST requirements for 0.1.** Many, but each protects trust or the first-run experience. The Phase 1 internal build is the true "prove the idea" milestone.
- **React.** Replaceable and low-risk. No router or state library is added until three screens outgrow plain component state.

### Weaknesses that remain

- Retrieval quality is unproven until the Phase 0 benchmark. Everything else is secondary to that result.
- The primary user is assumed, not interviewed. Five conversations with lawyers or accountants would test sections 3 to 5 cheaply.
- Nothing here says how people will discover the app. That is outside an architecture specification but not outside the project.
- This document will drift if kept whole. It should be split into the repository's `docs` folder and its ADRs in Phase 0.

### Second pass, after the owner's answers

A further review on 3 October 2026, once the answers in section 26 were applied, led to six more changes.

- **Phase 0 was under-estimated.** It had gained the two-week Rust test and the packaging spike without a new estimate. It is now 4 to 5 weeks, which moves the total to 19 to 28 weeks.
- **Right-to-left documents had no stated treatment.** They are now searchable on a best-effort basis in 0.1 (Q1).
- **The Store has a privacy cost of its own.** Microsoft learns that the app was installed. The network statement must say so.
- **Two channels mean two builds to test.** Recorded in ADR-13. Dropping the GitHub installer later stays an option.
- **"Use my own local server" is not free.** It hands retrieved passages to another program, so it is opt-in and limited to loopback addresses (Q16, ADR-11).
- **A large model download could fail on a small machine.** The app checks available memory before offering it (ANS-3).

## 26. Open Questions

Sixteen of the twenty questions were decided on 3 October 2026. Three still need the owner: the Rust test (Q2), the name (Q5) and the testers (Q14). One needs a measurement: the share of scans (Q9). Right-to-left documents are handled on a best-effort basis in 0.1, a default the owner can change (Q1). Every open item has a solution and a fallback at the end of this section.

### Blocking Phase 0

| # | Question | Status | Answer | Reasoning and consequences |
| --- | --- | --- | --- | --- |
| Q1 | Which languages are users' documents in? | Decided | A multilingual embedding model; English interface | Switching models later means re-indexing every library. Right-to-left documents such as Arabic are searchable in 0.1 on a best-effort basis: passages display in their natural direction, keyword text is normalised, and the evaluation set includes one such language. A mirrored interface waits for translations, and OCR for those scripts is settled in the 0.3 bake-off. Arabic is the test language. Its result decides whether right-to-left support is advertised or labelled experimental |
| Q2 | Is the maintainer productive in Rust? | Open | Settled by a two-week test at the start of Phase 0 | Build the smallest slice in Rust: read one PDF, embed it, store it, search it. If that fails, ADR-1 changes to a language the maintainer already ships in and the estimates are redone |
| Q3 | How many hours a week, and is there a second person? | Decided | One person at 10 hours a week | The low end is used on purpose, because schedules built on the best weeks slip. Section 20 is therefore read at the upper end of each range |
| Q4 | Which licence? | Decided | Apache-2.0 | The easiest for firms to adopt. Closed forks are allowed, and that cost is accepted. GPL and AGPL libraries are excluded, including some popular PDF ones |
| Q5 | What is the product called? | Open | Recommended: Catchword. Fallback: Owndex | A catchword is the word an entry is indexed under, and the keywords that head a law report. On 3 October 2026 its GitHub account, crates.io and npm names were free, and a web search found no search product using it. The owner still has to check the domain, the trademark databases and the Store reservation. It blocks INF-1. The shortlist is below |
| Q6 | Who publishes, and how are builds signed? | Decided | An individual publisher. Microsoft Store first, SignPath Foundation after 0.1 | Store registration is free for individuals and Microsoft signs MSIX packages ([Microsoft](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)). SignPath's [terms](https://signpath.org/terms) require a project already released, with verifiable reputation. Until it accepts, GitHub downloads are unsigned and show a Windows warning |
| Q7 | Is the repository public from the first commit? | Decided | Public with a pre-alpha notice, once the name is chosen | The visible history is the maintainer's portfolio and builds the track record SignPath looks for |
| Q8 | What library size must be supported? | Decided | 10,000 documents for 0.1 and 50,000 by 1.0 | At 10,000 documents, exact search is still fast. 100,000 needs an approximate index, which individuals do not need yet |

### Before 0.1 ships

| # | Question | Status | Answer | Reasoning and consequences |
| --- | --- | --- | --- | --- |
| Q9 | What share of real users' documents are scans? | To measure | Measured early: a 50-file hand count now, then a folder census from the Phase 0 tool | 0.1 counts PDFs with no extractable text and shows "N scanned files skipped". If scans exceed about a quarter of a typical tester's library, OCR moves ahead of Office formats |
| Q10 | Is the update check on or off by default? | Decided | Asked at first launch, pre-selected on | The check fetches one static file and sends nothing about the user. The promise is "your files never leave this computer", not "this app never goes online". Store installs update through Windows |
| Q11 | Is Windows 10 supported? | Decided | Best-effort | Free consumer security updates for Windows 10 now run to 12 October 2027 ([Help Net Security](https://www.helpnetsecurity.com/2026/06/26/microsoft-windows-10-free-security-updates-esu-program/)). The app must install and run there, but only Windows 11 is tested and promised |
| Q12 | Are network shares and cloud-synced folders in scope? | Decided | Local and external drives; cloud-only files skipped; shares best-effort | Never open cloud-only placeholder files, or indexing downloads a whole cloud drive. An unplugged drive is offline, not deleted |
| Q13 | What accessibility level is promised? | Decided | Aim for WCAG 2.2 AA; claim it only after an audit | Until then the wording is "working toward", with known gaps listed. For 0.1 the promise is full keyboard use and screen-reader labels, which can be tested in-house |
| Q14 | Who are the ten outside testers? | Open | To recruit, with a target mix | Three or four document-heavy professionals, two non-technical people, two developers, one 8 GB machine, one person with many scans, one with non-English files. A sign-up link goes in the README from day one |

### Can wait

| # | Question | Status | Answer | Reasoning and consequences |
| --- | --- | --- | --- | --- |
| Q15 | What does "email support" mean? | Decided | `.eml` and `.mbox` files, after 1.0 | Live mailboxes mean handling passwords, which undercuts the trust story. Outlook `.pst` can follow if users ask |
| Q16 | Answers: built in, or the user's own local server? | Decided | Built in, with the model downloaded the first time answers are switched on | Users will not set up a model server, and a multi-gigabyte installer would penalise everyone who only wants search. "Use my own local server" can follow as an opt-in. It is small to build, but must be limited to loopback addresses, because it hands retrieved passages to another program |
| Q17 | Is an encrypted index required? | Decided | Not in the MVP | Rely on the operating system's disk encryption and say so. State plainly that the index holds the full text of every document and is as sensitive as the files |
| Q18 | Are firms a target? | Decided | Individuals first; firms after 1.0 | Firms are where the maintainer's paid freelance work would come from, so nothing should block them. Settings stay in a file an administrator could preset |
| Q19 | Will the project take revenue? | Decided | No revenue | Signing can cost nothing (Q6). SignPath's free programme excludes commercial dual-licensing. Take advice before adding donations or paid support |
| Q20 | Are Windows on ARM builds shipped? | Decided | A native build after 0.1 | The standard build runs on ARM PCs through Windows' built-in emulation, only slower, so nobody is locked out |

### How each remaining item gets closed

Every open item now has a solution, a test for "done", and a fallback. The thresholds are proposals, to be confirmed against the Phase 0 baseline.

| Item | Solution | Done when | Fallback |
| --- | --- | --- | --- |
| Product name (Q5) | Catchword, with Owndex as second choice | Domain bought, trademark search clean, Store name reserved, GitHub organisation created | The next name on the shortlist |
| Rust readiness (Q2) | A 20-hour test over two weeks with a fixed scope and five pass marks | All five pass marks are met | Keep the architecture and change the language: rebuild the same slice in the language the maintainer already ships in, then redo the estimates |
| Ten testers (Q14) | A sign-up form from the first public commit, and active recruiting from Phase 2 through three channels | Ten confirmed testers covering the six profiles | Start the beta with at least five and extend the beta period |
| Right-to-left documents (Q1) | Arabic is the test language in the evaluation set | Recall for Arabic queries is within 10 points of the Latin-script result | Label right-to-left support experimental in the README |
| Embedding model | Provisional choice: granite-embedding-97m-multilingual-r2 in 8-bit form. Baseline: multilingual-e5-small | The Phase 0 benchmark has run on the evaluation set and the reference laptop | Switch to the baseline if Granite is more than 2 points worse on recall@10, or below 20 passages a second where the baseline is not |
| OCR engine | Provisional choice: PP-OCR models on ONNX Runtime, with Tesseract for scripts they read poorly | A bake-off on 200 real pages before 0.3, scored on search recall over the recognised text, pages per minute and added size | Tesseract for every script |
| Answer runtime | Provisional choice: llama.cpp in a worker process. The model is picked at 0.4 by licence, size under 3 GB and a citation test | At least 90% of citations are valid and at least 80% of unanswerable questions are declined | Ship answers as experimental, or not at all |
| Share of scans (Q9) | A hand count of 50 PDFs today, then a folder census from the Phase 0 tool | The result is known by the end of Phase 1 | Keep the planned order |

### Name shortlist

| Name | Why it fits | GitHub account | crates.io | npm | Found in use |
| --- | --- | --- | --- | --- | --- |
| Catchword | The word an entry is indexed under; the keywords heading a law report | Free | Free | Free | No search product found. A branding agency uses the name (from general knowledge) |
| Owndex | "Your own index" | Taken; `owndex-app` is free | Free | Free | Nothing found |
| Tuckaway | Finds what was tucked away | Taken | Free | Free | Not checked |
| Incipit | The opening words that identify an untitled manuscript | Taken | Taken | Taken | Small developer packages. Sounds like "insipid" |
| Muniment | A document kept as proof of rights | Taken | Taken | Free | A storage library on crates.io |
| Unlost | Makes lost files un-lost | Taken | Taken | Taken | A local-first code memory tool, which is too close |

Checked on 3 October 2026 against github.com, crates.io and the npm registry, with one web search each for Catchword and Owndex. Not checked: domain names, trademark databases, and the Microsoft Store. Those three need the owner.

### The Rust test

Twenty hours, in five blocks. Each block ends with something that runs.

1. **Hours 1 to 3.** Install the toolchain. Write a command-line program that reads a text file and counts its words.
2. **Hours 4 to 8.** Extract the text of one PDF, page by page, through the PDFium binding.
3. **Hours 9 to 12.** Split the text into passages and embed them with ONNX Runtime and any small model.
4. **Hours 13 to 16.** Store passages and vectors in SQLite. Query by keyword and by similarity.
5. **Hours 17 to 20.** One command indexes a folder of three PDFs and answers a query with the top five passages and their page numbers. Write one page on what was hard.

Pass marks:

- The final command works on three PDFs it has not seen before.
- It builds from a clean clone on Windows by following written steps.
- The maintainer can explain every line.
- At least one ownership error from the compiler was fixed without copying an answer.
- No more than half the hours went on toolchain and binding problems.

### Recruiting the testers

- **The form.** Five questions: profession; how many documents and what share are scans; languages; Windows version and memory; willingness to report problems.
- **Channels.** Personal and professional contacts for the document-heavy professionals. Local-AI, self-hosting and privacy communities for the developers. A university department or library for research and non-English files.
- **The ask.** One hour to install and index, three searches a day for a week, one feedback form.
- **The promise.** Nothing leaves their machine. They never send documents. Diagnostic exports are optional and readable first.
- **The thanks.** A credit in the release notes, if they want one.

### Measuring the share of scans

- **Today, by hand.** Pick 50 PDFs at random from the folders you would index. Try to select text in each. If you cannot in more than 12, scans exceed a quarter.
- **In Phase 0.** The command-line tool gains a census mode that reports, per folder, how many PDFs have no text layer. The owner and early testers run it and report only the counts.

### The embedding benchmark

- **Candidates.** Both models in the 8-bit form that would ship.
- **Queries.** The evaluation set of at least 100 judged queries, including Arabic and one other target language.
- **Passage sizes.** 200, 350 and 500 tokens.
- **Measures.** Recall@10, nDCG@10, passages per second on the reference laptop, and model size on disk.
- **Rule.** The provisional choice stands unless it loses by the margins in the table above.
