// The loader (`src/wasm.ts`) over hand-assembled wasm modules: no build step, no fixture
// file, every byte in the test. The bytes and the export-name list are `test/stub-module.mjs`,
// shared with `abi-version.test.mjs` — one list, checked there against `src/wasm.ts` itself,
// because two hand-maintained copies of it is how this file went red for a reason that had
// nothing to do with the loader.
//
// m8: a module carrying an import must be refused *by the loader's own check*, naming the
// import — not swallowed by the MIME-fallback retry and resurfacing as whatever the empty
// import object says.
// m9: the singleton is built from one source. A later caller asking for a different one must
// be told, not silently handed the module that is already live.
//
// `loadMotor` latches a failure for the session, so every case brackets itself with
// `resetForTests()`.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { ABI_VERSION, loadMotor, resetForTests } from "../src/wasm.ts";
import { importingModule, reportingModule } from "./stub-module.mjs";

/** A `data:` URL carrying the module as `application/wasm`, so
 *  `WebAssembly.instantiateStreaming` takes its streaming branch (Node's `fetch` resolves
 *  `data:` URLs; the media type is what `instantiateStreaming` insists on). */
const asWasmUrl = (bytes) => `data:application/wasm;base64,${Buffer.from(bytes).toString("base64")}`;

/** The error `loadMotor` rejects with, or `undefined` if it loaded. */
const refusing = async (source) => {
  try {
    await loadMotor(source);
  } catch (caught) {
    return caught;
  }
  return undefined;
};

// --- m8: the import refusal must survive the MIME fallback ---------------------------

test("m8 a module importing m.f is refused naming m.f, not by the empty import object", async () => {
  resetForTests();
  const error = await refusing(asWasmUrl(importingModule(["m", "f"])));
  assert.ok(error !== undefined, "the importing module loaded: nothing checked its imports");
  assert.equal(error.name, "WasmUnavailableError", error.message);
  const reason = String(error.reason?.message);
  assert.match(reason, /m\.f/, reason);
  resetForTests();
});

test("m8 the refusal names every import, not just the first", async () => {
  resetForTests();
  const error = await refusing(asWasmUrl(importingModule(["m", "f"], ["a", "b"])));
  const reason = String(error?.reason?.message);
  assert.match(reason, /m\.f[\s\S]*a\.b/, reason);
  resetForTests();
});

// --- m9: the singleton remembers which source it was built from -----------------------

test("m9 the same source asked for twice loads once", async () => {
  resetForTests();
  const url = asWasmUrl(reportingModule(ABI_VERSION, "one"));
  const first = await loadMotor(url);
  const second = await loadMotor(url);
  assert.equal(first, second, "the second caller got a different module");
  assert.equal(first.gm_abi_version(), ABI_VERSION);
  resetForTests();
});

test("m9 a different source is refused naming both, and resetForTests() loads it", async () => {
  resetForTests();
  const loaded = asWasmUrl(reportingModule(ABI_VERSION, "one"));
  const asked = asWasmUrl(reportingModule(ABI_VERSION, "two"));
  assert.notEqual(loaded, asked, "the two sources must differ for this case to mean anything");
  await loadMotor(loaded);
  const error = await refusing(asked);
  assert.ok(error !== undefined, "a second module was bound over the first, silently");
  assert.equal(error.name, "WasmUnavailableError", error.message);
  assert.match(error.message, /already loaded[\s\S]*resetForTests/, error.message);
  // The escape hatch is the documented one, and after it the other source is loadable.
  resetForTests();
  const other = await loadMotor(asked);
  assert.equal(other.gm_abi_version(), ABI_VERSION);
  resetForTests();
});

test("m9 the comparison is by key: a string and a URL agree, two byte copies do not", async () => {
  resetForTests();
  const url = asWasmUrl(reportingModule(ABI_VERSION, "href"));
  await loadMotor(url);
  const sameByHref = await loadMotor(new URL(url));
  assert.equal(sameByHref.gm_abi_version(), ABI_VERSION);
  // Bytes are compared by identity: two arrays holding equal bytes are still two sources, and
  // the singleton cannot know they are equal without hashing every load.
  const bytes = reportingModule(ABI_VERSION, "bytes");
  const error = await refusing(new Uint8Array(bytes));
  assert.match(String(error?.message), /resetForTests/, String(error?.message));
  resetForTests();
  const sameObject = await loadMotor(bytes);
  assert.equal(sameObject.gm_abi_version(), ABI_VERSION);
  assert.equal(await loadMotor(bytes), sameObject, "the same bytes object must load once");
  resetForTests();
});
