#!/bin/sh
# From a fresh clone to a working build, in one command (INF-3): checks the
# tools, installs the pinned Rust, downloads PDFium, ONNX Runtime, the
# embedding model and the evaluation data (each checked against a pinned
# checksum), installs the interface's packages, and builds everything.
# Safe to run again: what is already there is kept.
#
#   sh scripts/setup.sh          set everything up
#   sh scripts/setup.sh --check  only check that the tools are installed
#
# On Windows, run it from Git Bash.
set -eu
cd "$(dirname "$0")/.."

# Vite and Vitest need Node.js 22.12; jsdom, used only by the interface
# tests, asks for 22.22.2, though they pass on 22.20 with a warning.
NODE_MIN_MINOR=12
NODE_WANTED_MINOR=22

missing=0
need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing: $1. $2" >&2
    missing=1
  fi
}

need cargo "Install Rust from https://rustup.rs."
need rustup "Install Rust from https://rustup.rs: it brings rustup."
need node "Install Node.js 22 (22.22.2 or later) from https://nodejs.org."
need npm "It comes with Node.js."
need curl "The downloads use it. Git for Windows includes it."
if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
  echo "Missing: sha256sum or shasum. The downloads are checked with one of them." >&2
  missing=1
fi

if command -v node >/dev/null 2>&1; then
  version=$(node -p "process.versions.node")
  major=${version%%.*}
  rest=${version#*.}
  minor=${rest%%.*}
  if [ "$major" -lt 22 ] || { [ "$major" -eq 22 ] && [ "$minor" -lt "$NODE_MIN_MINOR" ]; }; then
    echo "Too old: Node.js $version. Install 22 (22.22.2 or later) from https://nodejs.org." >&2
    missing=1
  elif [ "$major" -eq 22 ] && [ "$minor" -lt "$NODE_WANTED_MINOR" ]; then
    echo "Note: Node.js $version works, but the interface tests' jsdom asks for 22.22.2 or later, so npm will warn. Update when you can."
  fi
fi

# The desktop app needs WebKitGTK on Linux (Tauri 2).
if [ "$(uname -s)" = "Linux" ]; then
  if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists webkit2gtk-4.1; then
    echo "Missing: the desktop app's Linux libraries. On Debian or Ubuntu:" >&2
    echo "  sudo apt-get install -y libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config" >&2
    missing=1
  fi
fi

if [ "$missing" -ne 0 ]; then
  echo "Install what is missing above, then run this again." >&2
  exit 1
fi
echo "OK: the tools are installed."
if [ "${1:-}" = "--check" ]; then
  exit 0
fi

echo "Installing the Rust version pinned in rust-toolchain.toml..."
rustup toolchain install

echo "Downloading PDFium, ONNX Runtime, the model and the evaluation data..."
sh scripts/fetch-pdfium.sh
sh scripts/fetch-embedding.sh
sh scripts/fetch-eval.sh

echo "Installing the interface's packages..."
(cd apps/desktop/ui && npm ci --no-fund --no-audit)

echo "Building everything (the first time takes several minutes)..."
cargo build --workspace

cat <<'NEXT'

Ready. Try it on the sample letters in eval/domain/docs:

  cargo run -p catchword -- index eval/domain/docs
  cargo run -p catchword -- search "tax refund"

Run the tests:   cargo test --workspace
Run the app:     cd apps/desktop && ./ui/node_modules/.bin/tauri dev
NEXT
