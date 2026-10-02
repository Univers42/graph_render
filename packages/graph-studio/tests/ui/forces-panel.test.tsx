/**
 * The forces panel: the ids the dock and the stylesheet reach, and the one line that now
 * draws the live loop's store instead of throwing it away.
 *
 * `renderToStaticMarkup` runs no effect, so the frame bound is not observable here; it is
 * what frame-throttle.test.ts holds, against a scheduler the test drives itself. What this
 * file keeps is the markup contract, and a negative control: a panel that reads the store
 * and drops it renders exactly the panel there was before.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { LiveBridge } from "../../src/motor/bridge.ts";
import { type Bar, HIDDEN, batchBar } from "../../src/ui/progress.ts";
import { ForcesPanel, drawnOf } from "../../src/ui/ForcesPanel.tsx";
import { DRAWN, IDLE, fakeBar, markup, studioWith } from "./desk.ts";

type BarStore = Pick<LiveBridge, "bar" | "onBar">;

/** The bar's store standing still at one value, so the panel has a line to draw. */
function storeAt(shown: Bar): BarStore {
  return { bar: () => shown, onBar: () => () => undefined };
}

function panel(bar: BarStore = fakeBar(), drawn = DRAWN): string {
  const { studio } = studioWith(drawn);
  return markup(createElement(ForcesPanel, { studio, state: drawn, bar }));
}

test("the panel keeps the ids the dock and the stylesheet reach", () => {
  const html = panel();
  assert.match(html, /id="gs-forces"/);
  assert.match(html, /class="gs-reason" id="gs-forces-reason"/);
  assert.equal(html.match(/type="range"[^>]*aria-disabled="true"/g)?.length, 4);
  for (const title of ["Center force", "Repel force", "Link force", "Link distance", "Reset", "Animate", "Pause", "Resume"]) {
    assert.ok(html.includes(title), title);
  }
});

test("nothing settling leaves the panel as it was: no line, and the reason once", () => {
  const html = panel();
  assert.doesNotMatch(html, /gs-busy/, "a hidden bar draws nothing at all");
  assert.equal(html.split("live forces need the motor session (force-wasm)").length - 1, 1);
});

test("a settling loop draws its own line, with how much of it is left", () => {
  const bar = storeAt({ visible: true, fraction: 0.5, label: "settling" });
  assert.match(panel(bar), /<div class="gs-busy"><span class="gs-busy-name">settling 50%<\/span><\/div>/);
});

test("a batch has no fraction of its own, so the line says what is running and stops", () => {
  const html = panel(storeAt(batchBar(3)));
  assert.match(html, /<span class="gs-busy-name">3 running<\/span>/);
  assert.doesNotMatch(html, /%/);
});

test("a line that changes sixty times a second is not a live region", () => {
  const bar = storeAt({ visible: true, fraction: 0.5, label: "settling" });
  const html = panel(bar);
  const start = html.indexOf('<div class="gs-busy">');
  const line = html.slice(start, html.indexOf("</div>", start) + "</div>".length);
  assert.notEqual(start, -1, "the line is there to be looked at");
  assert.doesNotMatch(line, /role=|aria-live/, "the reason below it is the region a reader hears");
});

test("with nothing drawn the sliders stay refused, and the loop's line is still there", () => {
  const bar = storeAt({ visible: true, fraction: 1, label: "settling" });
  const html = panel(bar, IDLE);
  assert.equal(html.match(/type="range"[^>]*aria-disabled="true"/g)?.length, 4, "no graph, no live knobs");
  assert.equal(html.split("live forces need the motor session (force-wasm)").length - 1, 1);
  assert.match(html, /settling 100%/, "the line reads the store, whatever the state says");
});

test("the panel's own snapshot of the bridge does not move when a frame does", () => {
  const { studio } = studioWith(DRAWN);
  const forces = studio.registry.actions.filter((action) => action.section === "Forces");
  const snapshot = drawnOf(forces, DRAWN);
  assert.equal(snapshot, drawnOf(forces, DRAWN), "a frame changes the fraction, not this");
  assert.match(snapshot, /force-wasm/, "the reason the worker has not answered yet is in it");
  // The rig's link never answers, so only the stable half is observable here; what moves it
  // in a studio is `available()` and the knob values, which is why both are in the string.
});

test("the panel reads the store once per render, not once per control", () => {
  let reads = 0;
  const bar: BarStore = {
    bar: () => {
      reads += 1;
      return HIDDEN;
    },
    onBar: () => () => undefined,
  };
  panel(bar);
  assert.equal(reads, 1, "one reader, so one bar a frame rather than one per slider");
});