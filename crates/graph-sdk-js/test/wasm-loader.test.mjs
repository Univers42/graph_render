// The loader (`src/wasm.ts`) over hand-assembled wasm modules, assembled the way
// `test/abi-version.test.mjs` does: no build step, no fixture file, every byte in the test.
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

const EXPORT_NAMES = [
  "gm_abi_version", "gm_alloc", "gm_free", "gm_layout_count", "gm_layout_id", "gm_build",
  "gm_build_contract", "gm_run", "gm_node_count", "gm_geometry_kind", "gm_edge_geometry_kind",
  "gm_dim", "gm_column_ptr", "gm_column_len", "gm_snapshot_json", "gm_snapshot_bytes",
  "gm_post_count", "gm_post_id", "gm_post_run", "gm_analysis_count", "gm_analysis_id",
  "gm_analysis_run", "gm_release", "gm_last_error", "gm_force_session_create",
  "gm_force_session_create_mesh", "gm_force_session_set_params", "gm_force_session_params",
  "gm_force_session_tick", "gm_force_session_alpha", "gm_force_session_reheat",
  "gm_force_session_pin", "gm_force_session_unpin", "gm_force_session_unpin_all",
  "gm_force_session_column_ptr", "gm_force_session_column_len", "gm_force_session_release",
];

const uleb = (n) => (n < 0x80 ? [n] : [(n & 0x7f) | 0x80, ...uleb(n >>> 7)]);
const sleb = (n) => {
  const byte = n & 0x7f;
  const rest = n >> 7;
  const done = (rest === 0 && !(byte & 0x40)) || (rest === -1 && byte & 0x40);
  return done ? [byte] : [byte | 0x80, ...sleb(rest)];
};
const vec = (items) => [...uleb(items.length), ...items.flat()];
const section = (id, body) => [id, ...uleb(body.length), ...body];
const name = (text) => vec([...new TextEncoder().encode(text)].map((b) => [b]));
const HEADER = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

/** A module the loader accepts: one page of memory and `() -> i32 { version }` under every
 *  name in `EXPORT_NAMES`. Only `gm_abi_version` is ever called. `tag` names a custom
 *  section, so two builds differ in bytes while both remain loadable — that is how a test
 *  gets two *different* sources out of one builder. */
function reportingModule(version, tag) {
  const body = [0x00, 0x41, ...sleb(version), 0x0b];
  const exports = [
    [...name("memory"), 0x02, 0x00],
    ...EXPORT_NAMES.map((n) => [...name(n), 0x00, 0x00]),
  ];
  return new Uint8Array([
    ...HEADER,
    ...section(0, name(tag)),
    ...section(1, vec([[0x60, 0x00, 0x01, 0x7f]])),
    ...section(3, vec([[0x00]])),
    ...section(5, vec([[0x00, 0x01]])),
    ...section(7, vec(exports)),
    ...section(10, vec([[...uleb(body.length), ...body]])),
  ]);
}

/** A module whose only section past the header and its type section is an import section
 *  naming each of `wants` as a function. The SDK loads no import object, so such a module is
 *  refused by `refuseImports` — which is the point: the refusal must name the import. */
function importingModule(...wants) {
  const imports = wants.map(([mod, field]) => [...name(mod), ...name(field), 0x00, 0x00]);
  return new Uint8Array([
    ...HEADER,
    ...section(1, vec([[0x60, 0x00, 0x00]])),
    ...section(2, vec(imports)),
    ...section(7, vec([])),
  ]);
}

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
