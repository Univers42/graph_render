// The loader's ABI handshake, over real modules: a hand-assembled wasm that exports
// every name the SDK requires, each bound to one function returning `version`. Only
// `gm_abi_version` is ever called, so the other names need no real body.
//
// The bytes and the export-name list live in `test/stub-module.mjs`, shared with
// `wasm-loader.test.mjs` — one list, checked there against `src/wasm.ts` itself.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { ABI_VERSION, loadMotor, resetForTests } from "../src/wasm.ts";
import { reportingModule } from "./stub-module.mjs";

/** The ABI revision this SDK speaks, **as a literal and not as the constant it imports**.
 *
 *  Both handshake tests used to compare against the imported `ABI_VERSION`, which pinned the
 *  constant to itself: changing `wasm.ts`'s `ABI_VERSION` to `2` left both of them green
 *  (`999 !== 2` and `2 === 2`), so nothing in JavaScript would have noticed the SDK speaking a
 *  revision the Rust side does not. The number is written here, and the test below asserts it
 *  is the number the module and the loader agree on — so moving one without the other is red
 *  in both directions. The only other place the revision is pinned is
 *  `crates/graph-wasm/src/errors/mirrors.rs`.
 *
 *  It is `2` because ABI revision 2 is the one where `gm_run`'s `params_ptr`/`params_len`
 *  stopped being refused and started carrying a layout's parameters, and `gm_layout_params`
 *  joined the module's exports (`docs/contract/wasm-abi.md`, "Exports"). */
const PINNED_ABI_VERSION = 2;

test("a module reporting another ABI version is refused, naming both numbers", async () => {
  resetForTests();
  const error = await loadMotor(reportingModule(999)).catch((caught) => caught);
  assert.ok(error instanceof Error, "the module loaded: nothing checked its ABI version");
  assert.equal(error.name, "WasmUnavailableError", error.message);
  const reason = String(error.reason?.message);
  assert.match(reason, /\b999\b/, reason);
  assert.match(reason, new RegExp(`\\b${PINNED_ABI_VERSION}\\b`), reason);
  resetForTests();
});

test("this SDK's own ABI version is the pinned literal, not whatever the constant says", async () => {
  resetForTests();
  assert.equal(ABI_VERSION, PINNED_ABI_VERSION, "wasm.ts's ABI_VERSION moved from the literal this file pins");
  resetForTests();
});

test("a module reporting this SDK's ABI version loads", async () => {
  resetForTests();
  const exports = await loadMotor(reportingModule(PINNED_ABI_VERSION));
  assert.equal(exports.gm_abi_version(), PINNED_ABI_VERSION);
  resetForTests();
});
