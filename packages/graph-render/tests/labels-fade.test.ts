import assert from "node:assert/strict";
import { test } from "node:test";

import { IDENTITY } from "../src/camera.ts";
import { fadeFactor } from "../src/labels2d/fade.ts";
import { DEFAULT_POLICY, LABEL_HEIGHT, type LabelInput, newLabelPlan, occupancyFor, planLabels } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";

const WEIGHTS = [0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1];

function shown(fade: number, scale: number, placement: "below" | "centred" = "below"): number[] {
  const style = styleFrom({
    labels: WEIGHTS.map((_, i) => `n${i}`), weights: Float32Array.from(WEIGHTS),
    colours: new Uint16Array(WEIGHTS.length), palette: ["a"], placement,
  });
  const given: LabelInput = {
    style, x: Float32Array.from(WEIGHTS, (_, i) => 5 + i * 12), y: new Float32Array(WEIGHTS.length).fill(100),
    extent: style.radius, camera: { ...IDENTITY, scale }, viewport: { width: 800, height: 600 }, lit: null,
    policy: { ...DEFAULT_POLICY, fade }, widthOf: () => 10, height: LABEL_HEIGHT,
  };
  const plan = newLabelPlan(given.policy.budget);
  planLabels(given, plan, occupancyFor(given.viewport));
  return [...plan.node.subarray(0, plan.count)];
}

test("fade 0 is the unfaded threshold", () => {
  assert.equal(fadeFactor(0), 1);
  assert.ok(shown(0, 1.5).length > 0);
});

test("the count only falls as the slider rises, and only rises as the zoom rises", () => {
  for (const placement of ["below", "centred"] as const) {
    for (const scale of [0.4, 1, 1.5, 3]) {
      const counts = [-3, -1.5, 0, 1.5, 3].map((fade) => shown(fade, scale, placement).length);
      assert.deepEqual([...counts].sort((a, b) => b - a), counts, `${placement} at ${scale}: ${counts.join(",")}`);
    }
    for (const fade of [-3, 0, 3]) {
      const counts = [0.4, 1, 1.5, 3].map((scale) => shown(fade, scale, placement).length);
      assert.deepEqual([...counts].sort((a, b) => a - b), counts, `${placement} fade ${fade}: ${counts.join(",")}`);
    }
  }
});

test("the ends of the slider differ, in both placements", () => {
  assert.ok(shown(-3, 1, "below").length > shown(3, 1, "below").length);
  assert.ok(shown(-3, 1, "centred").length > shown(3, 1, "centred").length);
});

test("a faded centred set is a subset of the unfaded one", () => {
  const all = new Set(shown(0, 1, "centred"));
  for (const fade of [1, 2, 3]) for (const node of shown(fade, 1, "centred")) assert.ok(all.has(node));
});
