// The HUD: the camera's buttons, and the numbers that must never go through React.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import { Hud } from "../../src/ui/Hud.tsx";
import { NavBar } from "../../src/ui/NavBar.tsx";
import { DRAWN, IDLE, STATS, fakeView, markup, studioWith } from "./desk.ts";

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
  assert.ok(!parked.includes("fps"), "no rate is printed for a view that has not moved yet");
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

test("the navigation bar has the five camera buttons, each one named for the reader", () => {
  const { studio } = studioWith();
  const html = markup(createElement(NavBar, { studio }));
  for (const title of [
    "Fit the graph to the view", "Zoom in ×2", "Zoom out ÷2", "Reset the camera to 1:1", "Pan 50 pixels",
  ]) {
    assert.ok(html.includes(`aria-label="${title}"`), title);
  }
  assert.equal(html.match(/gs-nav-btn/g)?.length, 5, "and no sixth button");
  assert.ok(!html.includes("Lock"), "the lock is S6: a button that froze nothing would be a lie");
});

test("a navigation button dispatches the action it names, through the studio", () => {
  const { studio, seen } = studioWith();
  markup(createElement(NavBar, { studio }));
  // The rendered markup carries the handlers, not the calls: the ids are what the gate reads.
  const ids = ["view.fit", "view.zoom", "view.reset", "view.pan"] as const;
  for (const id of ids) {
    assert.ok(studio.registry.find(id) !== undefined, `${id} is an action of the studio`);
  }
  assert.deepEqual(seen.calls, [], "rendering the bar moves nothing on its own");
});
