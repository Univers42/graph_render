// The live force port over the real wasm module: a real `ForceSession` from graph-sdk-js,
// driven the way the studio drives it. Every claim here is the motor's, not a stub's.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_KNOBS, type ForceParams } from "../src/motor/live.ts";
import { ALPHA_MIN, createForceHost } from "../src/motor/liveLoop.ts";
import { decodeSnapshot, idAt } from "../../graph-render/src/snapshot/decode.ts";
import { SKIP, FIXTURES_URL, realSession } from "./motor.ts";

const SOURCE = { kind: "fixture", path: "force/tree.json" } as const;
const LAYOUT = "layout.forceatlas2";

interface Rig {
  readonly live: NonNullable<ReturnType<ReturnType<typeof realSession>["forces"]>>;
  readonly ids: readonly string[];
  /** The dense index of `id`, through the same table the studio's drag uses. */
  readonly rowOf: (id: string) => number;
  /** One edge's two ends, as dense indices. */
  readonly edge: () => readonly [number, number];
}

async function rig(): Promise<Rig> {
  const session = realSession();
  await session.open("unused");
  await session.load(SOURCE, FIXTURES_URL);
  const run = await session.layout(LAYOUT, null);
  const live = session.forces();
  if (live === null) throw new Error("the real motor has no live session");
  const snapshot = decodeSnapshot(run.bytes);
  const ids = Array.from({ length: snapshot.nodeCount }, (_, i) => idAt(snapshot.nodeIds, i));
  return {
    live,
    ids,
    rowOf: (id) => ids.indexOf(id),
    edge: () => [snapshot.source[0] ?? 0, snapshot.target[0] ?? 0],
  };
}

function spread(xs: Float64Array, ys: Float64Array): number {
  let lo = Infinity;
  let hi = -Infinity;
  for (let i = 0; i < xs.length; i += 1) {
    lo = Math.min(lo, xs[i] ?? 0, ys[i] ?? 0);
    hi = Math.max(hi, xs[i] ?? 0, ys[i] ?? 0);
  }
  return hi - lo;
}

test("the real session has a live port, and its rows are the drawn nodes", { skip: SKIP }, async () => {
  const { ids, rowOf } = await rig();
  assert.ok(ids.length > 1, "the fixture holds more than one node");
  assert.equal(rowOf(ids[0] ?? ""), 0);
  assert.equal(rowOf("no-such-id"), -1);
});

test("stepping moves the nodes and cools alpha towards alpha_min", { skip: SKIP }, async () => {
  const { live } = await rig();
  const before = live.positions();
  const beforeX = Array.from(before.xs);
  const alpha = live.step(4);
  assert.ok(Number.isFinite(alpha), "the motor answers with an alpha");
  const after = Array.from(live.positions().xs);
  assert.notDeepEqual(after, beforeX, "four ticks moved the graph");
  assert.ok(spread(live.positions().xs, live.positions().ys) > 0, "the nodes are spread out");
});

test("a pin holds the node under the pointer, and its neighbours follow it", { skip: SKIP }, async () => {
  const { live, ids, rowOf, edge } = await rig();
  live.step(2);
  const [source, target] = edge();
  const neighbour = ids[source === 0 ? target : source] ?? "";
  const near = { x: (live.positions().xs[target] ?? 0) + 400, y: (live.positions().ys[target] ?? 0) + 400 };
  const before = { xs: Float64Array.from(live.positions().xs), ys: Float64Array.from(live.positions().ys) };
  live.pin(neighbour, near.x, near.y);
  live.step(8);
  const after = live.positions();
  const row = rowOf(neighbour);
  assert.ok(Math.abs((after.xs[row] ?? 0) - near.x) < 1e-6, "the pinned node is exactly under the pointer");
  assert.ok(Math.abs((after.ys[row] ?? 0) - near.y) < 1e-6);
  const moved = Math.hypot((after.xs[source] ?? 0) - (before.xs[source] ?? 0), (after.ys[source] ?? 0) - (before.ys[source] ?? 0));
  assert.ok(moved > 5, `the neighbour followed it, it moved ${moved}`);
});

test("unpinning lets the node integrate again, and the loop settles", { skip: SKIP }, async () => {
  const { live, ids } = await rig();
  const id = ids[1] ?? "";
  live.pin(id, 900, -900);
  live.step(2);
  live.unpin(id);
  let alpha = 1;
  for (let i = 0; i < 400 && alpha >= ALPHA_MIN; i += 1) alpha = live.step(8);
  assert.ok(alpha < ALPHA_MIN, `the settle reached the motor's own floor, alpha ${alpha}`);
});

/** The motor's own parameters, read back through the ABI: the only honest check. */
function motorParams(live: { params?: () => ForceParams }): ForceParams {
  if (live.params === undefined) throw new Error("the port cannot read the motor's parameters");
  return live.params();
}

test("the four knobs reach the motor's own parameters, and the rest keep its values", { skip: SKIP }, async () => {
  const { live } = await rig();
  const before = motorParams(live);
  live.setParams({ ...DEFAULT_KNOBS, gravity: 0.4, charge: -800, linkStrengthScale: 1.5, linkDistance: 120 });
  const after = motorParams(live);
  assert.deepEqual(
    {
      charge: after.charge,
      gravity: after.gravity,
      link_strength_scale: after.link_strength_scale,
      link_distance: after.link_distance,
    },
    { charge: -800, gravity: 0.4, link_strength_scale: 1.5, link_distance: 120 },
  );
  // The nine fields the studio does not name keep whatever the motor had, never a default
  // copied into this package: that is the whole reason params() is asked rather than guessed.
  for (const field of ["theta", "collide_radius", "velocity_decay", "alpha_decay", "alpha_min"] as const) {
    assert.equal(after[field], before[field], field);
  }
});

test("an out-of-range knob is refused by the motor and the session keeps its values", { skip: SKIP }, async () => {
  const { live } = await rig();
  const before = Float64Array.from(live.positions().xs);
  assert.throws(() => live.setParams({ ...DEFAULT_KNOBS, gravity: 99 }), "the motor range-checks every field");
  assert.deepEqual(Array.from(live.positions().xs), Array.from(before), "and changed nothing");
});

test("the real port drives the loop to a stop on its own", { skip: SKIP }, async () => {
  const { live } = await rig();
  const emitted: { running: boolean; alpha: number }[] = [];
  // WHY a holder and not a `let`: the loop calls `schedule` from inside `handle`, and a local
  // narrowed to null there would make every call site below a type error rather than a value.
  const frames: { run: (() => void) | null } = { run: null };
  const tick = (): void => {
    const run = frames.run;
    frames.run = null;
    run?.();
  };
  const host = createForceHost(() => live, {
    schedule: (run) => {
      frames.run = run;
      return () => { frames.run = null; };
    },
    // A real clock: the loop budgets wall time between ticks, so a clock frozen at zero
    // would run the whole settle inside one frame and prove nothing about the frame budget.
    now: () => performance.now(),
    emit: (result) => {
      if (result.type === "force-frame") emitted.push({ running: result.frame.running, alpha: result.frame.alpha });
    },
  });
  host.handle({ type: "force.start", knobs: DEFAULT_KNOBS });
  for (let i = 0; i < 200; i += 1) tick();
  assert.ok(emitted.length > 1, "the real session settled over more than one frame");
  assert.equal(emitted.at(-1)?.running, false, "and stopped on its own");
});