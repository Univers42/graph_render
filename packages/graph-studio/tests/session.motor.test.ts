// The worker's half, against the real wasm module: what is built, run, read back and
// refused. Gate row `decode-fixtures`: every bundled fixture, every layout that accepts it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";

import { assembleColumns, createMotor, type Handle, MotorTrapError } from "../../../crates/graph-sdk-js/src/index.ts";
import { decodeSnapshot, idAt } from "../../graph-render/src/snapshot/decode.ts";
import { serve } from "../src/motor/serve.ts";
import { type MotorLike, type Session, createSession, sha256Hex } from "../src/motor/session.ts";
import { FIXTURES, FIXTURES_URL, SKIP, WASM, realSession } from "./motor.ts";

const FIXTURE_ROOT = new URL("../../../fixtures/", import.meta.url);

const VAULT = { kind: "synthetic", seed: 1, nodes: 60, degree: 2, shape: "vault" } as const;

test("opening lists what the motor registers", { skip: SKIP }, async () => {
  const catalog = await realSession().open("unused");
  assert.ok(catalog.layouts.includes("layout.forceatlas2"));
  assert.ok(catalog.posts.includes("post.style.bezier"));
  assert.ok(catalog.analyses.includes("analysis.communities.louvain"));
});

test("a layout before any graph is refused", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await assert.rejects(session.layout("layout.grid", null), /no graph is loaded/);
});

test("a generated graph is built, laid out and described in the snapshot's order", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  const graph = await session.load(VAULT, FIXTURES_URL);
  assert.deepEqual([graph.nodeCount, graph.edgeCount, graph.notes], [60, 118, []]);
  const run = await session.layout("layout.forceatlas2", null);
  const snapshot = decodeSnapshot(run.bytes);
  assert.equal(snapshot.nodeCount, 60);
  assert.equal(snapshot.edgeCount, 118);
  assert.ok(run.meta !== null);
  assert.deepEqual(run.meta.ids, Array.from({ length: 60 }, (_, i) => idAt(snapshot.nodeIds, i)));
  assert.equal(run.meta.labels.length, 60);
  assert.equal(run.meta.degree.reduce((sum, degree) => sum + degree, 0), 2 * 118 - selfLoops(snapshot));
  assert.match(run.digest ?? "", /^[0-9a-f]{64}$/);
  assert.equal(run.postId, null);
});

function selfLoops(snapshot: { readonly source: Uint32Array; readonly target: Uint32Array }): number {
  let loops = 0;
  for (let e = 0; e < snapshot.source.length; e += 1) if (snapshot.source[e] === snapshot.target[e]) loops += 1;
  return loops;
}

test("the description is sent once per graph, and again after the next load", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  assert.ok((await session.layout("layout.grid", null)).meta !== null);
  assert.equal((await session.layout("layout.circular.radial", null)).meta, null);
  await session.load({ ...VAULT, seed: 2 }, FIXTURES_URL);
  assert.ok((await session.layout("layout.grid", null)).meta !== null);
});

test("the same graph and stages give the same bytes, and another layout does not", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  const once = await session.layout("layout.forceatlas2", "post.style.bezier");
  const twice = await session.layout("layout.forceatlas2", "post.style.bezier");
  const other = await session.layout("layout.grid", "post.style.bezier");
  assert.equal(once.digest, twice.digest);
  assert.deepEqual(once.bytes, twice.bytes);
  assert.notEqual(once.digest, other.digest);
});

test("an edge pass changes the edges and leaves the nodes where they were", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  const plain = decodeSnapshot((await session.layout("layout.forceatlas2", null)).bytes);
  const run = await session.layout("layout.forceatlas2", "post.style.bezier");
  const curved = decodeSnapshot(run.bytes);
  assert.equal(run.postId, "post.style.bezier");
  assert.equal(run.postError, null);
  assert.equal(curved.edgeKind, "Curve");
  assert.equal(curved.curveDegree, 3);
  assert.deepEqual(curved.x, plain.x);
  assert.deepEqual(curved.y, plain.y);
});

test("an edge pass the motor does not register is reported, and the layout is still drawn", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  const run = await session.layout("layout.grid", "post.style.wobbly");
  assert.equal(run.postId, null);
  assert.ok(run.postError !== null);
  assert.equal(decodeSnapshot(run.bytes).nodeCount, 60);
});

