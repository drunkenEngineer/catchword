// @vitest-environment node
// Colour contrast, checked in the stylesheet itself (A11Y-3, WCAG 2.2 AA):
// text at least 4.5 to 1 against what it sits on, and the edges and focus
// rings that show where a control is at least 3 to 1. Windows contrast
// themes replace every colour, so they need no check here.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The stylesheet's text, as shipped.
const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

/** The colour variables set in the first rule for `selector`. */
function colours(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const block = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6});/gi)].map((m) => [m[1], m[2]]));
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5]
    .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

const light = colours(":root");
const dark = { ...light, ...colours(':root[data-theme="dark"]') };

/** Text colours, and what each is shown on. */
const TEXT: [string, string][] = [
  ["text", "bg"],
  ["text", "panel"],
  ["text", "selected"],
  ["text", "mark"],
  ["muted", "bg"],
  ["muted", "panel"],
  ["muted", "selected"],
  ["accent", "bg"],
  ["accent", "panel"],
  ["accent-text", "accent"],
  ["danger", "bg"],
  ["danger", "panel"],
];

/** Edges and rings that show where a control is (WCAG 1.4.11). */
const EDGES: [string, string][] = [
  ["control-border", "bg"],
  ["control-border", "panel"],
  ["accent", "bg"],
  ["accent", "panel"],
  ["accent", "selected"],
];

describe("colour contrast", () => {
  for (const [name, theme] of [
    ["light", light],
    ["dark", dark],
  ] as const) {
    it(`gives text at least 4.5 to 1 in the ${name} theme`, () => {
      for (const [front, back] of TEXT) {
        expect(ratio(theme[front], theme[back]), `${front} on ${back}`).toBeGreaterThanOrEqual(4.5);
      }
    });

    it(`gives control edges and focus rings at least 3 to 1 in the ${name} theme`, () => {
      for (const [front, back] of EDGES) {
        expect(ratio(theme[front], theme[back]), `${front} on ${back}`).toBeGreaterThanOrEqual(3);
      }
    });
  }

  it("uses the same dark colours whether chosen or as Windows is set", () => {
    // The stylesheet has to write them twice; they must not drift apart.
    expect(colours(':root:not([data-theme="light"])')).toEqual(colours(':root[data-theme="dark"]'));
  });

  it("draws the edges of fields to type or choose in with the stronger colour", () => {
    for (const selector of [".search-box", ".filters select", ".limits input", ".patterns"]) {
      const start = css.indexOf(`\n${selector} {`);
      const block = css.slice(start, css.indexOf("}", start));
      expect(block, selector).toContain("var(--control-border)");
    }
  });
});
