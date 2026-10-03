// M7 from `docs/reviews/review-harness-sdk.md`: a column answering an illegal (ptr, len) pair
// is refused, not turned into a window. Split out of `motor.test.mjs` by the house's 300-line
// limit, and it needs no built module: it drives `ColumnViews` over a stub.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/views.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import { AbiContractError } from "../src/index.ts";
import { ColumnViews } from "../src/views.ts";

let stub = { ptr: 0, len: 4 };
test("M7: a column answering an illegal (ptr, len) pair is refused, not turned into a window", () => {
  // Driven through `ColumnViews` rather than a real module, because the module never answers an
  // illegal pair — that is the point: the check exists for the module that does, and this is
  // the only way to reach it.
  const buffer = new ArrayBuffer(64);
  const exports = {
    memory: { buffer },
    gm_column_ptr: () => stub.ptr,
    gm_column_len: () => stub.len,
  };
  const views = new ColumnViews(exports);
  const read = () => views.get(1, 0, "Circle", "Line", 0);

  stub = { ptr: 0, len: 4 };
  assert.throws(read, (e) => e instanceof AbiContractError && /present-but-empty column is \(0, 0\)/.test(e.message));
  stub = { ptr: 2, len: 4 };
  assert.throws(read, (e) => e instanceof AbiContractError && /not 4-aligned/.test(e.message));
  stub = { ptr: 60, len: 4 };
  assert.throws(read, (e) => e instanceof AbiContractError && /past the end/.test(e.message));
  stub = { ptr: 16, len: 1 };
  assert.equal(read().length, 1, "a legal pair must still be served zero-copy");
  stub = { ptr: 0, len: 0 };
  assert.equal(read().length, 0, "(0, 0) is present-but-empty and legal");
});
