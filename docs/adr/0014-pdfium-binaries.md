# ADR-14: Where the PDFium library comes from

Status: accepted, 3 October 2026. Builds on ADR-9 (PDFium inside the worker).

## Context

PDFium is a large C++ library. Building it from source needs Chromium's build tools and hours of compile time, which a one-person project cannot afford on every machine and CI run. A prebuilt library is a supply-chain risk (threat T8), so where it comes from and how it is checked must be written down.

## Options

1. Build PDFium from Google's source ourselves.
2. Use the prebuilt releases of [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries).
3. Use a PDFium copy shipped by another product.

## Trade-offs

- Option 1 gives full control, but costs hours per build and a second build system to maintain.
- Option 2 is built weekly from Google's source by public GitHub Actions. Each release publishes SHA-256 digests and a build attestation, and it is the source the `pdfium-render` binding documents. We depend on one maintainer's pipeline.
- Option 3 has unclear provenance and update timing.

## Direction

- Option 2, pinned to one release (currently `chromium/8076`, PDFium 156.0.8076.0, 29 September 2026).
- `scripts/fetch-pdfium.sh` downloads the build for the current platform and refuses it unless its SHA-256 matches the value written in the script. Nothing is downloaded when the app runs.
- The files go to `vendor/pdfium/`, which is not committed. CI runs the same script.
- Release builds of the worker load the library only from the worker's own folder, by absolute path (threat T17).

### Licences

- PDFium: BSD-3-Clause and Apache-2.0.
- The libraries compiled into it (FreeType, libjpeg-turbo, OpenJPEG, Little CMS, libpng, zlib, ICU, Abseil, fast_float, simdutf, AGG, LLVM libc) are permissive. FreeType is offered under the FreeType License (FTL) or GPLv2; we use it under the FTL, which requires a credit in the documentation.
- The download includes these texts in `licenses/`. Any installer must ship that folder and credit FreeType (REL-2).

## Updating PDFium

1. Pick the new release tag and read its notes.
2. Copy each platform's `digest` from `https://api.github.com/repos/bblanchon/pdfium-binaries/releases/tags/<tag>`.
3. Change `RELEASE` and the four checksums in the script, run it, and run the full test suite.
4. Keep the version current: PDFium parses untrusted files, so security fixes matter more than stability of the pin.

## Revisit if

- The binaries project stops publishing regularly or its build stops being public.
- We need build options it does not offer.
