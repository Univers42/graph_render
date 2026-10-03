// `src/options.ts`: the closed option shape this phase (C16). Every refusal must be an
// `InvalidOptionsError` naming the option — a raw `TypeError` out of `Object.keys` is a stack
// trace where a caller expects a diagnosis.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { checkOptions } from "../src/options.ts";
import { InvalidOptionsError } from "../src/errors.ts";

/** Asserts `checkOptions` refuses, and that the message names what to change. */
function refuses(options, pattern) {
  const error = (() => {
    try {
      checkOptions(options);
    } catch (caught) {
      return caught;
    }
    return undefined;
  })();
  assert.ok(error !== undefined, "the options were accepted instead of being refused");
  assert.ok(error instanceof InvalidOptionsError, `${error.name}: ${error.message}`);
  assert.match(error.message, pattern, error.message);
}

test("m15 `null` is refused as InvalidOptionsError, not a raw TypeError", () => {
  refuses(null, /options/);
});

test("m15 an array is refused: it is an object but not an option bag", () => {
  refuses([], /options/);
});

test("m15 a non-object argument is refused", () => {
  refuses(42, /options/);
  refuses("auto", /options/);
  refuses(true, /options/);
  refuses(() => {}, /options/);
});

test("m15 `undefined` is still the documented no-options case", () => {
  checkOptions(undefined);
});

test("m15 `{ exec: 'auto' }` and `{}` are accepted", () => {
  checkOptions({});
  checkOptions({ exec: "auto" });
});

test("an unknown key is refused, naming it", () => {
  refuses({ exec: "auto", threads: 4 }, /threads/);
});

test("an `exec` other than `auto` is refused, citing the decision record", () => {
  refuses({ exec: "cpu" }, /exec[\s\S]*auto[\s\S]*compute-tiers\.md/);
});
