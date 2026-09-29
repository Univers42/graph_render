import assert from "node:assert/strict";
import { test } from "node:test";

import { movedScene } from "../src/drag.ts";
import { gridOf, pickNode } from "../src/grid.ts";
import { sceneOf } from "../src/scene.ts";
import { plainStyle, styleFrom } from "../src/style.ts";
import { mulberry32, randomFrame } from "./support.ts";

/** Candidates examined per query may not grow with the node count: this is the bound. */
const CANDIDATE_BOUND = 64;

test("a query at 10 000 nodes examines a bounded number of candidates", () => {
  const frame = randomFrame(10000, 0, 21);
  const grid = gridOf(frame, frame.bounds);
  const next = mulberry32(22);
  let worst = 0;
  for (let i = 0; i < 2000; i += 1) {
    let examined = 0;
    pickNode(grid, {
      x: next() * 1000, y: next() * 1000, tolerance: 4, reach: 3,
      distance: () => { examined += 1; return Infinity; },
    });
    worst = Math.max(worst, examined);
  }
  assert.ok(worst > 0, "the probe reached at least one candidate");
  assert.ok(worst <= CANDIDATE_BOUND, `worst ${worst} examined of 10000`);
});

test("the grid is rebuilt when positions change and not when only the style does", () => {
  const frame = randomFrame(100, 0, 3);
  const scene = sceneOf(frame, plainStyle(100), null);
  const restyled = sceneOf(frame, styleFrom({
    labels: [], weights: new Float32Array(100), colours: new Uint16Array(100), palette: ["a"],
    hidden: new Uint8Array(100),
  }), scene);
  assert.equal(restyled.grid, scene.grid);
  const moved = movedScene(scene, { x: frame.x.map((v) => v + 1), y: frame.y });
  assert.notEqual(moved.grid, scene.grid);
  const other = sceneOf({ ...frame, x: frame.x.map((v) => v + 1) }, plainStyle(100), scene);
  assert.notEqual(other.grid, scene.grid);
});
