// The adapter's fallback flag, node-only.
//
// What it pins: an adapter that reports itself as the browser's software fallback reads as one
// whichever build it is on. `isFallbackAdapter` moved between `adapter` and `info` across
// browser builds, and a build that carries it only on `info` must still be seen — or the
// hardware arm holds a software adapter to the hardware ceiling, and a lost device reads as a
// pass.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-adapter.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import { fallbackOf } from "../src/gpu/adapter.ts";

test("a_fallback_adapter_reports_itself_on_info_too", () => {
  // On `adapter`, where this build carries it.
  const onAdapter = { isFallbackAdapter: true };
  assert.equal(
    fallbackOf(onAdapter),
    true,
    "isFallbackAdapter on the adapter must read as a fallback",
  );

  // On `info`, where the other build carries it. This is the defect: the flag was read off
  // `adapter` only, so this adapter read as hardware.
  const onInfo = { info: { isFallbackAdapter: true } };
  assert.equal(
    fallbackOf(onInfo),
    true,
    "isFallbackAdapter on the info must read as a fallback",
  );

  // Neither: a real adapter that says nothing is not a fallback.
  const neither = { info: { vendor: "acme", architecture: "gpu" } };
  assert.equal(fallbackOf(neither), false, "an adapter that says nothing is not a fallback");
});
