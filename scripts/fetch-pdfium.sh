#!/bin/sh
# Download the pinned PDFium build for this machine into vendor/pdfium/ and
# check it against the SHA-256 recorded below. Run it once after cloning, and
# again whenever RELEASE changes. The app itself never downloads PDFium.
#
# Why this source and how to update it: docs/adr/0014-pdfium-binaries.md
set -eu

RELEASE="chromium/8076"

case "$(uname -s)-$(uname -m)" in
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64)
    ASSET=pdfium-win-x64.tgz
    SHA256=808d36da9bc5a3104315fb307c80998121f565ee53953633bf33e80d7429e5ac
    LIBRARY=bin/pdfium.dll ;;
  Linux-x86_64)
    ASSET=pdfium-linux-x64.tgz
    SHA256=d9d67bc40af03aef4fe28a60b19b1086f28ace019c8c9caf19cb7fe3d14ceca3
    LIBRARY=lib/libpdfium.so ;;
  Darwin-arm64)
    ASSET=pdfium-mac-arm64.tgz
    SHA256=0d6781fe08906baff3d82c90953e519fbc4eb253fe76431e5ed53b157763b97c
    LIBRARY=lib/libpdfium.dylib ;;
  Darwin-x86_64)
    ASSET=pdfium-mac-x64.tgz
    SHA256=40865f34642c34d82cc336132df9e0347133f4692cd46776647af160f9a5cca9
    LIBRARY=lib/libpdfium.dylib ;;
  *)
    echo "FAIL: no pinned PDFium build for $(uname -s) $(uname -m)." >&2
    exit 1 ;;
esac

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST="$ROOT/vendor/pdfium"
STAMP="$RELEASE $ASSET $SHA256"

if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: PDFium $RELEASE is already in vendor/pdfium."
  exit 0
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

curl -fsSL --proto '=https' --tlsv1.2 -o "$WORK/$ASSET" \
  "https://github.com/bblanchon/pdfium-binaries/releases/download/$RELEASE/$ASSET"

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "$WORK/$ASSET" | cut -d ' ' -f 1)
else
  ACTUAL=$(shasum -a 256 "$WORK/$ASSET" | cut -d ' ' -f 1)
fi
if [ "$ACTUAL" != "$SHA256" ]; then
  echo "FAIL: $ASSET does not match its pinned checksum. Nothing was installed." >&2
  echo "  expected $SHA256" >&2
  echo "  received $ACTUAL" >&2
  exit 1
fi

mkdir "$WORK/unpacked"
tar -xzf "$WORK/$ASSET" -C "$WORK/unpacked"
rm -rf "$DEST"
mkdir -p "$DEST"
cp "$WORK/unpacked/$LIBRARY" "$DEST/"
cp -R "$WORK/unpacked/licenses" "$DEST/licenses"
for file in LICENSE VERSION; do
  if [ -f "$WORK/unpacked/$file" ]; then cp "$WORK/unpacked/$file" "$DEST/"; fi
done
printf '%s\n' "$STAMP" > "$DEST/STAMP"
echo "OK: PDFium $RELEASE ($ASSET) matched its checksum and is in vendor/pdfium."
