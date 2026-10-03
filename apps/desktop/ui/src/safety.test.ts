// Document text is untrusted (SEC-1, threat T3). React shows text as text,
// unless code goes around it; this test fails if any source does.
import { describe, expect, it } from "vitest";

const sources = import.meta.glob(["./**/*.ts", "./**/*.tsx", "!./**/*.test.*"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

describe("the interface's own code", () => {
  it("is all checked", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(5);
  });

  it("never writes HTML from strings or runs strings as code", () => {
    const forbidden = /dangerouslySetInnerHTML|innerHTML|outerHTML|insertAdjacentHTML|document\.write|\beval\(|new Function\(/;
    const offenders = Object.entries(sources)
      .filter(([, text]) => forbidden.test(text))
      .map(([file]) => file);
    expect(offenders).toEqual([]);
  });

  it("calls the shell only through the engine client", () => {
    const offenders = Object.entries(sources)
      .filter(([file, text]) => file !== "./engine.ts" && text.includes("@tauri-apps/api"))
      .map(([file]) => file);
    expect(offenders).toEqual([]);
  });
});
