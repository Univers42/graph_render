// The strings the chrome prints: an id without its family, a digest that fits, a duration
// that does not lie about being zero.
import assert from "node:assert/strict";
import { test } from "node:test";

import type { ViewStats } from "../../graph-render/src/view.ts";
import { digest8, frameLine, ms, shortName, sig3 } from "../src/ui/names.ts";

const STATS: ViewStats = {
  backend: "canvas2d", backendFailure: "", refining: false, nodes: 3, edges: 2, drawnNodes: 3, drawnEdges: 2, drawnLabels: 2, drawnArrows: 0, arrowSize: 0, curvedEdges: 0, strokeWidth: 0,
  draws: 12, strokeCalls: 1, edgeStyles: 1, mixedEdges: 0, gradientStrokes: 0, arrowFills: 0, glowFills: 0, spritesRasterised: 0, layoutRuns: 1, frameMs: 4.2, fps: 60, frames: 42, gpuEdgeMs: 0,
};

test("the line of a frame: what is drawn, how fast, how long the last frame took, and by what", () => {
  assert.equal(frameLine(STATS, 0), "3 n · 2 e · 60 fps · 4 ms · canvas2d");
  assert.equal(frameLine({ ...STATS, fps: 59.62 }, 12), "3 n · 2 e · 60 fps · 4 ms · canvas2d");
});

test("a parked view has no rate, and the rate of its last move stays next to that", () => {
  assert.equal(frameLine({ ...STATS, fps: 0 }, 0), "3 n · 2 e · idle · 4 ms · canvas2d");
  assert.equal(frameLine({ ...STATS, fps: 0 }, 37.6), "3 n · 2 e · idle, last 38 fps · 4 ms · canvas2d");
});

test("an id loses its first dotted segment, and one without a dot is kept", () => {
  assert.equal(shortName("layout.force.barnes_hut"), "force.barnes_hut");
  assert.equal(shortName("off"), "off");
  assert.equal(shortName("layout.grid"), "grid");
  assert.equal(shortName(""), "");
});

test("a fixture path is shown whole, not cut at its extension", () => {
  assert.equal(shortName("dag/chain.json"), "dag/chain.json");
  assert.equal(shortName("hierarchy/tree-balanced.json"), "hierarchy/tree-balanced.json");
});

test("a digest shows its first eight characters, and nothing at all is a dash", () => {
  assert.equal(digest8("0123456789abcdef"), "01234567");
  assert.equal(digest8("short"), "short");
  assert.equal(digest8(null), "—");
});

test("a duration reads as milliseconds, or as seconds once there are a thousand of them", () => {
  assert.equal(ms(0), "<1 ms");
  assert.equal(ms(0.4), "<1 ms");
  assert.equal(ms(1), "1 ms");
  assert.equal(ms(12.4), "12 ms");
  assert.equal(ms(999), "999 ms");
  assert.equal(ms(1000), "1.0 s");
  assert.equal(ms(1340), "1.3 s");
  assert.equal(ms(61230), "61.2 s");
});

test("a value shows three significant digits and no more", () => {
  assert.equal(sig3(0.5), "0.5");
  assert.equal(sig3(1), "1");
  assert.equal(sig3(1.23456), "1.23");
  assert.equal(sig3(123456), "123000");
  assert.equal(sig3(0.000123456), "0.000123");
});
