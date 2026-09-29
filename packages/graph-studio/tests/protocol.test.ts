import assert from "node:assert/strict";
import { test } from "node:test";

import { isRequest, isResult } from "../src/motor/protocol.ts";

test("a message is read only when it has a sequence number and a known body", () => {
  assert.ok(isRequest({ seq: 1, body: { type: "open", wasmUrl: "/m.wasm" } }));
  assert.ok(isRequest({ seq: 2, body: { type: "layout", layoutId: "layout.grid", postId: null } }));
  assert.ok(isResult({ seq: 1, body: { type: "opened", catalog: { layouts: [], posts: [], analyses: [] } } }));
  assert.ok(isResult({ seq: 3, body: { type: "failed", error: { title: "E", code: null, detail: "", hint: "" } } }));
});

const NOT_MESSAGES: readonly (readonly [string, unknown])[] = [
  ["a string", "open"],
  ["null", null],
  ["no sequence number", { body: { type: "open" } }],
  ["a sequence number that is text", { seq: "1", body: { type: "open" } }],
  ["no body", { seq: 1 }],
  ["an unknown type", { seq: 1, body: { type: "shutdown" } }],
  ["a result where a request belongs", { seq: 1, body: { type: "opened" } }],
];

for (const [name, value] of NOT_MESSAGES) {
  test(`${name} is not a request`, () => {
    assert.equal(isRequest(value), false);
  });
}

test("a request is not a result", () => {
  assert.equal(isResult({ seq: 1, body: { type: "open", wasmUrl: "/m.wasm" } }), false);
});
