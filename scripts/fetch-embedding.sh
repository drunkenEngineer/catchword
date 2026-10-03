#!/bin/sh
# Download the pinned ONNX Runtime library and embedding model into vendor/
# and check each file against the SHA-256 recorded below. Run it once after
# cloning, and again whenever a pin changes. The app itself never downloads
# them; the release installer will ship them.
#
# Why these sources and how to update them: docs/adr/0018-embedding-runtime-and-model.md
set -eu

ORT_VERSION="1.28.3"
MODEL_NAME="granite-embedding-97m-multilingual-r2"
MODEL_REVISION="835ad14087e140460703cf0fae09f97d469d65c2"
MODEL_SHA256=a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e
TOKENIZER_SHA256=4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f

case "$(uname -s)-$(uname -m)" in
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64)
    ORT_ASSET=onnxruntime-win-x64-$ORT_VERSION.zip
    ORT_SHA256=1d6fab48e85f948436af7c8c971d2c145cf224e2c444755dec894f8b0de11a83
    ORT_LIBRARY=lib/onnxruntime.dll
    ORT_NAME=onnxruntime.dll ;;
  Linux-x86_64)
    ORT_ASSET=onnxruntime-linux-x64-$ORT_VERSION.tgz
    ORT_SHA256=db14e4863bd37893fc59729d986ab2a0d043d10b7d44da1913c4982b7e3d009c
    ORT_LIBRARY=lib/libonnxruntime.so.$ORT_VERSION
    ORT_NAME=libonnxruntime.so ;;
  Darwin-arm64)
    ORT_ASSET=onnxruntime-osx-arm64-$ORT_VERSION.tgz
    ORT_SHA256=c436bd9f47dbce6f6311ccc21de97f829b3c862c09d24aa321958cb72f0a1d32
    ORT_LIBRARY=lib/libonnxruntime.$ORT_VERSION.dylib
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
STAMP="$ORT_ASSET $ORT_SHA256"
if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: ONNX Runtime $ORT_VERSION is already in vendor/onnxruntime."
else
  fetch "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/$ORT_ASSET" \
    "$WORK/$ORT_ASSET" "$ORT_SHA256"
  mkdir "$WORK/ort"
  case "$ORT_ASSET" in
    *.zip) unzip -q "$WORK/$ORT_ASSET" -d "$WORK/ort" ;;
    *) tar -xzf "$WORK/$ORT_ASSET" -C "$WORK/ort" ;;
  esac
  UNPACKED="$WORK/ort/${ORT_ASSET%.*}"
  rm -rf "$DEST"
  mkdir -p "$DEST"
  cp "$UNPACKED/$ORT_LIBRARY" "$DEST/$ORT_NAME"
  cp "$UNPACKED/LICENSE" "$UNPACKED/ThirdPartyNotices.txt" "$DEST/"
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
