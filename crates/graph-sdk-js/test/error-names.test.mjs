// Every error class keeps its name under a minifier. A minifier renames `class
// WasmUnavailableError` to `class f`; redefining each constructor's own `name` to "f" here is the
// same rename, so a class that still read `new.target.name` would report "f" and fail below.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import * as errors from "../src/errors.ts";

const classes = Object.entries(errors).filter(
  ([, value]) => typeof value === "function" && value.prototype instanceof Error,
);

test("the error module exports the base and its subclasses", () => {
  assert.ok(classes.length >= 15, `found ${classes.map(([name]) => name).join(", ")}`);
});

for (const [exported, ErrorClass] of classes) {
  test(`${exported} keeps its name once a minifier renames the class`, () => {
    const original = Object.getOwnPropertyDescriptor(ErrorClass, "name");
    Object.defineProperty(ErrorClass, "name", { value: "f", configurable: true });
    try {
      assert.equal(new ErrorClass("probe").name, exported);
    } finally {
      Object.defineProperty(ErrorClass, "name", original);
    }
  });
}
