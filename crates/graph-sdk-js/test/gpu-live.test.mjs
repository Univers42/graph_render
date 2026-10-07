// The GPU mesh's orchestration, node-only: a fake session and a fake driver stand in for the
// motor and the device, so what is pinned here is the order, the fallbacks and the rebuilds —
// `docs/decisions/gpu-g1d.md` conditions 7 and 11. The device half is the browser probe's.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-live.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";
import { setTimeout as sleep } from "node:timers/promises";

import { attachGpuMesh } from "../src/gpu/live-attach.ts";
import { deviceOpener } from "../src/gpu/live-driver.ts";
import { liveFixture, sideFor, twiddlesFor } from "../src/gpu/live-fixture.ts";
import { edgeTables, linkCsr } from "../src/gpu/link.ts";
import { gridFor } from "../src/gpu/collide-grid.ts";
import { FROZEN_LAW } from "../src/gpu/law.ts";
import { Refusal } from "../src/gpu/adapter.ts";

/** A session over `n` nodes on a line, its edges a path; `tick` moves every x by 1 per tick. */
function fakeSession(n) {
  const state = {
    xs: Float64Array.from({ length: n }, (_, i) => i * 10),
    ys: new Float64Array(n),
    vxs: new Float64Array(n).fill(0.5),
    vys: new Float64Array(n).fill(-0.5),
    alpha: 1,
    params: { ...FROZEN_LAW },
    cpuTicks: 0,
    detached: false,
  };
  const path = (count) => ({
    lo: Uint32Array.from({ length: count - 1 }, (_, i) => i),
    hi: Uint32Array.from({ length: count - 1 }, (_, i) => i + 1),
    strength: new Float64Array(count - 1).fill(1),
  });
  return {
    state,
    positions: () => ({ xs: state.xs, ys: state.ys }),
    velocities: () => ({ vxs: state.vxs, vys: state.vys }),
    edges: () => path(state.xs.length),
    params: () => ({ ...state.params }),
    alpha: () => state.alpha,
    reheat: (alpha) => { state.alpha = alpha; },
    pin: () => {},
    unpin: () => {},
    unpinAll: () => {},
    setParams: (params) => { Object.assign(state.params, params); },
    grow: () => {
      const grow = (from, fill) => Float64Array.from([...from, fill]);
      state.xs = grow(state.xs, 1000);
      state.ys = grow(state.ys, 1000);
      state.vxs = grow(state.vxs, 0);
      state.vys = grow(state.vys, 0);
    },
    tick: (ticks) => {
      state.cpuTicks += ticks;
      state.xs.forEach((x, i) => { state.xs[i] = x + ticks; });
      state.alpha *= 0.94 ** ticks;
      return { status: "running", alpha: state.alpha, ticksRun: ticks };
    },
    detach: () => { state.detached = true; },
  };
}

/** A driver whose positions move by +2 in y per tick; `fail` makes the n-th tick reject. */
function fakeDriver({ failAt = -1 } = {}) {
  const log = { loads: [], ticks: 0, inFlight: 0, maxInFlight: 0, destroyed: false };
  let x = new Float32Array(0);
  let v = new Float32Array(0);
  return {
    log,
    marks: "fake/arch",
    lost: new Promise(() => {}),
    async load(state) {
      log.loads.push({ n: state.graph.posX.length, m: state.graph.lo.length, law: state.law, fixture: liveFixture(state.graph, state.law) });
      const n = state.graph.posX.length;
      x = new Float32Array(n * 2);
      v = new Float32Array(n * 2);
      for (let i = 0; i < n; i += 1) {
        x.set([state.graph.posX[i], state.graph.posY[i]], 2 * i);
        v.set([state.vx[i], state.vy[i]], 2 * i);
      }
    },
    pin() {},
    unpinAll() {},
    async tick() {
      log.inFlight += 1;
      log.maxInFlight = Math.max(log.maxInFlight, log.inFlight);
      await sleep(1);
      log.inFlight -= 1;
      if (log.ticks === failAt) throw new Error("device lost: the driver reset");
      log.ticks += 1;
      for (let i = 1; i < x.length; i += 2) x[i] += 2;
      v.fill(3);
    },
    async read(velocities) {
      return { x: x.slice(), v: velocities ? v.slice() : null };
    },
    destroy() { log.destroyed = true; },
  };
}

const opener = (driver) => async () => driver;

test("a_gpu_mesh_ticks_on_the_device_and_writes_the_positions_back", async () => {
  const session = fakeSession(4);
  const driver = fakeDriver();
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  assert.equal(mesh.tier, "gpu");
  const done = await mesh.tick(3);
  assert.equal(done.ticksRun, 3);
  assert.deepEqual(Array.from(session.state.ys), [6, 6, 6, 6], "three device ticks, read back once");
  assert.equal(session.state.cpuTicks, 0, "no CPU tick ran");
  assert.equal(session.state.alpha, mesh.alpha, "the session's alpha is the mesh's after a batch");
  let want = 1;
  for (let k = 0; k < 3; k += 1) want += (0 - want) * 0.06;
  assert.equal(mesh.alpha, want, "the session's own decay, in its own order");
});

test("ticks_never_overlap_and_release_waits_for_the_batch_in_flight", async () => {
  const session = fakeSession(4);
  const driver = fakeDriver();
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  const first = mesh.tick(5);
  const second = mesh.tick(5);
  const released = mesh.release();
  await Promise.all([first, second, released]);
  assert.equal(driver.log.maxInFlight, 1, "two batches must never tick at once");
  assert.equal(driver.log.ticks, 10, "the release waited for both batches");
  assert.ok(driver.log.destroyed && session.state.detached);
  assert.ok(session.state.vxs.every((value) => value === 3), "release writes the velocities back");
  assert.throws(() => mesh.tick(1), { name: "GpuMeshRefusedError" });
});

