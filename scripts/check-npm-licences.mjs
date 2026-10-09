// Every package the interface uses must be under a licence on the list in
// deny.toml, the same list as the Rust libraries (MNT-3). cargo-deny checks
// only Rust; this checks apps/desktop/ui/package-lock.json, which records
// each package's licence. Known security problems are npm's own check:
// `npm audit` in apps/desktop/ui.
//
//   node scripts/check-npm-licences.mjs
//   node --test scripts/check-npm-licences.test.mjs   (its tests)
//
// Reads only local files: nothing is fetched.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/**
 * Allowed only for packages that build or test the interface, never shipped
 * in the app. Both are permissive and approved by the OSI, and ask no more
 * than MIT does: BlueOak-1.0.0 (lru-cache) and MIT-0, MIT without even the
 * notice (two @csstools packages).
 */
export const BUILD_AND_TEST_ONLY = ["BlueOak-1.0.0", "MIT-0"];

/** The `allow` list in deny.toml's [licenses] section. */
export function allowedLicences(denyToml) {
  const section = /^\[licenses\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(denyToml.replace(/\r\n/g, "\n"));
  const list = section && /^allow\s*=\s*\[([\s\S]*?)\]/m.exec(section[1]);
  if (!list) throw new Error("deny.toml has no allow list under [licenses]");
  return [...list[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

/**
 * Whether an SPDX licence expression is satisfied by the allowed licences:
 * either side of OR, both sides of AND, and "X WITH Y" only as listed.
 */
export function satisfies(expression, allowed) {
  const tokens = expression.match(/\(|\)|[^\s()]+/g) ?? [];
  let at = 0;
  const peek = () => tokens[at]?.toUpperCase();
  const fail = () => {
    throw new Error(`cannot read the licence "${expression}"`);
  };
  const either = () => {
    let ok = both();
    while (peek() === "OR") {
      at += 1;
      ok = both() || ok;
    }
    return ok;
  };
  const both = () => {
    let ok = one();
    while (peek() === "AND") {
      at += 1;
      ok = one() && ok;
    }
    return ok;
  };
  const one = () => {
    const token = tokens[at++];
    if (token === "(") {
      const ok = either();
      if (tokens[at++] !== ")") fail();
      return ok;
    }
    if (token === undefined || token === ")" || ["AND", "OR", "WITH"].includes(token.toUpperCase())) fail();
    if (peek() === "WITH") {
      const exception = tokens[at + 1];
      if (exception === undefined) fail();
      at += 2;
      return allowed.has(`${token} WITH ${exception}`);
    }
    return allowed.has(token);
  };
  const ok = either();
  if (at !== tokens.length) fail();
  return ok;
}

/**
 * The packages in a package-lock.json (version 2 or 3) whose licence is not
 * allowed, missing or unreadable. A package marked `dev` is used only to
 * build and test, so it may also use BUILD_AND_TEST_ONLY.
 */
export function licenceProblems(lock, allowed) {
  const shipped = new Set(allowed);
  const tooling = new Set([...allowed, ...BUILD_AND_TEST_ONLY]);
  const problems = [];
  for (const [path, entry] of Object.entries(lock.packages ?? {})) {
    if (path === "" || entry.link) continue; // the interface itself, or a link to it
    const name = path.replace(/^.*node_modules\//, "");
    const use = entry.dev ? "build and test only" : "shipped in the app";
    const licence = entry.license;
    let ok = false;
    let reason = "not on the allowed list";
    if (typeof licence !== "string" || licence.trim() === "") {
      reason = "no licence recorded";
    } else {
      try {
        ok = satisfies(licence, entry.dev ? tooling : shipped);
      } catch (error) {
        reason = error.message;
      }
    }
    if (!ok) problems.push(`${name} ${entry.version ?? "?"} (${use}): ${licence ?? "none"}: ${reason}`);
  }
  return problems;
}

function main() {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const allowed = allowedLicences(readFileSync(join(root, "deny.toml"), "utf8"));
  const lock = JSON.parse(readFileSync(join(root, "apps", "desktop", "ui", "package-lock.json"), "utf8"));
  const problems = licenceProblems(lock, allowed);
  if (problems.length > 0) {
    for (const problem of problems) console.log(problem);
    console.error("FAIL: the interface packages above are not under a licence deny.toml allows.");
    process.exit(1);
  }
  const count = Object.keys(lock.packages).filter((path) => path !== "").length;
  console.log(`OK: all ${count} interface packages are under allowed licences.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
