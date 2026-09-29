/**
 * A label that sits on its node, the way the SciGraphs overlay draws it, and the one
 * difference that placement makes: the studio's coarse screen grid never gets to drop a
 * label, because the declutter that does that job has already run.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { IDENTITY } from "../src/camera.ts";
import { DEFAULT_POLICY, type LabelInput, newLabelPlan, occupancyFor, planLabels } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import type { LabelPlacement } from "../src/style.ts";

const VIEWPORT = { width: 800, height: 600 };
const HEIGHT = 38;

function input(x: readonly number[], patch: Partial<LabelInput> = {}): LabelInput {
  const style = styleFrom({
    labels: x.map((_, i) => `label ${i}`),
    weights: Float32Array.from(x, () => 0.5),
    colours: new Uint16Array(x.length),
    palette: ["#888888"],
    placement: "centred",
  });
  return {
    style, x: Float32Array.from(x), y: new Float32Array(x.length).fill(100), extent: style.radius,
    camera: { ...IDENTITY, scale: 2 }, viewport: VIEWPORT, lit: null, policy: DEFAULT_POLICY,
    widthOf: () => 0, height: HEIGHT, ...patch,
  };
}

function planned(given: LabelInput): number[] {
  const plan = newLabelPlan(given.policy.budget);
  planLabels(given, plan, occupancyFor(given.viewport));
  return [...plan.node.subarray(0, plan.count)];
}

/** The style the given placement builds, so `below` and `centred` differ in one field. */
function withPlacement(given: LabelInput, placement: LabelPlacement): LabelInput {
  return { ...given, style: { ...given.style, placement } };
}

test("a centred label's box top is half the sprite height above its node", () => {
  const given = input([100]);
  const plan = newLabelPlan(given.policy.budget);
  planLabels(given, plan, occupancyFor(VIEWPORT));
  assert.equal(plan.count, 1);
  // x = 100*2 = 200, y = 100*2 = 200, and the box straddles the node: 200 - 38/2.
  assert.equal(plan.x[0], 200);
  assert.equal(plan.y[0], 181);
  assert.equal(plan.alpha[0], 1);
});

test("a label below its node keeps the studio's placement and grid", () => {
  const given = withPlacement(input([100]), "below");
  const plan = newLabelPlan(given.policy.budget);
  planLabels(given, plan, occupancyFor(VIEWPORT));
  // 200 + extent*scale + LABEL_GAP, the studio's own offset.
  assert.equal(plan.y[0], 200 + (given.extent[0] ?? 0) * 2 + 4);
});

test("two labels the coarse grid would collide on are both drawn when centred", () => {
  // 100 and 120 scale to 200 and 240: the 32 px occupancy cells of the labels overlap.
  assert.deepEqual(planned(input([100, 120])), [0, 1]);
  assert.deepEqual(planned(withPlacement(input([100, 120]), "below")), [0]);
});

test("the budget still bounds a centred frame, and off-screen nodes still draw nothing", () => {
  const x = Array.from({ length: 40 }, (_, i) => (i % 8) * 50);
  const given = input(x, {
    y: Float32Array.from(x, (_, i) => Math.floor(i / 8) * 20),
    policy: { threshold: 1.1, budget: 5 },
  });
  assert.equal(planned(given).length, 5);
  assert.deepEqual(planned(input([5000, 100])), [1]);
});

test("a centred label is opaque at any zoom: the source fades nothing in", () => {
  // The studio's own threshold would cull every label at a scale of 0.1.
  const far = withPlacement(input([100], { camera: { ...IDENTITY, scale: 0.1 } }), "centred");
  const plan = newLabelPlan(far.policy.budget);
  planLabels(far, plan, occupancyFor(VIEWPORT));
  assert.equal(plan.count, 1);
  assert.equal(plan.alpha[0], 1);
  assert.deepEqual(planned({ ...far, style: { ...far.style, placement: "below" } }), []);
});

test("a node with no text draws no label in either placement", () => {
  const bare = input([100]);
  const style = { ...bare.style, labels: [] };
  assert.deepEqual(planned({ ...bare, style }), []);
  assert.deepEqual(planned({ ...withPlacement(bare, "below"), style }), []);
});
