// `ForceSession.gpuMesh()` against a real module, in node, where there is no WebGPU: the GPU
// engine refused at creation with no session left behind, the one-driver rule (the session's
// verbs refuse while a mesh drives it, its reads stay open), the CPU fallback moving the real
// positions, and `release()` handing the session back (`docs/decisions/gpu-g1d.md` conditions
// 4 and 7). The device half is the browser probe's (`deploy/perf/gpu-mesh.py --pass live`).
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-session.test.mjs
//
// Needs a built module at target/wasm32-unknown-unknown/release/graph_wasm.wasm, as
// `motor.test.mjs` does.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { GpuMesh, GpuMeshRefusedError, Motor, resetForTests } from "../src/index.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = join(HERE, "..", "..", "..", "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

if (!existsSync(WASM)) {
  throw new Error(`${WASM} is missing: build it with scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`);
}

const node = (id) => ({
  id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(),
  group: null, weight: 0.5, version: 0, has_note: false, icon: null,
});
const edge = (id, source, target) => ({
  id, source, target, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null,
  child_first: false,
});

/** Six nodes on a path. */
const PATH = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d", "e", "f"].map(node),
  edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d"), edge("e3", "d", "e"), edge("e4", "e", "f")],
});

const copyOf = (session) => {
  const { xs, ys } = session.positions();
  return [...xs, ...ys];
};

let motor;
before(async () => {
  motor = await Motor.create(readFileSync(WASM));
  assert.ok(motor.available, "the module did not load; every test below would be vacuous");
});
after(() => resetForTests());

test("a_gpu_engine_is_refused_by_create_force_session", () => {
  const graph = motor.build(PATH);
  const before = motor.forceSession(graph, undefined, "particle_mesh");
  assert.throws(() => motor.forceSession(graph, undefined, "particle_mesh_gpu"), GpuMeshRefusedError);
  const next = motor.forceSession(graph, undefined, "particle_mesh");
  assert.equal(next.id, before.id + 1, "the refusal reached no create export, so it left no session behind");
  before.release();
  next.release();
});

test("a_session_without_an_adapter_falls_back_to_the_cpu_mesh_and_moves", async () => {
  const graph = motor.build(PATH);
  const session = motor.forceSession(graph, undefined, "particle_mesh");
  const mesh = await session.gpuMesh({ host: {} });
  assert.ok(mesh instanceof GpuMesh);
  assert.equal(mesh.tier, "cpu-no-adapter");
  assert.match(mesh.reason, /navigator\.gpu/, "the reason names what is missing");
  const start = copyOf(session);
  const done = await mesh.tick(3);
  assert.equal(done.ticksRun, 3);
  assert.notDeepEqual(copyOf(session), start, "the CPU mesh moved the session's own positions");
  await mesh.release();
  session.release();
});

test("while_a_mesh_drives_the_session_its_verbs_refuse_and_its_reads_stay_open", async () => {
  const graph = motor.build(PATH);
  const session = motor.forceSession(graph, undefined, "particle_mesh");
  const mesh = await session.gpuMesh({ host: {} });
  for (const verb of [() => session.tick(1), () => session.pin(0, 1, 1), () => session.unpin(0), () => session.unpinAll(),
    () => session.reheat(0.5), () => session.setParams({ gravity: 0.1 }), () => session.grow(graph)]) {
    assert.throws(verb, GpuMeshRefusedError);
  }
  await assert.rejects(session.gpuMesh({ host: {} }), GpuMeshRefusedError, "one driver at a time");
  assert.equal(session.positions().xs.length, 6);
  assert.equal(session.params().gravity, 0);
  assert.equal(typeof session.alpha, "number");
  mesh.pin(0, 5, 5);
  mesh.setParams({ gravity: 0.1 });
  await mesh.tick(1);
  assert.equal(session.params().gravity, 0.1, "the mesh's own verbs reach the session");
  await mesh.release();
  assert.equal(session.tick(1).ticksRun, 1, "release hands the session back");
  session.release();
});

test("a_barnes_hut_session_is_refused_a_gpu_mesh", async () => {
  const graph = motor.build(PATH);
  const session = motor.forceSession(graph);
  await assert.rejects(session.gpuMesh({ host: {} }), GpuMeshRefusedError);
  assert.equal(session.tick(1).ticksRun, 1, "the refused session is still the caller's");
  session.release();
});

test("a_session_released_under_its_mesh_frees_it", async () => {
  const graph = motor.build(PATH);
  const session = motor.forceSession(graph, undefined, "particle_mesh");
  const mesh = await session.gpuMesh({ host: {} });
  session.release();
  assert.equal(mesh.released, true);
  assert.throws(() => mesh.tick(1), GpuMeshRefusedError);
});
