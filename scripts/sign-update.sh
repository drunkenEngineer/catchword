#!/bin/sh
# Sign the GitHub installer for the updater, and write its manifest,
# latest.json (REL-3, ADR-24). The owner runs this at release time, after
# scripts/package-nsis.sh. The private key never leaves their computer, and
# Tauri asks for its password.
#
#   sh scripts/sign-update.sh
#
# Then attach the installer, its .sig and latest.json to the GitHub release
# v<version>. The app finds the newest release's latest.json.
#
# NOTES sets the text the app shows with the update; the default points to
# the changelog. For a test: KEY, VERSION and INSTALLER override the defaults.
set -eu
cd "$(dirname "$0")/.."

VERSION=${VERSION:-$(sed -n 's/^version = "\([0-9.]*\)"$/\1/p' Cargo.toml | head -n 1)}
NAME="Catchword_${VERSION}_x64-setup.exe"
INSTALLER=${INSTALLER:-target/release/bundle/nsis/$NAME}
KEY=${KEY:-$HOME/.tauri/catchword-update.key}
REPOSITORY=https://github.com/drunkenEngineer/catchword
NOTES=${NOTES:-"What changed: $REPOSITORY/blob/main/CHANGELOG.md"}

if [ ! -f "$INSTALLER" ]; then
  echo "No installer at $INSTALLER. Build it first: sh scripts/package-nsis.sh" >&2
  exit 1
fi
if [ ! -f "$KEY" ]; then
  echo "No private key at $KEY. Set KEY to where it is kept." >&2
  exit 1
fi

# The version goes into the signature itself: the app refuses an update
# whose manifest names another version (requireSignedVersion).
TAURI=$(pwd)/apps/desktop/ui/node_modules/.bin/tauri
"$TAURI" signer sign --private-key-path "$KEY" --app-version "$VERSION" "$INSTALLER"

node - "$INSTALLER.sig" "$VERSION" "$REPOSITORY/releases/download/v$VERSION/$NAME" "$NOTES" "$(dirname "$INSTALLER")/latest.json" <<'EOF'
const [sigFile, version, url, notes, out] = process.argv.slice(2);
const fs = require("node:fs");
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms: { "windows-x86_64": { signature: fs.readFileSync(sigFile, "utf8").trim(), url } },
};
fs.writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`Wrote ${out}`);
EOF
echo "Attach to the GitHub release v$VERSION: $INSTALLER, $INSTALLER.sig and latest.json."
