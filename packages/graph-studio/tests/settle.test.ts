// Which layout a request runs by graph size, and the engine its live session ticks on.
// The same rule over the real motor is in `settle.motor.test.ts`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { LIVE_NODES, PARTICLE_MESH, SCATTER, planRun } from "../src/motor/settle.ts";

const FORCE = "layout.forceatlas2.barnes_hut";

test("a force layout past LIVE_NODES is scattered and reported as the particle mesh", () => {
  assert.deepEqual(planRun(FORCE, LIVE_NODES, true), { run: SCATTER, report: PARTICLE_MESH, engine: "particle_mesh" });
});

test("under LIVE_NODES the frozen layout runs, and its session ticks Barnes-Hut", () => {
  assert.deepEqual(planRun(FORCE, LIVE_NODES - 1, true), { run: FORCE, report: FORCE, engine: "barnes_hut" });
});

test("a large graph runs a layout that does not settle, and a motor without a session, frozen", () => {
  assert.deepEqual(planRun("layout.grid", LIVE_NODES, true), { run: "layout.grid", report: "layout.grid", engine: "particle_mesh" });
  assert.deepEqual(planRun(FORCE, LIVE_NODES, false), { run: FORCE, report: FORCE, engine: "particle_mesh" });
});

test("the particle mesh named outright settles on the mesh at any size", () => {
  assert.equal(planRun(PARTICLE_MESH, 10, true).engine, "particle_mesh");
});
