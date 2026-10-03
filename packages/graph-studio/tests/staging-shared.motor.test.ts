// Staging into a shared memory, the threads artifact's. Browsers refuse `encodeInto` into a
// view over a `SharedArrayBuffer` (TypeError "must not be shared") and Node accepts it, so
// these tests put the browser's rule on the SDK's one encoder for their duration. Without the
// shared path the studio built no graph at all on a threaded page; the real browser check is
// `scripts/studio-smoke.sh`. The unshared test is the fast path's guard: it must still encode
// in place, once.
import assert from "node:assert/strict";
import { test } from "node:test";

import { encoder } from "../../../crates/graph-sdk-js/src/calls.ts";
import { buildStaged, type StagedBuild, type StagingExports } from "../../../crates/graph-sdk-js/src/staging.ts";

const SPEC: StagedBuild = {
  buffer: "ingest",
  call: "gm_build",
  refusal: "gm_build refused",
  refuse: (message) => new Error(message),
};

/** A motor's staging exports over `memory`: a bump allocator, and a build that keeps the bytes it was handed. */
function fakeMotor(memory: WebAssembly.Memory): { exports: StagingExports; built: Uint8Array[] } {
  const built: Uint8Array[] = [];
  let top = 16;
  const build = (ptr: number, len: number): number => {
    built.push(new Uint8Array(memory.buffer, ptr, len).slice());
    return 1;
  };
  const exports: StagingExports = {
    memory,
    gm_alloc: (len) => {
      const ptr = top;
      top += len + 8;
      return ptr;
    },
    gm_free: () => undefined,
    gm_last_error: () => 0,
    gm_build: build,
    gm_build_contract: build,
  };
  return { exports, built };
}

/** Runs `body` with `encodeInto` refusing shared views as browsers do, and counts its calls. */
function withBrowserRule(body: () => void): number {
  const native = encoder.encodeInto.bind(encoder);
  let calls = 0;
  encoder.encodeInto = (source, destination) => {
    calls += 1;
    if (destination.buffer instanceof SharedArrayBuffer) throw new TypeError("The provided Uint8Array value must not be shared.");
    return native(source, destination);
  };
  try {
    body();
  } finally {
    // The patch is an own property; deleting it uncovers the prototype's method again.
    Reflect.deleteProperty(encoder, "encodeInto");
  }
  return calls;
}

function stagedText(shared: boolean, text: string): { text: string; encodeIntoCalls: number } {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 1, shared });
  const motor = fakeMotor(memory);
  const encodeIntoCalls = withBrowserRule(() => buildStaged({ exports: motor.exports, views: { bump: () => undefined } }, text, SPEC));
  assert.equal(motor.built.length, 1, "one build call");
  return { text: new TextDecoder().decode(motor.built[0]), encodeIntoCalls };
}

const ASCII = '{"version":1,"nodes":[{"id":"n-0"}],"edges":[]}';
const WIDE = ASCII.replace("n-0", "n-0 é中😀");

test("a shared memory stages an ASCII document whole, without encodeInto", () => {
  assert.deepEqual(stagedText(true, ASCII), { text: ASCII, encodeIntoCalls: 0 });
});

test("a shared memory stages a multi-byte document whole, without encodeInto", () => {
  assert.deepEqual(stagedText(true, WIDE), { text: WIDE, encodeIntoCalls: 0 });
});

test("an unshared memory still encodes ASCII in place, once", () => {
  assert.deepEqual(stagedText(false, ASCII), { text: ASCII, encodeIntoCalls: 1 });
});
