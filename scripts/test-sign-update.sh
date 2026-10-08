#!/bin/sh
# Test scripts/sign-update.sh with a throwaway key and a stand-in installer:
# it must sign, bind the version into the signature, and write a manifest
# the updater reads. Needs the interface's packages (npm ci in apps/desktop/ui).
set -eu
cd "$(dirname "$0")/.."

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
TAURI=$(pwd)/apps/desktop/ui/node_modules/.bin/tauri

"$TAURI" signer generate --ci -p "" -w "$WORK/test.key" >/dev/null
printf 'not a real installer\n' > "$WORK/Catchword_9.8.7_x64-setup.exe"

TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" KEY="$WORK/test.key" VERSION=9.8.7 \
  INSTALLER="$WORK/Catchword_9.8.7_x64-setup.exe" NOTES="A test release." \
  sh scripts/sign-update.sh >/dev/null

node - "$WORK" <<'EOF'
const fs = require("node:fs");
const work = process.argv[2];
const fail = (why) => { console.error(`FAIL: ${why}`); process.exit(1); };
const manifest = JSON.parse(fs.readFileSync(`${work}/latest.json`, "utf8"));
const signature = fs.readFileSync(`${work}/Catchword_9.8.7_x64-setup.exe.sig`, "utf8").trim();
const windows = manifest.platforms?.["windows-x86_64"];
if (manifest.version !== "9.8.7") fail(`version ${manifest.version}`);
if (manifest.notes !== "A test release.") fail(`notes ${manifest.notes}`);
if (Number.isNaN(Date.parse(manifest.pub_date))) fail(`pub_date ${manifest.pub_date}`);
if (!windows) fail("no windows-x86_64 entry");
if (windows.url !== "https://github.com/drunkenEngineer/catchword/releases/download/v9.8.7/Catchword_9.8.7_x64-setup.exe")
  fail(`url ${windows.url}`);
if (windows.signature !== signature) fail("the manifest's signature is not the .sig file's");
// The signature is a minisign signature, base64-encoded; its trusted
// comment, which the signature covers, must name the version.
const decoded = Buffer.from(signature, "base64").toString("utf8");
if (!/trusted comment:.*9\.8\.7/.test(decoded)) fail(`the version is not in the signature:\n${decoded}`);
console.log("OK: the installer is signed for its version, and latest.json is right.");
EOF
