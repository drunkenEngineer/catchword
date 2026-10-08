#!/bin/sh
# Build the GitHub installer (REL-2): Tauri's NSIS installer, installing for
# the current user only, so it never asks for administrator rights (SEC-4).
# Run from Git Bash on Windows, after scripts/fetch-pdfium.sh and
# scripts/fetch-embedding.sh, with cargo-about installed (see notices.mjs).
#
# It makes target/release/bundle/nsis/Catchword_<version>_x64-setup.exe,
# unsigned until SignPath signing is granted (Q6).
#
# The first run downloads what Tauri builds installers with: NSIS and the
# WebView2 bootstrapper, from their publishers. The installer itself
# downloads nothing, unless WebView2 is missing (never on Windows 11).
set -eu
cd "$(dirname "$0")/.."

MODEL=granite-embedding-97m-multilingual-r2
for file in vendor/pdfium/pdfium.dll vendor/onnxruntime/onnxruntime.dll \
  "vendor/models/$MODEL/model.onnx" "vendor/models/$MODEL/tokenizer.json"; do
  if [ ! -f "$file" ]; then
    echo "Missing $file: run scripts/fetch-pdfium.sh and scripts/fetch-embedding.sh first." >&2
    exit 1
  fi
done

echo "Building the PDF reader and the notices..."
cargo build --release --locked -p catchword-worker
node scripts/notices.mjs --updater

echo "Building the app and its installer..."
# The packaging settings live apart from tauri.conf.json, so that ordinary
# builds do not need the files they bundle.
# With the updater (ADR-24): the GitHub build checks for new versions, if
# the user agrees. Sign a release with scripts/sign-update.sh afterwards.
(cd apps/desktop && ./ui/node_modules/.bin/tauri build --bundles nsis --features updater --config src-tauri/tauri.nsis.conf.json)
ls target/release/bundle/nsis/*-setup.exe