test("a_lost_device_resumes_from_the_last_readback", async () => {
  const session = fakeSession(4);
  const driver = fakeDriver({ failAt: 3 });
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  await mesh.tick(2);
  const lastX = Array.from(session.state.xs);
  const lastY = Array.from(session.state.ys);
  const done = await mesh.tick(4);
  assert.equal(mesh.tier, "cpu-device-lost");
  assert.match(mesh.reason, /device lost/);
  assert.ok(driver.log.destroyed, "the lost device is freed");
  assert.deepEqual(Array.from(session.state.ys), lastY, "the y the device moved past the last write-back is dropped");
  assert.deepEqual(Array.from(session.state.xs), lastX.map((x) => x + 4), "the failed batch reran on the CPU from the last write-back");
  assert.ok([...session.state.vxs, ...session.state.vys].every((value) => value === 0), "the momentum is dropped");
  assert.equal(done.ticksRun, 4);
});

test("no_adapter_falls_back_to_the_cpu_mesh_and_says_so", async () => {
  const session = fakeSession(4);
  const { mesh } = await attachGpuMesh(session, {}, deviceOpener({}));
  assert.equal(mesh.tier, "cpu-no-adapter");
  assert.match(mesh.reason, /no navigator\.gpu/);
  const before = Array.from(session.state.xs);
  await mesh.tick(2);
  assert.deepEqual(Array.from(session.state.xs), before.map((x) => x + 2), "the CPU tick moved the positions");
});

test("a_software_adapter_is_refused_under_hardware", async () => {
  const limits = {
    maxStorageBufferBindingSize: 2 ** 30,
    maxBufferSize: 2 ** 30,
    maxComputeWorkgroupStorageSize: 32768,
    maxComputeInvocationsPerWorkgroup: 1024,
    maxComputeWorkgroupsPerDimension: 65535,
  };
  let destroyed = 0;
  const adapter = {
    info: { vendor: "google", architecture: "swiftshader" },
    limits,
    features: new Set(),
    requestDevice: async () => ({ destroy: () => { destroyed += 1; } }),
  };
  const host = { gpu: { requestAdapter: async () => adapter } };
  const { mesh } = await attachGpuMesh(fakeSession(4), { arm: "hardware" }, deviceOpener(host));
  assert.equal(mesh.tier, "cpu-no-adapter");
  assert.match(mesh.reason, /swiftshader/, "the reason names the adapter");
  assert.equal(destroyed, 1, "the refused device is freed");
  await assert.rejects(deviceOpener(host)(4, "hardware"), Refusal);
});

test("a_grow_recomputes_the_bias_of_the_edges_whose_degrees_moved", async () => {
  const session = fakeSession(3);
  const driver = fakeDriver();
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  await mesh.tick(1);
  await mesh.grow(0);
  await mesh.tick(1);
  const [before, after] = driver.log.loads.map((load) => load.fixture);
  assert.equal(before.m, 2);
  assert.equal(after.m, 3, "the rig is rebuilt over the grown graph's edges");
  const biasOf = (fixture, edge) => edgeTables(fixture, linkCsr(fixture.edgeLo, fixture.edgeHi, fixture.n)).geometry[edge * 4 + 2];
  // Edge 1 is (1, 2): node 2 had degree 1 and has 2 once (2, 3) is appended.
  assert.equal(biasOf(before, 1), Math.fround(2 / 3));
  assert.equal(biasOf(after, 1), Math.fround(1 / 2), "the bias follows the grown degrees");
});

test("a_grow_rebuilds_the_collide_grid", async () => {
  const session = fakeSession(3);
  const driver = fakeDriver();
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  await mesh.tick(1);
  for (let k = 0; k < 2; k += 1) await mesh.grow(0);
  await mesh.tick(1);
  const [before, after] = driver.log.loads.map((load) => load.fixture);
  assert.deepEqual([before.n, after.n], [3, 5], "the second load is over the grown graph");
  assert.ok(gridFor(after.posX, after.posY).buckets > gridFor(before.posX, before.posY).buckets, "the grid is sized from the grown n");
  assert.ok(session.state.vxs.slice(0, 3).every((value) => value === 3), "the grow synced the device's velocities first");
});

test("set_params_rebuilds_the_rig_at_the_new_law", async () => {
  const session = fakeSession(4);
  const driver = fakeDriver();
  const { mesh } = await attachGpuMesh(session, {}, opener(driver));
  await mesh.tick(1);
  mesh.setParams({ gravity: 0.2, collide_radius: 0 });
  await mesh.tick(1);
  assert.equal(driver.log.loads.length, 2);
  assert.equal(driver.log.loads[1].law.gravity, 0.2);
  assert.equal(driver.log.loads[1].law.collide_radius, 0);
});

test("the_live_fixture_is_graph_cores_side_and_twiddles", () => {
  assert.deepEqual([0, 1, 1000, 16384, 16385, 2_000_000].map(sideFor), [128, 128, 128, 128, 256, 1024]);
  const { re, im } = twiddlesFor(8);
  // Stage half = 4 reads root[j] = exp(-2πij/8) at 4..8.
  assert.ok(Math.abs(re[5] - Math.SQRT1_2) < 1e-15 && Math.abs(im[5] + Math.SQRT1_2) < 1e-15);
  assert.equal(re[1], 1);
  assert.equal(re[3], Math.cos(-Math.PI / 2));
});
