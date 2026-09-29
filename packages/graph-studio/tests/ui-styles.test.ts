// The stylesheet the shadow root carries: what may not be in it, and what must be.
import assert from "node:assert/strict";
import { test } from "node:test";

import { STUDIO_CSS } from "../src/styles/studio.css.ts";

const PLAIN = STUDIO_CSS.replace(/\/\*[\s\S]*?\*\//g, "");

/** Everything written before a `{`: a selector, or the prelude of a nested rule. */
const SELECTORS: readonly string[] = [...PLAIN.matchAll(/([^{}]+)\{/g)].map((found) => found[1] ?? "");

const CLASS_NAME = /\.([A-Za-z_][\w-]*)/g;

test("nothing in the stylesheet makes the browser recomposite the drawing", () => {
  assert.ok(!PLAIN.includes("backdrop-filter"), "a backdrop-filter blurs what is behind the panel");
  assert.ok(!PLAIN.includes("filter"), "a filter is one too");
  assert.ok(!PLAIN.includes("@keyframes"), "an animation that runs on its own is never what a panel needs");
  assert.ok(!PLAIN.includes("animation"), "the same for the shorthand");
});

test("every class selector starts with gs-, so the chrome cannot reach the host's", () => {
  const names = SELECTORS.flatMap((selector) => [...selector.matchAll(CLASS_NAME)].map((found) => found[1] ?? ""));
  assert.ok(names.length > 20, `only ${names.length} class selectors in the stylesheet`);
  assert.deepEqual(names.filter((name) => !name.startsWith("gs-")), []);
});

test("the two themes are the custom properties the panels read", () => {
  assert.match(STUDIO_CSS, /\.gs-chrome\s*\{[^}]*--gs-panel:\s*#1e1e22/);
  assert.match(STUDIO_CSS, /\.gs-chrome\[data-theme="light"\][^{]*\{[^}]*--gs-panel:\s*#f6f6f4/);
  for (const name of ["--gs-panel-2", "--gs-border", "--gs-text", "--gs-muted", "--gs-accent", "--gs-danger", "--gs-radius"]) {
    assert.ok(STUDIO_CSS.includes(`${name}:`), `${name} is never set`);
  }
  assert.match(STUDIO_CSS, /@media \(prefers-reduced-motion: reduce\)/);
});

test("what is hidden is not displayed, whatever its class says about display", () => {
  assert.match(PLAIN, /\.gs-chrome \[hidden\]\s*\{\s*display:\s*none\s*!important;?\s*\}/);
});

test("a section header is a row with its caret at the far end, and a long list takes two columns", () => {
  assert.match(PLAIN, /\.gs-section-head\s*\{[^}]*display:\s*flex[^}]*justify-content:\s*space-between/);
  assert.match(PLAIN, /\.gs-list\s*\{[^}]*display:\s*grid[^}]*grid-template-columns:\s*1fr 1fr/);
});

test("the chrome lets the canvas through: the root takes no pointer, the panels take it back", () => {
  const root = SELECTORS.join(" ").includes(".gs-chrome");
  assert.ok(root);
  assert.match(STUDIO_CSS, /\.gs-chrome\s*\{[^}]*pointer-events:\s*none/);
  assert.match(STUDIO_CSS, /\.gs-panel\s*\{[^}]*pointer-events:\s*auto/);
});

test("a closed drawer is off the screen, and the toggle that opens it is not", () => {
  // The toggles hang off the root, which takes no pointer: without this they never click.
  assert.match(PLAIN, /\.gs-toggle\s*\{[^}]*position:\s*fixed[^}]*pointer-events:\s*auto/);
  assert.match(PLAIN, /\.gs-toggle\s*\{[^}]*min-width:\s*30px/);
  assert.match(PLAIN, /\.gs-toggle-left\s*\{\s*left:/);
  assert.match(PLAIN, /\.gs-toggle-right\s*\{\s*right:/);
  // Hidden where the panels are not drawers, so the desktop keeps the arrangement it had.
  assert.match(PLAIN, /\.gs-toggle\s*\{\s*display:\s*none/);
  assert.match(PLAIN, /\.gs-left, \.gs-dock\s*\{[^}]*visibility:\s*hidden/);
  assert.match(PLAIN, /\.gs-left\.gs-open, \.gs-dock\.gs-open\s*\{[^}]*visibility:\s*visible[^}]*translateX\(0\)/);
  assert.ok(PLAIN.includes(".gs-btn, .gs-toggle { transition: none; }"), "reduced motion takes the toggles too");
});

test("the open drawer is under the console, so the command line is never behind it", () => {
  assert.match(PLAIN, /\.gs-console\s*\{[^}]*z-index:\s*2/);
  const narrow = PLAIN.slice(PLAIN.indexOf("@media (max-width: 768px)"));
  assert.match(narrow, /\.gs-left, \.gs-dock\s*\{[^}]*z-index:\s*1/);
});

test("the swatch's box is a target the eye still reads as a 10 px chip", () => {
  assert.match(PLAIN, /\.gs-swatch\s*\{[^}]*width:\s*24px[^}]*height:\s*24px[^}]*border:\s*7px solid var\(--gs-panel\)/);
});
