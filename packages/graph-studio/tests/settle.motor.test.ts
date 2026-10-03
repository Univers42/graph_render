// The size rule over the real motor: what it runs, and the engine its live session ticks on.
// The two runs are each other's negative control: the same request on a graph one size class
// down must run the frozen layout on Barnes-Hut, or the rule has stopped reading size.
import assert from "node:assert/strict";
import { test } from "node:test";

import { createMotor, type Handle } from "../../../crates/graph-sdk-js/src/index.ts";
import { type Session, createSession } from "../src/motor/session.ts";
import { LIVE_NODES, PARTICLE_MESH, SCATTER } from "../src/motor/settle.ts";
import { FIXTURES_URL, SKIP, WASM } from "./motor.ts";

const FORCE = "layout.forceatlas2.barnes_hut";

interface Seen {
  readonly layouts: string[];
  readonly engines: (string | undefined)[];
}

/** The real session over the real motor, recording what it runs and how each session ticks. */
function spiedSession(seen: Seen): Session {
  return createSession({
    motorFrom: async () => {
      const motor = await createMotor(WASM ?? new Uint8Array(0));
      const run = motor.run.bind(motor);
      const forceSession = motor.forceSession.bind(motor);
      motor.run = (handle: Handle, id: string) => (seen.layouts.push(id), run(handle, id));
      motor.forceSession = (handle, params, engine) => (seen.engines.push(engine), forceSession(handle, params, engine));
      return motor;
    },
    fetchText: () => Promise.reject(new Error("this test fetches nothing")),
    digest: () => Promise.resolve(null),
    now: () => 0,
  });
}

async function forceRun(nodes: number): Promise<{ readonly session: Session; readonly seen: Seen; readonly layoutId: string }> {
  const seen: Seen = { layouts: [], engines: [] };
  const session = spiedSession(seen);
  await session.open("unused");
  await session.load({ kind: "synthetic", seed: 1, nodes, degree: 2, shape: "vault" }, FIXTURES_URL);
  const run = await session.layout(FORCE, null);
  return { session, seen, layoutId: run.layoutId };
}

test("a force run past LIVE_NODES scatters, reports the mesh, and settles on it", { skip: SKIP }, async () => {
  const { session, seen, layoutId } = await forceRun(LIVE_NODES);
  assert.deepEqual([layoutId, seen.layouts], [PARTICLE_MESH, [SCATTER]]);
  const port = session.forces();
  assert.ok(port !== null);
  assert.deepEqual(seen.engines, ["particle_mesh"]);
  port.step(1);
  const { xs, ys } = port.positions();
  assert.equal(xs.length, LIVE_NODES);
  assert.ok(xs.every(Number.isFinite) && ys.every(Number.isFinite));
});

test("the same run under LIVE_NODES is the frozen layout, settled on Barnes-Hut", { skip: SKIP }, async () => {
  const { session, seen, layoutId } = await forceRun(60);
  assert.deepEqual([layoutId, seen.layouts], [FORCE, [FORCE]]);
  assert.ok(session.forces() !== null);
  assert.deepEqual(seen.engines, ["barnes_hut"]);
});
