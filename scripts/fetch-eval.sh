#!/bin/sh
# Download what the retrieval evaluation needs, and nothing the app needs:
# the XQuAD dataset (English, German, Arabic) and the baseline embedding
# model it is compared with. Every file is checked against the SHA-256
# recorded below. Files go to vendor/, which is not committed.
#
# What the evaluation is: eval/README.md. Why these sources: docs/adr/0019-evaluation-set.md
set -eu

XQUAD_COMMIT="7d30520c717524000f0d9d2f9c10a069acd9d285"
XQUAD_EN=e4c57d1c9143aaa1c5d265ba5987a65f4e69528d2a98f29d6e75019b10344f29
XQUAD_DE=990b5d746746ed65ed4702ea5f35f99ffa4e2f1c390c07d003642acd937916f9
XQUAD_AR=abdabd7afed5c635d99cca0f3f0d0c9d9ed0bc77451e963c2e4e0638c29e486d

BASELINE_NAME="multilingual-e5-small"
BASELINE_REVISION="614241f622f53c4eeff9890bdc4f31cfecc418b3"
BASELINE_MODEL=dd476dd0c2514e9b9be83aeb3853fac0763e0bdf4a71645407587d77c48a2d88
BASELINE_TOKENIZER=0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39

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

# XQuAD: 240 Wikipedia paragraphs with 1,190 questions, translated in parallel.
DEST="$ROOT/vendor/eval/xquad"
STAMP="$XQUAD_COMMIT $XQUAD_EN $XQUAD_DE $XQUAD_AR"
if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: XQuAD is already in vendor/eval/xquad."
else
  BASE="https://raw.githubusercontent.com/google-deepmind/xquad/$XQUAD_COMMIT"
  fetch "$BASE/xquad.en.json" "$WORK/xquad.en.json" "$XQUAD_EN"
  fetch "$BASE/xquad.de.json" "$WORK/xquad.de.json" "$XQUAD_DE"
  fetch "$BASE/xquad.ar.json" "$WORK/xquad.ar.json" "$XQUAD_AR"
  curl -fsSL --proto '=https' --tlsv1.2 -o "$WORK/LICENSE.txt" "$BASE/CC-BY-SA4.0.txt"
  rm -rf "$DEST"
  mkdir -p "$DEST"
  mv "$WORK"/xquad.*.json "$WORK/LICENSE.txt" "$DEST/"
  printf '%s\n' "$STAMP" > "$DEST/STAMP"
  echo "OK: XQuAD matched its checksums and is in vendor/eval/xquad."
fi

# The baseline model the spec compares the provisional model with (ADR-6).
DEST="$ROOT/vendor/models/$BASELINE_NAME"
STAMP="$BASELINE_REVISION $BASELINE_MODEL $BASELINE_TOKENIZER"
if [ -f "$DEST/STAMP" ] && [ "$(cat "$DEST/STAMP")" = "$STAMP" ]; then
  echo "OK: the $BASELINE_NAME model is already in vendor/models."
else
  BASE="https://huggingface.co/intfloat/$BASELINE_NAME/resolve/$BASELINE_REVISION"
  fetch "$BASE/onnx/model_qint8_avx512_vnni.onnx" "$WORK/model.onnx" "$BASELINE_MODEL"
  fetch "$BASE/onnx/tokenizer.json" "$WORK/tokenizer.json" "$BASELINE_TOKENIZER"
  rm -rf "$DEST"
  mkdir -p "$DEST"
  mv "$WORK/model.onnx" "$WORK/tokenizer.json" "$DEST/"
  cat > "$DEST/SOURCE" <<EOF
intfloat/$BASELINE_NAME, revision $BASELINE_REVISION
https://huggingface.co/intfloat/$BASELINE_NAME
model.onnx is onnx/model_qint8_avx512_vnni.onnx. Licence: MIT.
EOF
  printf '%s\n' "$STAMP" > "$DEST/STAMP"
  echo "OK: the $BASELINE_NAME model matched its checksums and is in vendor/models."
fi
