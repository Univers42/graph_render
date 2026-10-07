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

import { type ForceLink, NO_FORCE_LINK } from "../../src/actions/forces.ts";
import type { LiveBridge } from "../../src/motor/bridge.ts";
import { DEFAULT_KNOBS } from "../../src/motor/live.ts";
import { withSettings } from "../../src/state/settings.ts";
import { type Bar, HIDDEN, batchBar } from "../../src/ui/progress.ts";
import { ForcesPanel, drawnOf } from "../../src/ui/ForcesPanel.tsx";
import { DRAWN, IDLE, fakeBar, markup, studioWith } from "./desk.ts";

type BarStore = Pick<LiveBridge, "bar" | "onBar">;

const SLIDERS = [
  "Center force", "Repel force", "Link force", "Link distance", "Node spacing", "Friction", "Cooling", "Repel range", "Accuracy",
];

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
  assert.equal(html.match(/type="range"[^>]*aria-disabled="true"/g)?.length, 9);
  for (const title of [...SLIDERS, "Spread", "Compact", "Reset", "Animate", "Pause", "Resume"]) {
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
  assert.equal(html.match(/type="range"[^>]*aria-disabled="true"/g)?.length, 9, "no graph, no live knobs");
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
/** A live link over the store's own knobs, as the studio wires it: the panel draws what is saved. */
function liveLink(): ForceLink {
  return { ...NO_FORCE_LINK, disabled: () => null };
}

test("each slider's range is the action's own, so the panel is never wider than the motor", () => {
  const { studio } = studioWith(DRAWN, liveLink());
  const html = markup(createElement(ForcesPanel, { studio, state: DRAWN, bar: fakeBar() }));
  const ranges = [...html.matchAll(/type="range" min="([^"]+)" max="([^"]+)"/g)].map((found) => `${found[1] ?? ""}..${found[2] ?? ""}`);
  assert.deepEqual(ranges, ["0..1", "0..1000", "0..2", "10..500", "0..400", "0.01..0.99", "0.005..0.5", "10..5000", "0.3..1.5"]);
  assert.doesNotMatch(html, /aria-disabled="true"/, "a live link leaves every control on");
});

test("a slider shows the knob the state holds: a preset or a recalled source moves the thumb", () => {
  const { studio } = studioWith(DRAWN, liveLink());
  const state = { ...DRAWN, settings: withSettings(DRAWN.settings, { forces: { ...DEFAULT_KNOBS, collideRadius: 7.5 } }) };
  studio.store.set(state);
  const html = markup(createElement(ForcesPanel, { studio, state, bar: fakeBar() }));
  assert.match(html, /<span class="gs-field-label">Node spacing<\/span><span class="gs-row"><input[^>]*value="7.5"/);
});

test("the_gpu_switch_is_drawn_off_by_default", () => {
  const html = panel();
  assert.match(html, /<input type="checkbox" role="switch"[^>]*aria-disabled="true"[^>]*\/><span class="gs-field-label">GPU forces<\/span>/);
  assert.doesNotMatch(html, /role="switch"[^>]*checked/, "off until the user turns it on");
});

test("the_theta_knob_says_it_does_nothing_on_the_gpu_arm", () => {
  const link: ForceLink = { ...NO_FORCE_LINK, disabled: () => null, gpu: () => true, setGpu: () => undefined };
  const { studio } = studioWith(DRAWN, link);
  const html = markup(createElement(ForcesPanel, { studio, state: DRAWN, bar: fakeBar() }));
  const off = html.match(/type="range"[^>]*aria-disabled="true"[^>]*/g) ?? [];
  assert.equal(off.length, 1, "Accuracy alone is off; the other eight still move the mesh");
  assert.match(off.join(""), /aria-describedby="gs-forces-reason-accuracy"/);
  assert.match(html, /<span class="gs-reason" id="gs-forces-reason-accuracy">Accuracy is Barnes-Hut&#x27;s theta, and the GPU arm ticks the particle mesh, which has none<\/span>/);
  assert.match(html, /role="switch"[^>]*checked=""/, "the switch shows the arm is on");
});
