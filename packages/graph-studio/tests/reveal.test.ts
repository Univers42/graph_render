// The reveal is transient: it hides nodes past a count in ingest order and leaves the settings alone.
import assert from "node:assert/strict";
import { test } from "node:test";

import { withReveal } from "../src/look/reveal.ts";
import { type RevealDeps, createReveal } from "../src/studio/reveal.ts";

function fixture(total: number) {
  const shown: (number | null)[] = [];
  let clock = 0;
  let pending: (() => void) | null = null;
  const deps: RevealDeps = {
    total: () => total,
    show: (count) => void shown.push(count),
    now: () => clock,
    schedule: (run) => {
      pending = run;
      return () => void (pending = null);
    },
  };
  const tick = (ms: number): void => {
    clock += ms;
    const run = pending;
    pending = null;
    run?.();
  };
  return { deps, shown, tick, waiting: () => pending !== null };
}

test("withReveal hides every node from the count on, keeps what the filter hid, and is null when all show", () => {
  assert.deepEqual([...(withReveal(null, 2, 4) ?? [])], [0, 0, 1, 1]);
  assert.deepEqual([...(withReveal(Uint8Array.of(1, 0, 0, 0), 2, 4) ?? [])], [1, 0, 1, 1]);
  assert.equal(withReveal(null, null, 4), null);
  assert.equal(withReveal(null, 4, 4), null);
});

test("start shows none at once, then a growing prefix, then all", () => {
  const { deps, shown, tick, waiting } = fixture(10);
  const reveal = createReveal(deps);
  reveal.start(100);
  assert.deepEqual(shown, [0]);
  tick(50);
  tick(30);
  tick(20);
  assert.deepEqual(shown, [0, 5, 8, null]);
  assert.equal(waiting(), false);
});

test("cancel stops the steps and keeps the partial state", () => {
  const { deps, shown, tick, waiting } = fixture(10);
  const reveal = createReveal(deps);
  reveal.start(100);
  tick(40);
  assert.equal(reveal.cancel().message, "stopped at 4 of 10 nodes");
  assert.equal(waiting(), false);
  assert.deepEqual(shown, [0, 4]);
  assert.equal(reveal.cancel().message, "nothing is being revealed");
});

test("a zero duration shows everything at once, and a second start restarts", () => {
  const { deps, shown, tick } = fixture(3);
  const reveal = createReveal(deps);
  reveal.start(0);
  assert.deepEqual(shown, [null]);
  reveal.start(30);
  tick(10);
  reveal.start(30);
  assert.deepEqual(shown, [null, 0, 1, 0]);
});
