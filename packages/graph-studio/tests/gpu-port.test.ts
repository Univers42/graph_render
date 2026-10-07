/**
 * The GPU arm's port (`gpuPort.ts`): the mesh is opened by the first tick that runs, a frame
 * never queues a batch behind the one in flight, a session with no GPU arm ticks on the CPU
 * and says why, and a grow on the mesh is awaited before the delta queue reads the node count.
 * The session and the mesh are fakes: the device half is the SDK's browser probe.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { setTimeout as sleep } from "node:timers/promises";

import { createDeltaQueue } from "../src/motor/deltas.ts";
import { type MeshPort, NO_GPU_ARM, gpuPort } from "../src/motor/gpuPort.ts";
import type { ForceParams } from "../src/motor/live.ts";
import type { GraphBatch, Result } from "../src/motor/protocol.ts";

interface Gate { open: () => void; readonly wait: Promise<void> }

function gate(): Gate {
  let open = (): void => undefined;
  const wait = new Promise<void>((resolve) => { open = resolve; });
  return { open, wait };
}

/** A mesh whose batches finish when the test says so; every call is logged. */
function fakeMesh(log: string[]): MeshPort<number> & { batches: Gate[]; nodes: number } {
  const batches: Gate[] = [];
  const mesh = {
    tier: "gpu", reason: "", marks: "fake/arch", alpha: 1, nodes: 3, batches,
    tick: async (ticks: number) => {
      log.push(`mesh.tick ${ticks}`);
      const batch = gate();
      mesh.batches.push(batch);
      await batch.wait;
      mesh.alpha *= 0.5;
    },
    pin: (row: number) => { log.push(`mesh.pin ${row}`); },
    unpin: (row: number) => { log.push(`mesh.unpin ${row}`); },
    reheat: (alpha: number) => { mesh.alpha = alpha; },
    setParams: () => { log.push("mesh.params"); },
    grow: async (handle: number) => {
      log.push(`mesh.grow ${handle}`);
      await sleep(1);
      mesh.nodes += 2;
    },
  };
  return mesh;
}

const PARAMS: ForceParams = {
  charge: -90, theta: 0.9, distance_min: 1, distance_max: 520, link_distance: 60, link_strength_scale: 0.15,
  collide_radius: 16, center_strength: 1, gravity: 0, velocity_decay: 0.58, alpha_decay: 0.06, alpha_min: 0.001,
  initial_alpha: 1,
};

/** A particle-mesh session; `armed` false is a motor whose SDK has no `gpuMesh`. */
function fakeSession(log: string[], mesh: MeshPort<number> | null) {
  const session = {
    alpha: 1,
    tick: (ticks: number) => { log.push(`cpu.tick ${ticks}`); session.alpha *= 0.9; return { alpha: session.alpha }; },
    pin: (row: number) => { log.push(`cpu.pin ${row}`); },
    unpin: () => undefined,
    reheat: (alpha: number) => { session.alpha = alpha; },
    setParams: () => undefined,
    positions: () => ({ xs: new Float64Array(3), ys: new Float64Array(3) }),
    params: () => PARAMS,
    release: () => { log.push("release"); },
    grow: (handle: number) => { log.push(`cpu.grow ${handle}`); },
  };
  return mesh === null ? session : { ...session, gpuMesh: async () => { log.push("open"); await sleep(1); return mesh; } };
}

test("the_first_tick_opens_the_mesh_and_the_ticks_after_it_run_on_it", async () => {
  const log: string[] = [];
  const mesh = fakeMesh(log);
  const port = gpuPort(fakeSession(log, mesh));
  assert.equal(port.tick(0).alpha, 1, "a tick of nothing opens no device");
  assert.deepEqual(log, []);
  assert.equal(port.tick(1).alpha, 1, "until the mesh answers, a tick runs nothing");
  assert.deepEqual(port.tier(), { tier: "opening", reason: "", marks: "" });
  await sleep(5);
  port.pin(2, 0, 0);
  port.tick(1);
  assert.deepEqual(log, ["open", "mesh.pin 2", "mesh.tick 1"], "no CPU tick ran, and the pin went to the mesh");
  assert.equal(port.tier().tier, "gpu");
});

test("a_frame_while_a_batch_is_in_flight_starts_none", async () => {
  const log: string[] = [];
  const mesh = fakeMesh(log);
  const port = gpuPort(fakeSession(log, mesh));
  port.tick(1);
  await sleep(5);
  port.tick(1);
  port.tick(1);
  port.tick(1);
  assert.equal(mesh.batches.length, 1, "three frames over one batch in flight start nothing");
  mesh.batches[0]?.open();
  await sleep(1);
  assert.equal(port.tick(1).alpha, 0.5, "the next frame reads the alpha the batch left");
  assert.equal(mesh.batches.length, 2);
});

test("a_session_without_a_gpu_arm_ticks_on_the_cpu_and_says_why", () => {
  const log: string[] = [];
  const port = gpuPort(fakeSession(log, null));
  port.tick(1);
  port.tick(1);
  assert.deepEqual(log, ["cpu.tick 1", "cpu.tick 1"]);
  assert.deepEqual(port.tier(), { tier: "cpu-no-adapter", reason: NO_GPU_ARM, marks: "" });
});

test("a_grow_on_the_mesh_is_awaited_before_the_node_count_is_answered", async () => {
  const log: string[] = [];
  const mesh = fakeMesh(log);
  const port = gpuPort(fakeSession(log, mesh));
  port.tick(1);
  await sleep(5);
  const emitted: Result[] = [];
  const queue = createDeltaQueue({
    extend: () => undefined,
    grow: () => { port.grow?.(7); },
    settled: () => port.settled() ?? null,
    reheat: () => undefined,
    alpha: () => 0.5,
    nodeCount: () => mesh.nodes,
    emit: (result) => emitted.push(result),
    now: () => 0,
  });
  const batch: GraphBatch = { nodes: [], edges: [] };
  const answer = queue.push(batch);
  await queue.drain(1);
  const result = await answer;
  assert.ok(log.includes("mesh.grow 7") && !log.includes("cpu.grow 7"), "the mesh grew the session");
  assert.equal(result.type === "deltas-applied" ? result.nodeCount : -1, 5, "the count is read after the grow");
  assert.equal(port.settled(), null, "nothing is left on its way");
});
