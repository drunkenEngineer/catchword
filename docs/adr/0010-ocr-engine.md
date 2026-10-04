# ADR-10: OCR engine

Status: **proposed**. The engine stays open until a comparison before 0.3.

Recorded in section 23 of `docs/specification.md`; copied here on 4 October 2026 (backlog item ARC-1), with what has happened since. The specification stays the source: if the two differ, the specification wins until the owner changes it.

## Context

Scans are central to the primary user. Text recognition (OCR) is slow, and depends on the language.

## Options

1. PP-OCR models on ONNX Runtime.
2. Tesseract.
3. Windows' built-in OCR.

## Trade-offs

Reusing the runtime the app already has, against maturity, against tying the app to one platform.

## Direction

Reserve the seam now. Write the interface together with its first implementation, chosen by comparison before 0.3.

## Revisit if

User research shows that scans dominate. That pulls the decision forward.

## Since then

As of 4 October 2026:

- There is no OCR.
- A PDF page without a text layer is recorded with the reason "needs-ocr". The library lists such files, and an empty search counts them among its likely causes, so they can be read again once OCR arrives.
- The nearest seam is the `Extractor` trait (ADR-21), where a reader for scans would plug in. No OCR interface is written yet.
- The share of scans in real libraries (Q9) has not been measured.
