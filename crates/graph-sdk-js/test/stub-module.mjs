// The hand-assembled wasm modules the loader tests are built from — one builder, one list.
//
// These bytes used to be written twice, in `abi-version.test.mjs` and `wasm-loader.test.mjs`,
// each with its own copy of the export-name list. That is a gate row waiting to go red: when
// ABI revision 2 added `gm_layout_params` to `src/wasm.ts`'s `EXPORT_NAMES`, both copies were
// one name short, so every test that loads a stub failed in `requireExports` — "module lacks
// gm_layout_params" — before reaching the thing it was written to check. Seven failures, none
// of them about the loader. One list, and the check below, is the fix.
//
// No build step and no fixture file: every byte is assembled here, so a test that needs "a
// module this SDK would accept" reads as the module itself rather than as a path.

import { readFileSync } from "node:fs";

/** Every export name `src/wasm.ts` requires of a module, minus `memory` (the builders below
 *  export that one as a memory rather than a function). The list is checked against
 *  `src/wasm.ts` itself when this module is loaded, so it cannot silently fall behind again. */
export const REQUIRED_EXPORT_NAMES = [
  "gm_abi_version", "gm_alloc", "gm_free", "gm_layout_count", "gm_layout_id", "gm_layout_params",
  "gm_build", "gm_build_contract", "gm_build_columns", "gm_run", "gm_node_count",
  "gm_geometry_kind",
  "gm_edge_geometry_kind", "gm_dim", "gm_column_ptr", "gm_column_len", "gm_snapshot_json",
  "gm_snapshot_bytes", "gm_post_count", "gm_post_id", "gm_post_run", "gm_analysis_count",
  "gm_analysis_id", "gm_analysis_run", "gm_release", "gm_last_error", "gm_force_session_create",
  "gm_force_session_create_mesh", "gm_force_session_create_warm", "gm_force_session_set_params",
  "gm_force_session_params",
  "gm_force_session_tick", "gm_force_session_alpha", "gm_force_session_reheat",
  "gm_force_session_pin", "gm_force_session_unpin", "gm_force_session_unpin_all",
  "gm_force_session_column_ptr", "gm_force_session_column_len", "gm_force_session_release",
  "gm_graph_extend", "gm_force_session_grow",
];

/** The names `src/wasm.ts` declares, read out of the file rather than imported: `wasm.ts` does
 *  not export its own list (it is a build-time completeness check, not public surface), and
 *  this one must not be edited to make a test pass. */
function declaredInSource() {
  const source = readFileSync(new URL("../src/wasm.ts", import.meta.url), "utf8");
  const start = source.indexOf("EXPORT_NAMES");
  if (start < 0) throw new Error("src/wasm.ts no longer declares EXPORT_NAMES: this guard is stale, not the list");
  const end = source.indexOf("};", start);
  const names = [...source.slice(start, end).matchAll(/(\w+):\s*true/g)].map((match) => match[1]).filter((n) => n !== "memory");
  if (names.length === 0) throw new Error("src/wasm.ts's EXPORT_NAMES block did not parse; fix this guard rather than skip it");
  return names;
}

{
  const declared = new Set(declaredInSource());
  const listed = new Set(REQUIRED_EXPORT_NAMES);
  const missing = [...declared].filter((n) => !listed.has(n));
  const extra = [...listed].filter((n) => !declared.has(n));
  if (missing.length > 0 || extra.length > 0) {
    throw new Error(
      "test/stub-module.mjs is behind src/wasm.ts's EXPORT_NAMES — " +
      `${missing.length > 0 ? `missing ${missing.join(", ")}` : ""}` +
      `${missing.length > 0 && extra.length > 0 ? "; " : ""}` +
      `${extra.length > 0 ? `no longer declared: ${extra.join(", ")}` : ""}. ` +
      "A stub missing a name is refused by requireExports before the test that wanted it runs.",
    );
  }
}

// --- the bytes ------------------------------------------------------------------------------

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
 *  name in {@link REQUIRED_EXPORT_NAMES}. Only `gm_abi_version` is ever called, so the other
 *  names need no real body.
 *
 *  `tag` names a custom section, so two builds differ in bytes while both remain loadable —
 *  that is how a test gets two *different* sources out of one builder, which is what m9's
 *  "the singleton is built from one source" needs. */
export function reportingModule(version, tag) {
  const body = [0x00, 0x41, ...sleb(version), 0x0b];
  const exports = [
    [...name("memory"), 0x02, 0x00],
    ...REQUIRED_EXPORT_NAMES.map((n) => [...name(n), 0x00, 0x00]),
  ];
  return new Uint8Array([
    ...HEADER,
    ...(tag === undefined ? [] : section(0, name(tag))),
    ...section(1, vec([[0x60, 0x00, 0x01, 0x7f]])),
    ...section(3, vec([[0x00]])),
    ...section(5, vec([[0x00, 0x01]])),
    ...section(7, vec(exports)),
    ...section(10, vec([[...uleb(body.length), ...body]])),
  ]);
}

/** A module whose only section past the header and its type section is an import section
 *  naming each of `wants` as a function. The SDK loads no import object, so such a module is
 *  refused by `refuseImports` — which is the point: the refusal must name the import rather
 *  than surfacing as whatever an empty import object says (m8). */
export function importingModule(...wants) {
  const imports = wants.map(([mod, field]) => [...name(mod), ...name(field), 0x00, 0x00]);
  return new Uint8Array([
    ...HEADER,
    ...section(1, vec([[0x60, 0x00, 0x00]])),
    ...section(2, vec(imports)),
    ...section(7, vec([])),
  ]);
}
