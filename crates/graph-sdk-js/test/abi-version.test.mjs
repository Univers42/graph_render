// The loader's ABI handshake (F-33), over real modules: a hand-assembled wasm that exports
// every name the SDK requires, each bound to one function returning `version`. Only
// `gm_abi_version` is ever called, so the other names need no real body.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { ABI_VERSION, loadMotor, resetForTests } from "../src/wasm.ts";

const NAMES = [
  "gm_abi_version", "gm_alloc", "gm_free", "gm_layout_count", "gm_layout_id", "gm_build",
  "gm_build_contract",
  "gm_build_columns", "gm_run", "gm_node_count", "gm_geometry_kind", "gm_edge_geometry_kind",
  "gm_dim", "gm_column_ptr", "gm_column_len", "gm_snapshot_json", "gm_snapshot_bytes",
  "gm_post_count", "gm_post_id", "gm_post_run", "gm_analysis_count", "gm_analysis_id",
  "gm_analysis_run", "gm_release", "gm_last_error", "gm_force_session_create",
  "gm_force_session_set_params", "gm_force_session_params", "gm_force_session_tick",
  "gm_force_session_alpha", "gm_force_session_reheat", "gm_force_session_pin",
  "gm_force_session_unpin", "gm_force_session_unpin_all", "gm_force_session_column_ptr",
  "gm_force_session_column_len", "gm_force_session_release",
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

/** A module with no imports, one page of memory, and `() -> i32 { version }` under every name. */
function moduleReporting(version) {
  const body = [0x00, 0x41, ...sleb(version), 0x0b];
  const exports = [[...name("memory"), 0x02, 0x00], ...NAMES.map((n) => [...name(n), 0x00, 0x00])];
  return new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, vec([[0x60, 0x00, 0x01, 0x7f]])),
    ...section(3, vec([[0x00]])),
    ...section(5, vec([[0x00, 0x01]])),
    ...section(7, vec(exports)),
    ...section(10, vec([[...uleb(body.length), ...body]])),
  ]);
}

test("a module reporting another ABI version is refused, naming both numbers", async () => {
  resetForTests();
  const error = await loadMotor(moduleReporting(999)).catch((caught) => caught);
  assert.ok(error instanceof Error, "the module loaded: nothing checked its ABI version");
  assert.equal(error.name, "WasmUnavailableError", error.message);
  const reason = String(error.reason?.message);
  assert.match(reason, /\b999\b/, reason);
  assert.match(reason, new RegExp(`\\b${ABI_VERSION}\\b`), reason);
  resetForTests();
});

test("a module reporting this SDK's ABI version loads", async () => {
  resetForTests();
  const exports = await loadMotor(moduleReporting(ABI_VERSION));
  assert.equal(exports.gm_abi_version(), ABI_VERSION);
  resetForTests();
});