test("an analysis needs no layout and returns one value per node", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  const degree = session.analysis("analysis.centrality.degree");
  assert.equal(degree.values.length, 60);
  assert.ok(degree.values instanceof Float64Array);
  assert.deepEqual([degree.converged, degree.modularity, degree.max], [null, null, null]);
  const depth = session.analysis("analysis.depth.bfs");
  assert.ok(depth.values instanceof Uint32Array);
  assert.equal(depth.max, Math.max(...depth.values));
  assert.equal(typeof session.analysis("analysis.communities.louvain").modularity, "number");
  assert.equal(typeof session.analysis("analysis.centrality.eigenvector").converged, "boolean");
});

test("a fixture that is not one of the bundled ones is refused before anything is fetched", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await assert.rejects(session.load({ kind: "fixture", path: "../Cargo.toml" }, FIXTURES_URL), /not a bundled fixture/);
});

test("a refusal is an answer, not a crash", { skip: SKIP }, async () => {
  const session = realSession();
  assert.equal((await serve(session, { type: "open", wasmUrl: "unused" })).result.type, "opened");
  const refused = await serve(session, { type: "load", source: { kind: "document", name: "bad.json", text: "{" }, fixturesUrl: FIXTURES_URL });
  assert.deepEqual(refused.result, {
    type: "failed",
    error: {
      title: "IngestRefusal", code: null, detail: "bad.json: not JSON",
      hint: "The document is not the ingest shape, or is larger than the studio opens. Fix the JSON, open a smaller graph, or load one of the bundled fixtures.",
    },
  });
  const unknown = await serve(session, { type: "layout", layoutId: "layout.nope", postId: null });
  assert.equal(unknown.result.type, "failed");
});

test("the bytes of a run travel with the answer instead of being copied", { skip: SKIP }, async () => {
  const session = realSession();
  await serve(session, { type: "open", wasmUrl: "unused" });
  await serve(session, { type: "load", source: VAULT, fixturesUrl: FIXTURES_URL });
  const answer = await serve(session, { type: "layout", layoutId: "layout.grid", postId: null });
  assert.ok(answer.result.type === "laid-out");
  assert.deepEqual(answer.transfer, [answer.result.run.bytes.buffer]);
});

/**
 * Every fixture under every layout, counting what decoded and what the motor refused. A trap
 * is neither: it throws, so a broken stage fails the sweep instead of shrinking the count.
 */
async function sweep(session: Session, layouts: readonly string[]): Promise<{ decoded: number; refused: number }> {
  let decoded = 0;
  let refused = 0;
  for (const path of FIXTURES) {
    const graph = await session.load({ kind: "fixture", path }, FIXTURES_URL);
    for (const layoutId of layouts) {
      const answer = await serve(session, { type: "layout", layoutId, postId: null });
      if (answer.result.type === "laid-out") {
        const snapshot = decodeSnapshot(answer.result.run.bytes);
        assert.equal(snapshot.nodeCount, graph.nodeCount, `${path} under ${layoutId}`);
        assert.equal(snapshot.edgeCount, graph.edgeCount, `${path} under ${layoutId}`);
        decoded += 1;
        continue;
      }
      if (answer.result.type === "failed" && answer.result.error.title === "MotorTrapError") {
        throw new Error(`${path} under ${layoutId} trapped: ${answer.result.error.detail}`);
      }
      refused += 1;
    }
  }
  return { decoded, refused };
}

test("every bundled fixture decodes under every layout that accepts it", { skip: SKIP }, async () => {
  const session = realSession();
  const { layouts } = await session.open("unused");
  const { decoded, refused } = await sweep(session, layouts);
  assert.equal(decoded + refused, FIXTURES.length * layouts.length);
  assert.ok(decoded > refused, `${decoded} decoded, ${refused} refused`);
});

test("a trap in the sweep fails it, not counts as a refusal", { skip: SKIP }, async () => {
  const real = await createMotor(WASM ?? new Uint8Array(0));
  const trapping: MotorLike<Handle> = {
    layouts: () => real.layouts(),
    posts: () => real.posts(),
    analyses: () => real.analyses(),
    build: (json) => real.build(json),
    buildColumns: (bytes) => real.buildColumns(bytes),
    layout: () => {
      throw new MotorTrapError("gm_run", new Error("unreachable"));
    },
    post: (handle, postId) => real.post(handle, postId),
    analysis: (handle, analysisId) => real.analysis(handle, analysisId),
    toBytes: (handle) => real.toBytes(handle),
    release: (handle) => real.release(handle),
  };
  const session = createSession({
    motorFrom: () => Promise.resolve(trapping),
    fetchText: (url) => readFile(new URL(url.replace("fixtures:/", ""), FIXTURE_ROOT), "utf8"),
    digest: sha256Hex,
    assemble: assembleColumns,
    now: () => performance.now(),
  });
  const { layouts } = await session.open("unused");
  await assert.rejects(sweep(session, layouts), /trapped/);
});
