import assert from "node:assert/strict";
import { test } from "node:test";

import { IDENTITY } from "../src/camera.ts";
import { DEFAULT_POLICY, LABEL_HEIGHT, type LabelInput, newLabelPlan, occupancyFor, planLabels, zoomAlpha } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";

const VIEWPORT = { width: 800, height: 600 };

function input(weights: readonly number[], x: readonly number[], patch: Partial<LabelInput> = {}): LabelInput {
  const style = styleFrom({
    labels: weights.map((_, i) => `label ${i}`), weights: Float32Array.from(weights),
    colours: new Uint16Array(weights.length), palette: ["a"],
  });
  return {
    style, x: Float32Array.from(x), y: new Float32Array(x.length).fill(100), extent: style.radius,
    camera: { ...IDENTITY, scale: 2 }, viewport: VIEWPORT, lit: null, policy: DEFAULT_POLICY, widthOf: () => 0,
    height: LABEL_HEIGHT,
    ...patch,
  };
}

function planned(given: LabelInput): number[] {
  const plan = newLabelPlan(given.policy.budget);
  planLabels(given, plan, occupancyFor(given.viewport));
  return [...plan.node.subarray(0, plan.count)];
}

test("a label's opacity never drops as the zoom grows, and heavier shows first", () => {
  let previous = 0;
  for (let scale = 0.05; scale < 3; scale += 0.05) {
    const alpha = zoomAlpha(scale, 0.3, 1.1);
    assert.ok(alpha >= previous);
    assert.ok(zoomAlpha(scale, 0.9, 1.1) >= alpha);
    previous = alpha;
  }
  assert.equal(zoomAlpha(0.2, 0, 1.1), 0);
  assert.equal(zoomAlpha(3, 0, 1.1), 1);
});

test("two labels on one spot: the heavier node keeps its label", () => {
  assert.deepEqual(planned(input([0.1, 0.9], [100, 104])), [1]);
});

test("labels apart are both drawn, heaviest first", () => {
  assert.deepEqual(planned(input([0.1, 0.9], [50, 300])), [1, 0]);
});

test("far out, only the heavy nodes are labelled", () => {
  assert.deepEqual(planned(input([0, 1], [50, 300], { camera: { ...IDENTITY, scale: 0.5 } })), [1]);
  assert.deepEqual(planned(input([0, 1], [50, 300], { camera: { ...IDENTITY, scale: 0.1 } })), []);
});

test("the budget bounds a frame", () => {
  const x = Array.from({ length: 40 }, (_, i) => (i % 8) * 50);
  const given = input(x.map(() => 1), x, {
    y: Float32Array.from(x, (_, i) => Math.floor(i / 8) * 20), policy: { threshold: 1.1, budget: 5 },
    camera: { ...IDENTITY, scale: 2 },
  });
  assert.equal(planned(given).length, 5);
});

test("a lit neighbourhood is labelled at any zoom, and nothing else is", () => {
  const lit = Uint8Array.from([1, 0, 1]);
  const given = input([0, 1, 0], [50, 300, 600], { lit, camera: { ...IDENTITY, scale: 0.4 } });
  assert.deepEqual(planned(given), [0, 2]);
});

test("a lit neighbour is labelled even where its text touches another lit label", () => {
  const lit = Uint8Array.from([1, 1]);
  assert.deepEqual(planned(input([1, 1], [100, 102], { lit })), [0, 1]);
  assert.deepEqual(planned(input([1, 1], [100, 102])), [0]);
});

test("a node off screen or without a label draws none", () => {
  assert.deepEqual(planned(input([1, 1], [5000, 100])), [1]);
  const bare = input([1], [100]);
  assert.deepEqual(planned({ ...bare, style: { ...bare.style, labels: [] } }), []);
});
