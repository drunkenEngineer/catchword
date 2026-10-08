// Write target/notices/THIRD-PARTY-NOTICES.txt: everything shipped with
// Catchword that others made, each with its licence text (REL-2). The app
// shows the file in Settings, About; the packages carry it beside the program.
//
//   node scripts/notices.mjs            for the Store package
//   node scripts/notices.mjs --updater  for the GitHub installer, which also
//                                       carries the updater's libraries (ADR-24)
//
// Needs cargo-about (cargo install cargo-about --locked --features cli), and
// PDFium, ONNX Runtime and the model in vendor/ (scripts/fetch-*.sh). Reads
// only local files: nothing is fetched. A Rust library under a licence not
// listed in about.toml stops it.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (...parts) => readFileSync(join(root, ...parts), "utf8").replace(/\r\n/g, "\n").trim();
const MODEL = "granite-embedding-97m-multilingual-r2";
const RULE = "=".repeat(78);
const LINE = "-".repeat(78);

/** The Rust libraries a program uses, by licence text. Our own crates are left out. */
function rustLicences(manifest, into, features = []) {
  const json = execFileSync(
    "cargo",
    ["about", "generate", "--format", "json", "-c", "about.toml", "-m", manifest, "--frozen", "--fail", ...features],
    { cwd: root, encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "inherit"] },
  );
  for (const licence of JSON.parse(json).licenses) {
    const text = licence.text.replace(/\r\n/g, "\n").trim();
    const key = `${licence.id}\n${text}`;
    const entry = into.get(key) ?? { id: licence.id, name: licence.name, text, crates: new Set() };
    for (const { crate } of licence.used_by) {
      if (!crate.name.startsWith("catchword")) entry.crates.add(`${crate.name} ${crate.version}`);
    }
    if (entry.crates.size > 0) into.set(key, entry);
  }
}

/** The interface's JavaScript packages that end up in the app, and their licence files. */
function javascriptPackages() {
  const ui = join(root, "apps", "desktop", "ui");
  const seen = new Map();
  const walk = (name) => {
    if (seen.has(name)) return;
    const folder = join(ui, "node_modules", name);
    const manifest = JSON.parse(readFileSync(join(folder, "package.json"), "utf8"));
    const files = readdirSync(folder)
      .filter((file) => /^(licen[cs]e|copying|notice)/i.test(file) && !file.endsWith(".spdx"))
      .sort();
    seen.set(name, { version: manifest.version, licence: manifest.license, files: files.map((f) => join(folder, f)) });
    for (const dependency of Object.keys(manifest.dependencies ?? {})) walk(dependency);
  };
  const app = JSON.parse(readFileSync(join(ui, "package.json"), "utf8"));
  for (const dependency of Object.keys(app.dependencies)) walk(dependency);
  return [...seen.entries()].sort(([a], [b]) => a.localeCompare(b));
}

const rust = new Map();
const updater = process.argv.includes("--updater") ? ["--features", "updater"] : [];
rustLicences("apps/desktop/src-tauri/Cargo.toml", rust, updater);
rustLicences("crates/worker/Cargo.toml", rust);
const version = JSON.parse(readFileSync(join(root, "apps", "desktop", "src-tauri", "tauri.conf.json"), "utf8")).version;

const out = [];
const section = (title) => out.push("", RULE, title, RULE);
const part = (title, text) => out.push("", LINE, title, LINE, "", text);

out.push(
  `Third-party notices for Catchword ${version}`,
  "",
  "Catchword is free software under the Apache License 2.0 (see LICENSE). It",
  "includes the parts below, made by others, each under its own licence.",
);

section("Rust libraries");
const licences = [...rust.values()].sort((a, b) => a.id.localeCompare(b.id) || a.text.localeCompare(b.text));
for (const licence of licences) {
  const crates = [...licence.crates].sort((a, b) => a.localeCompare(b));
  part(`${licence.name} (${licence.id})\nUsed by: ${crates.join(", ")}`, licence.text);
}

section("JavaScript libraries, in the interface");
for (const [name, { version: v, licence, files }] of javascriptPackages()) {
  part(`${name} ${v} (${licence})`, files.map((file) => readFileSync(file, "utf8").replace(/\r\n/g, "\n").trim()).join("\n\n"));
}

section("PDFium, which reads PDF files");
part(`PDFium (${read("vendor", "pdfium", "VERSION").split("\n")[0]})`, read("vendor", "pdfium", "LICENSE"));
for (const file of readdirSync(join(root, "vendor", "pdfium", "licenses")).sort()) {
  part(`PDFium includes: ${file.replace(/\.[^.]+$/, "")}`, read("vendor", "pdfium", "licenses", file));
}

section("ONNX Runtime, which runs the embedding model");
part("ONNX Runtime (MIT)", read("vendor", "onnxruntime", "LICENSE"));
part("ONNX Runtime's third-party notices", read("vendor", "onnxruntime", "ThirdPartyNotices.txt"));

section("The embedding model");
part(
  `${MODEL} (Apache-2.0), by IBM`,
  `${read("vendor", "models", MODEL, "SOURCE")}\n\nIts licence is the Apache License 2.0, the same text as Catchword's own LICENSE.`,
);

mkdirSync(join(root, "target", "notices"), { recursive: true });
const file = join(root, "target", "notices", "THIRD-PARTY-NOTICES.txt");
writeFileSync(file, out.join("\n") + "\n");
const crates = licences.reduce((n, l) => n + l.crates.size, 0);
console.log(`Wrote ${file}: ${crates} Rust library uses under ${licences.length} licence texts.`);
