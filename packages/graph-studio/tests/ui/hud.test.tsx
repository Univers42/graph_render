// The HUD: one line of numbers, and the part of it that must never go through React.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import { Hud } from "../../src/ui/Hud.tsx";
import { DRAWN, IDLE, STATS, fakeView, markup } from "./desk.ts";

function hud(state = DRAWN, view: Pick<View, "stats" | "on"> = fakeView()): string {
  return markup(createElement(Hud, { state, view }));
}

function viewAt(fps: number): Pick<View, "stats" | "on"> {
  return { ...fakeView(), stats: () => ({ ...STATS, fps }) };
}

test("a rate is a whole number, and a parked view has none", () => {
  assert.match(hud(DRAWN, viewAt(59.62)), /· 60 fps ·/);
  const parked = hud(DRAWN, viewAt(0));
  assert.match(parked, /· idle ·/);
  assert.ok(!parked.includes("fps"), "no rate is printed for a view that does not move");
});

test("what the view drew is on the line, and the backend is named", () => {
  const html = hud();
  assert.match(html, /3 n/);
  assert.match(html, /2 e/);
  assert.match(html, /60 fps/);
  assert.match(html, /canvas2d/);
});

test("the last run is on the line too, with eight characters of its digest", () => {
  const html = hud();
  assert.match(html, /layout 13 ms/);
  assert.match(html, /01234567/);
});

test("with nothing drawn the line stops at the view", () => {
  const html = hud(IDLE);
  assert.ok(!html.includes("layout"), "no run to report");
  assert.ok(html.includes("3 n"), "and the view is still measured");
});

test("the numbers from the view are written into one element, not into the tree", () => {
  const html = hud();
  assert.match(html, /class="gs-hud-frame"/, "the part the frame handler writes into is there");
});
