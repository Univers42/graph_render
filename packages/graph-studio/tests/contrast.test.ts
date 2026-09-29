import { test } from "node:test";
import assert from "node:assert/strict";

/** WCAG 2.x relative luminance. */
function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const r = ((n >> 16) & 0xff) / 255;
  const g = ((n >> 8) & 0xff) / 255;
  const b = (n & 0xff) / 255;
  const f = (c: number): number => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

/** WCAG contrast ratio between two hex colours. */
function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

const DARK = {
  text: "#dadada",
  muted: "#8e8e96",
  accent: "#8b7cf6",
  danger: "#ff3200",
  panel: "#1e1e22",
  panel2: "#26262b",
  border: "rgba(255,255,255,.08)",
};

const LIGHT = {
  text: "#2a2a2e",
  muted: "#6a6a6e",
  accent: "#6c5ce0",
  danger: "#c62d33",
  panel: "#f6f6f4",
  panel2: "#ecece9",
  border: "rgba(0,0,0,.10)",
};

test("dark theme: every text token pair meets 4.5:1", () => {
  const pairs: Array<[string, string, string]> = [
    ["text/panel", DARK.text, DARK.panel],
    ["text/panel2", DARK.text, DARK.panel2],
    ["muted/panel", DARK.muted, DARK.panel],
    ["muted/panel2", DARK.muted, DARK.panel2],
    ["accent/panel", DARK.accent, DARK.panel],
    ["danger/panel", DARK.danger, DARK.panel],
  ];
  for (const [name, fg, bg] of pairs) {
    const ratio = contrast(fg, bg);
    assert.ok(ratio >= 4.5, `dark ${name}: ${ratio.toFixed(2)} < 4.5`);
  }
});

test("light theme: every text token pair meets 4.5:1", () => {
  const pairs: Array<[string, string, string]> = [
    ["text/panel", LIGHT.text, LIGHT.panel],
    ["text/panel2", LIGHT.text, LIGHT.panel2],
    ["muted/panel", LIGHT.muted, LIGHT.panel],
    ["muted/panel2", LIGHT.muted, LIGHT.panel2],
    ["accent/panel", LIGHT.accent, LIGHT.panel],
    ["danger/panel", LIGHT.danger, LIGHT.panel],
  ];
  for (const [name, fg, bg] of pairs) {
    const ratio = contrast(fg, bg);
    assert.ok(ratio >= 4.5, `light ${name}: ${ratio.toFixed(2)} < 4.5`);
  }
});

test("dark theme: UI tokens meet 3:1 against panel", () => {
  const pairs: Array<[string, string]> = [
    ["accent", DARK.accent],
    ["danger", DARK.danger],
    ["border", "#6b6b6d"],
  ];
  for (const [name, fg] of pairs) {
    const ratio = contrast(fg, DARK.panel);
    assert.ok(ratio >= 3, `dark UI ${name}: ${ratio.toFixed(2)} < 3`);
  }
});

test("light theme: UI tokens meet 3:1 against panel", () => {
  const pairs: Array<[string, string]> = [
    ["accent", LIGHT.accent],
    ["danger", LIGHT.danger],
    ["border", "#8c8c8b"],
  ];
  for (const [name, fg] of pairs) {
    const ratio = contrast(fg, LIGHT.panel);
    assert.ok(ratio >= 3, `light UI ${name}: ${ratio.toFixed(2)} < 3`);
  }
});
