// Tests for check-npm-licences.mjs: node --test scripts/check-npm-licences.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { allowedLicences, licenceProblems, satisfies } from "./check-npm-licences.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ALLOWED = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "ISC"];

/** A package-lock.json with the given packages, keyed by name. */
function lock(packages) {
  const entries = { "": { name: "catchword-ui", license: "Apache-2.0" } };
  for (const [name, entry] of Object.entries(packages)) entries[`node_modules/${name}`] = { version: "1.0.0", ...entry };
  return { lockfileVersion: 3, packages: entries };
}

test("the allowed list is read from deny.toml's [licenses] section only", () => {
  const toml = [
    "[advisories]",
    'ignore = ["NOT-A-LICENCE"]',
    "",
    "[licenses]",
    "allow = [",
    '    "MIT",',
    '    "Apache-2.0 WITH LLVM-exception",',
    "]",
    "confidence-threshold = 0.9",
    "",
    "[bans]",
    'deny = ["ALSO-NOT"]',
  ].join("\r\n");
  assert.deepEqual(allowedLicences(toml), ["MIT", "Apache-2.0 WITH LLVM-exception"]);
  assert.throws(() => allowedLicences("[bans]\nallow = []\n"));
});

test("an expression needs one side of OR and both sides of AND", () => {
  const allowed = new Set(ALLOWED);
  assert.equal(satisfies("MIT", allowed), true);
  assert.equal(satisfies("GPL-3.0-only", allowed), false);
  assert.equal(satisfies("GPL-3.0-only OR MIT", allowed), true);
  assert.equal(satisfies("(MIT OR GPL-2.0)", allowed), true);
  assert.equal(satisfies("MIT AND ISC", allowed), true);
  assert.equal(satisfies("MIT AND AGPL-3.0-only", allowed), false);
  assert.equal(satisfies("(MIT AND AGPL-3.0-only) OR ISC", allowed), true);
  assert.equal(satisfies("MIT AND (AGPL-3.0-only OR ISC)", allowed), true);
  assert.equal(satisfies("Apache-2.0 WITH LLVM-exception", allowed), true);
  assert.equal(satisfies("MIT WITH Some-exception", allowed), false);
  assert.equal(satisfies("mit or GPL-3.0-only", new Set(["mit"])), true);
});

test("an expression that cannot be read is an error, not a pass", () => {
  const allowed = new Set(ALLOWED);
  for (const broken of ["", "MIT OR", "(MIT", "MIT)", "MIT ISC", "OR MIT", "MIT WITH", "AND"]) {
    assert.throws(() => satisfies(broken, allowed), undefined, broken);
  }
});

test("a GPL, AGPL, unknown or missing licence is reported", () => {
  const problems = licenceProblems(
    lock({
      fine: { license: "MIT" },
      either: { license: "Apache-2.0 OR MIT" },
      copyleft: { license: "GPL-3.0-or-later" },
      network: { license: "AGPL-3.0-only", dev: true },
      unknown: { license: "SEE LICENSE IN LICENSE.txt" },
      missing: {},
      legacy: { license: { type: "MIT" } },
    }),
    ALLOWED,
  );
  assert.equal(problems.length, 5, problems.join("\n"));
  for (const name of ["copyleft", "network", "unknown", "missing", "legacy"]) {
    assert.ok(problems.some((problem) => problem.startsWith(`${name} `)), name);
  }
});

test("the two build-and-test licences are allowed only for packages not shipped", () => {
  const problems = licenceProblems(
    lock({
      "lru-cache": { license: "BlueOak-1.0.0", dev: true },
      "@csstools/color-helpers": { license: "MIT-0", dev: true },
      shipped: { license: "BlueOak-1.0.0" },
      "also-shipped": { license: "MIT-0", devOptional: true },
    }),
    ALLOWED,
  );
  assert.deepEqual(
    problems.map((problem) => problem.split(" ")[0]),
    ["shipped", "also-shipped"],
  );
  assert.match(problems[0], /shipped in the app/);
});

test("the interface itself and links to it are not checked", () => {
  const entries = lock({});
  entries.packages[""].license = "UNLICENSED";
  entries.packages["node_modules/catchword-ui"] = { resolved: "..", link: true };
  assert.deepEqual(licenceProblems(entries, ALLOWED), []);
});

test("every package in the real lock file is under an allowed licence", () => {
  const allowed = allowedLicences(readFileSync(join(root, "deny.toml"), "utf8"));
  const real = JSON.parse(readFileSync(join(root, "apps", "desktop", "ui", "package-lock.json"), "utf8"));
  assert.ok(Object.keys(real.packages).length > 100);
  assert.deepEqual(licenceProblems(real, allowed), []);
});
