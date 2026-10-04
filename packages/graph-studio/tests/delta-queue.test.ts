/**
 * The delta queue's own contract: one extend per batch in arrival order, one grow and one
 * reheat for the burst, one answer per call, and a refused batch that changes nothing but its
 * own answer. Driven through the real host and a fake port, as `force-rig.ts` does.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { createDeltaQueue } from "../src/motor/deltas.ts";
import { type DeltaQueue, RING_LIMIT } from "../src/motor/deltas.ts";
import { type LiveForce } from "../src/motor/live.ts";
import { createForceHost } from "../src/motor/liveLoop.ts";
import type { GraphBatch, Result } from "../src/motor/protocol.ts";

function batch(count: number, tag: string): GraphBatch {
  return {
    nodes: Array.from({ length: count }, (_, at) => ({
      id: `${tag}-${at}`, kind: "record", database_id: null, source: "test", label: `${tag}-${at}`,
      group: null, weight: 1, version: 1, has_note: false, icon: null,
    })),
    edges: [],
  };
}

/** A port that records what it was asked to do, and can refuse one batch by name. */
function port(refuse?: string): LiveForce & { readonly calls: string[]; nodes: number } {
  const calls: string[] = [];
  const made = {
    calls,
    nodes: 10,
    dead: false,
    pin: () => undefined,
    unpin: () => undefined,
    setParams: () => undefined,
    step: () => 0.5,
    positions: () => ({ xs: new Float64Array(made.nodes), ys: new Float64Array(made.nodes) }),
    reheat: (alpha: number) => { calls.push(`reheat ${alpha}`); },
    extend: (one: GraphBatch) => {
      calls.push(`extend ${one.nodes.length}`);
      if (refuse !== undefined && one.nodes[0]?.id === `${refuse}-0`) {
        const refusal = new Error("a node id is already in the graph");
        refusal.name = "BuildRefusedError";
        throw refusal;
      }
      made.nodes += one.nodes.length;
    },
    grow: () => { calls.push("grow"); },
  };
  return made;
}

interface Rig {
  readonly host: ReturnType<typeof createForceHost>;
  readonly emitted: Result[];
  readonly tick: () => void;
}

function rig(port: LiveForce, clock = { now: 0 }): Rig {
  const emitted: Result[] = [];
  let next: (() => void) | null = null;
  const host = createForceHost(() => port, {
    schedule: (run) => { next = run; return () => { next = null; }; },
    now: () => clock.now,
    emit: (result) => { emitted.push(result); },
    structure: async () => null,
  });
  return { host, emitted, tick: () => { const run = next; next = null; run?.(); } };
}

/** Three calls before one tick: three extends, one grow, one reheat, three answers. */
test("three batches queued before one tick cost three extends, one grow and one reheat", async () => {
  const fake = port();
  const { host, tick } = rig(fake);
  const answers = [
    host.deltas(batch(2, "a")),
    host.deltas(batch(3, "b")),
    host.deltas(batch(4, "c")),
  ];
  tick();
  const settled = await Promise.all(answers);
  assert.deepEqual(fake.calls, ["extend 2", "extend 3", "extend 4", "grow", "reheat 0.3"]);
  assert.deepEqual(settled.map((one) => one.type), ["deltas-applied", "deltas-applied", "deltas-applied"]);
  assert.deepEqual(answers.length, 3);
});

test("every answer carries the nodes of its own batch and the graph's size after the grow", async () => {
  const fake = port();
  const { host, tick } = rig(fake);
  const answers = [host.deltas(batch(2, "a")), host.deltas(batch(3, "b"))];
  tick();
  const settled = await Promise.all(answers);
  for (const one of settled) {
    assert.equal(one.type, "deltas-applied");
    if (one.type !== "deltas-applied") continue;
    assert.equal(one.nodeCount, 15);
  }
  assert.deepEqual(settled.map((one) => (one.type === "deltas-applied" ? one.applied : -1)), [2, 3]);
});

/** The negative control: a refused middle batch answers `failed` and the other two apply. */
test("a refused middle batch answers failed, and the other two still go in", async () => {
  const fake = port("b");
  const { host, tick } = rig(fake);
  const answers = [host.deltas(batch(2, "a")), host.deltas(batch(3, "b")), host.deltas(batch(4, "c"))];
  tick();
  const settled = await Promise.all(answers);
  assert.deepEqual(fake.calls, ["extend 2", "extend 3", "extend 4", "grow", "reheat 0.3"]);
  assert.equal(fake.nodes, 16);
  const refused = settled[1];
  assert.equal(refused?.type, "failed");
  if (refused?.type === "failed") assert.equal(refused.error.title, "BuildRefusedError");
  assert.deepEqual(settled.map((one) => one.type), ["deltas-applied", "failed", "deltas-applied"]);
});

test("every batch in a burst of refusals is refused, and no grow runs", async () => {
  const fake = port("b");
  const { host, tick } = rig(fake);
  const answers = [host.deltas(batch(2, "b")), host.deltas(batch(2, "b"))];
  tick();
  const settled = await Promise.all(answers);
  assert.deepEqual(fake.calls, ["extend 2", "extend 2"]);
  assert.deepEqual(settled.map((one) => one.type), ["failed", "failed"]);
});

test("no live session refuses the batch and the graph is untouched", async () => {
  const emitted: Result[] = [];
  const host = createForceHost(() => null, {
    schedule: () => () => undefined,
    now: () => 0,
    emit: (result) => { emitted.push(result); },
  });
  const answer = await host.deltas(batch(2, "a"));
  assert.equal(answer.type, "failed");
  if (answer.type === "failed") assert.match(answer.error.detail, /live forces need the motor session/);
  assert.deepEqual(emitted, []);
});

test("the grow ring keeps only the last 1024 entries", async () => {
  const queue: DeltaQueue = createDeltaQueue({
    extend: () => undefined,
    grow: () => undefined,
    reheat: () => undefined,
    alpha: () => 0.1,
    nodeCount: () => 1,
    emit: () => undefined,
    now: () => 0,
  });
  for (let tick = 1; tick <= RING_LIMIT + 5; tick += 1) {
    queue.push(batch(1, `n${tick}`));
    await queue.drain(tick);
  }
  const marks = queue.grows();
  assert.equal(marks.length, RING_LIMIT);
  assert.deepEqual(marks[0], { tick: 6, batch: 0 });
  assert.deepEqual(marks.at(-1), { tick: RING_LIMIT + 5, batch: 0 });
});
