/**
 * The structure snapshot's cadence: at most twice a second while batches keep arriving, and at
 * once when a burst is over — the case `docs/contract/delta.md` calls the queue draining. The
 * clock is injected, so a "second" here is a number and not a wait.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { STRUCTURE_MS, createDeltaQueue } from "../src/motor/deltas.ts";
import type { GraphBatch, Result, RunReport } from "../src/motor/protocol.ts";

function batch(tag: string): GraphBatch {
  return {
    nodes: [{
      id: tag, kind: "record", database_id: null, source: "test", label: tag,
      group: null, weight: 1, version: 1, has_note: false, icon: null,
    }],
    edges: [],
  };
}

/** The detail of a refusal, or a failure naming what came back instead. */
function failedOf(result: Result): string {
  if (result.type !== "failed") throw new Error(`expected failed, got ${result.type}`);
  return result.error.detail;
}

function report(nodeCount: number): RunReport {
  return {
    layoutId: "layout.random", postId: null, postError: null,
    bytes: new Uint8Array(nodeCount), digest: null, layoutMs: 1, postMs: 0, meta: null,
  };
}

interface Cadence {
  readonly queue: ReturnType<typeof createDeltaQueue>;
  readonly emitted: Result[];
  /** One batch and the tick that applied it, at the clock's own time. */
  readonly at: (ms: number) => Promise<void>;
  /** A tick with nothing queued: the burst before it is over. */
  readonly idle: (ms: number) => Promise<void>;
}

function cadence(): Cadence {
  const emitted: Result[] = [];
  const clock = { now: 0 };
  const queue = createDeltaQueue({
    extend: () => undefined,
    grow: () => undefined,
    reheat: () => undefined,
    alpha: () => 0.1,
    nodeCount: () => 1,
    structure: () => Promise.resolve(report(1)),
    emit: (result) => { emitted.push(result); },
    now: () => clock.now,
  });
  const at = async (ms: number) => {
    clock.now = ms;
    void queue.push(batch(`n${ms}`));
    await queue.drain(ms);
  };
  const idle = async (ms: number) => {
    clock.now = ms;
    await queue.drain(ms);
  };
  return { queue, emitted, at, idle };
}

test("a burst that never pauses rebuilds at most twice a simulated second", async () => {
  const { queue, emitted, at } = cadence();
  for (const ms of [0, 16, 32, 48, 64, 200, 400, 600, 800, 1000]) await at(ms);
  const structures = emitted.filter((one) => one.type === "deltas-structure").length;
  assert.ok(structures <= 2, `${structures} rebuilds in 1000 ms`);
  assert.equal(structures, queue.rebuilds());
  assert.ok(structures >= 1, "a batch that applied must be drawn at least once");
});

test("a burst with no idle tick rebuilds at t=0, then once a cadence: t=500, t=1000", async () => {
  const { queue, emitted, at } = cadence();
  for (const ms of [0, 100, 200, 300, 400, 500, 600, 700, 800, 900]) await at(ms);
  const drawn = (): number => emitted.filter((one) => one.type === "deltas-structure").length;
  assert.equal(drawn(), 2);
  assert.equal(queue.rebuilds(), 2);
  await at(1000);
  assert.equal(drawn(), 3);
  assert.equal(queue.rebuilds(), 3);
});

test("a burst that ends is drawn at once, without waiting out the cadence", async () => {
  const { queue, at, idle } = cadence();
  await at(0);
  await at(16);
  assert.equal(queue.rebuilds(), 1);
  // Nothing was queued for the tick before, so this burst is over and the nodes can be drawn.
  await idle(32);
  await at(48);
  assert.equal(queue.rebuilds(), 2);
  assert.equal(STRUCTURE_MS, 500);
});

test("a refused rebuild leaves the queue answering and counts nothing extra", async () => {
  const emitted: Result[] = [];
  const queue = createDeltaQueue({
    extend: () => undefined,
    grow: () => undefined,
    reheat: () => undefined,
    alpha: () => 0.1,
    nodeCount: () => 1,
    structure: () => Promise.reject(new Error("RunRefusedError")),
    emit: (result) => { emitted.push(result); },
    now: () => 0,
  });
  const answer = queue.push(batch("one"));
  await queue.drain(1);
  assert.equal((await answer).type, "deltas-applied");
  assert.deepEqual(emitted, []);
  assert.equal(queue.rebuilds(), 1);
});

test("a queue whose port has no extend path refuses every batch without queueing it", async () => {
  const emitted: Result[] = [];
  const queue = createDeltaQueue({
    grow: () => undefined,
    reheat: () => undefined,
    alpha: () => 0.1,
    nodeCount: () => 1,
    emit: (result) => { emitted.push(result); },
    now: () => 0,
  });
  const answer = await queue.push(batch("one"));
  assert.match(failedOf(answer), /cannot add to a built graph/);
  assert.equal(queue.rebuilds(), 0);
  await queue.drain(1);
  assert.deepEqual(queue.grows(), []);
  assert.deepEqual(emitted, []);
});
