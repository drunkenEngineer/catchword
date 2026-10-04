# Security policy

## Reporting a vulnerability

Please report privately through GitHub: **Security → Report a vulnerability**. Do not open a public issue.

You will get an acknowledgement within 7 days. The project is maintained part-time.

## Supported versions

Before 1.0, only the latest release is supported.

## Threat model

What Catchword defends against, what is in place for each threat, and the review checklist for the code that decides its security: [docs/threat-model.md](docs/threat-model.md).

## In scope

- A crafted file that makes Catchword run code, crash, or read outside the chosen folders.
- Anything that causes document text, queries or file names to leave the computer.
- Weaknesses in how updates are verified.

## Out of scope

- Malware already running as the same Windows user. It can read the documents directly.
