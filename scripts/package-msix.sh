#!/bin/sh
# Build the Windows app and pack it as an MSIX (REL-1). Run from Git Bash on
# Windows, after scripts/fetch-pdfium.sh and scripts/fetch-embedding.sh.
#
# It makes:
#   target/package/Catchword/       every file the package installs
#   target/package/Catchword.msix   the package, unsigned
#
# To install it on this PC for a test, see docs/packaging.md. Nothing here
# downloads anything: makeappx comes with the Windows SDK, which the Visual
# Studio C++ build tools install.
set -eu
cd "$(dirname "$0")/.."

MODEL=granite-embedding-97m-multilingual-r2
OUT=target/package
LAYOUT=$OUT/Catchword
# The workspace version, plus the fourth number MSIX needs (always 0).
VERSION=$(sed -n 's/^version = "\([0-9.]*\)"$/\1/p' Cargo.toml | head -n 1).0

for file in vendor/pdfium/pdfium.dll vendor/onnxruntime/onnxruntime.dll \
  "vendor/models/$MODEL/model.onnx" "vendor/models/$MODEL/tokenizer.json"; do
  if [ ! -f "$file" ]; then
    echo "Missing $file: run scripts/fetch-pdfium.sh and scripts/fetch-embedding.sh first." >&2
    exit 1
  fi
done

MAKEAPPX=$(ls "/c/Program Files (x86)/Windows Kits/10/bin/"*/x64/makeappx.exe 2>/dev/null | sort -V | tail -n 1)
if [ -z "$MAKEAPPX" ]; then
  echo "makeappx.exe not found: install the Windows SDK (it comes with the C++ build tools)." >&2
  exit 1
fi

echo "Building the app and the PDF reader..."
(cd apps/desktop && ./ui/node_modules/.bin/tauri build --no-bundle)
cargo build --release --locked -p catchword-worker

echo "Assembling $LAYOUT..."
rm -rf "$OUT"
mkdir -p "$LAYOUT/models/$MODEL" "$LAYOUT/Assets" "$LAYOUT/licenses/pdfium" "$LAYOUT/licenses/onnxruntime"
# The program and what it loads by full path from its own folder.
cp target/release/catchword-desktop.exe "$LAYOUT/Catchword.exe"
cp target/release/catchword-worker.exe vendor/pdfium/pdfium.dll vendor/onnxruntime/onnxruntime.dll "$LAYOUT/"
cp "vendor/models/$MODEL/model.onnx" "vendor/models/$MODEL/tokenizer.json" "$LAYOUT/models/$MODEL/"
cp apps/desktop/msix/Assets/*.png "$LAYOUT/Assets/"
sed "s/{VERSION}/$VERSION/" apps/desktop/msix/AppxManifest.xml > "$LAYOUT/AppxManifest.xml"
# Catchword's licence, and those of the libraries and model it bundles.
# The Rust crates' notices are not generated yet (see docs/packaging.md).
cp LICENSE NOTICE "$LAYOUT/"
cp vendor/pdfium/LICENSE vendor/pdfium/licenses/* "$LAYOUT/licenses/pdfium/"
cp vendor/onnxruntime/LICENSE vendor/onnxruntime/ThirdPartyNotices.txt "$LAYOUT/licenses/onnxruntime/"
cp "vendor/models/$MODEL/SOURCE" "$LAYOUT/licenses/$MODEL.txt"

echo "Packing..."
# Git Bash would turn makeappx's /options into paths; this stops it.
MSYS_NO_PATHCONV=1 "$MAKEAPPX" pack /o /d "$(cygpath -w "$LAYOUT")" /p "$(cygpath -w "$OUT/Catchword.msix")"
echo "Made $OUT/Catchword.msix, version $VERSION."
