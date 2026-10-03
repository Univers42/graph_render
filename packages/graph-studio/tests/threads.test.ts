import assert from "node:assert/strict";
import { test } from "node:test";
import { MAX_THREADS, threadsFor } from "../src/motor/threads.ts";

test("a page that is not isolated ticks serially, whatever it asks", () => {
  assert.equal(threadsFor(undefined, 16, false), 1);
  assert.equal(threadsFor(4, 16, false), 1);
});

test("left out, one thread per core but one, at most MAX_THREADS", () => {
  assert.equal(threadsFor(undefined, 4, true), 3);
  assert.equal(threadsFor(undefined, 1, true), 1);
  assert.equal(threadsFor(undefined, 64, true), MAX_THREADS);
});

test("an ask is clamped to 1..MAX_THREADS, and nonsense is serial", () => {
  assert.equal(threadsFor(2, 1, true), 2);
  assert.equal(threadsFor(0, 8, true), 1);
  assert.equal(threadsFor(99, 8, true), MAX_THREADS);
  assert.equal(threadsFor(Number.NaN, 8, true), 1);
});
