#!/bin/sh
# Download the pinned ONNX Runtime library and embedding model into vendor/
# and check each file against the SHA-256 recorded below. Run it once after
# cloning, and again whenever a pin changes. The app itself never downloads
# them; the release installer will ship them.
#
# Why these sources and how to update them: docs/adr/0018-embedding-runtime-and-model.md
set -eu

# ONNX Runtime comes from Microsoft's NuGet package, which holds the library
# for every system. Its GitHub releases disappeared in November 2026; see
# ADR-18's update. The package's Microsoft signature was checked when this
# checksum was pinned (dotnet nuget verify --all).
ORT_VERSION="1.31.1"
ORT_PACKAGE=microsoft.ml.onnxruntime.$ORT_VERSION.nupkg
ORT_SHA256=6871ccbc97f3d5bdb4095a1dcec33aa78d56f6ed8de5980ef5fa1d9f2e7c679a
MODEL_NAME="granite-embedding-97m-multilingual-r2"
MODEL_REVISION="835ad14087e140460703cf0fae09f97d469d65c2"
MODEL_SHA256=a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e
TOKENIZER_SHA256=4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f

case "$(uname -s)-$(uname -m)" in
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64)
    ORT_LIBRARY=runtimes/win-x64/native/onnxruntime.dll
    ORT_NAME=onnxruntime.dll ;;
  Linux-x86_64)
    ORT_LIBRARY=runtimes/linux-x64/native/libonnxruntime.so
    ORT_NAME=libonnxruntime.so ;;
  Darwin-arm64)
    ORT_LIBRARY=runtimes/osx-arm64/native/libonnxruntime.dylib
    ORT_NAME=libonnxruntime.dylib ;;
  *)
    echo "FAIL: no pinned ONNX Runtime build for $(uname -s) $(uname -m)." >&2
    exit 1 ;;
esac

ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# fetch URL FILE SHA256: download URL to FILE, or stop if its checksum differs.
fetch() {
  curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"
  actual=$(sha256_of "$2")
  if [ "$actual" != "$3" ]; then
    echo "FAIL: $(basename "$2") does not match its pinned checksum. Nothing was installed." >&2
    echo "  expected $3" >&2
    echo "  received $actual" >&2
    exit 1
  fi
}

# ONNX Runtime: the library and its licence texts.
DEST="$ROOT/vendor/onnxruntime"
STAMP="$ORT_PACKAGE $ORT_SHA256 $ORT_LIBRARY"
if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: ONNX Runtime $ORT_VERSION is already in vendor/onnxruntime."
else
  fetch "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/$ORT_VERSION/$ORT_PACKAGE" \
    "$WORK/$ORT_PACKAGE" "$ORT_SHA256"
  mkdir "$WORK/ort"
  # A NuGet package is a zip archive: take this system's library and the
  # licence texts only.
  unzip -q "$WORK/$ORT_PACKAGE" "$ORT_LIBRARY" LICENSE ThirdPartyNotices.txt -d "$WORK/ort"
  rm -rf "$DEST"
  mkdir -p "$DEST"
  cp "$WORK/ort/$ORT_LIBRARY" "$DEST/$ORT_NAME"
  cp "$WORK/ort/LICENSE" "$WORK/ort/ThirdPartyNotices.txt" "$DEST/"
  printf '%s\n' "$STAMP" > "$DEST/STAMP"
  echo "OK: ONNX Runtime $ORT_VERSION matched its checksum and is in vendor/onnxruntime."
fi

# The embedding model and its tokenizer, from one fixed revision.
DEST="$ROOT/vendor/models/$MODEL_NAME"
STAMP="$MODEL_REVISION $MODEL_SHA256 $TOKENIZER_SHA256"
if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: the $MODEL_NAME model is already in vendor/models."
else
  BASE="https://huggingface.co/ibm-granite/$MODEL_NAME/resolve/$MODEL_REVISION"
  fetch "$BASE/onnx/model_quint8_avx2.onnx" "$WORK/model.onnx" "$MODEL_SHA256"
  fetch "$BASE/tokenizer.json" "$WORK/tokenizer.json" "$TOKENIZER_SHA256"
  rm -rf "$DEST"
  mkdir -p "$DEST"
  mv "$WORK/model.onnx" "$WORK/tokenizer.json" "$DEST/"
  cat > "$DEST/SOURCE" <<EOF
ibm-granite/$MODEL_NAME, revision $MODEL_REVISION
https://huggingface.co/ibm-granite/$MODEL_NAME
model.onnx is onnx/model_quint8_avx2.onnx. Licence: Apache-2.0.
EOF
  printf '%s\n' "$STAMP" > "$DEST/STAMP"
  echo "OK: the $MODEL_NAME model matched its checksums and is in vendor/models."
fi
