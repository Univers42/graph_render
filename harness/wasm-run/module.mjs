// Loading the arm's module: no imports, and every export this mode drives — the review's
// m39. Before this file the only export checked was `gm_probe`, and only in probe mode,
// so a module that lacked `gm_analysis_count`/`gm_post_count`/`gm_snapshot_bytes`/
/// `gm_seed_ingest` died on `exports.gm_snapshot_bytes is not a function` — exit 1 with a
// stack, against the header's own promise that a module this arm cannot drive is exit 2
// "could not run". A missing export is named here instead, all of them at once, so one
// run tells the reader the whole gap.

import { refuse } from "./lib.mjs";

/// The retained Phase 2/3 shims: `hash` and `stages` name them as stages.
const SHIM_EXPORTS = ["gm_topology", "gm_layout_grid"];

/// The real ABI every non-shim stage drives, plus the handle and error verbs around it.
const PIPELINE_EXPORTS = [
  "gm_seed_ingest",
  "gm_alloc",
  "gm_free",
  "gm_build",
  "gm_run",
  "gm_release",
  "gm_snapshot_bytes",
  "gm_last_error",
  "gm_analysis_run",
  "gm_post_run",
];

/// The three registries this arm resolves every layout, analysis and POST name through
/// (C1). `*_count`/`*_id` are read even when a stage list names only one registry, so
/// they are in the set unconditionally.
const REGISTRY_EXPORTS = [
  "gm_layout_count",
  "gm_layout_id",
  "gm_analysis_count",
  "gm_analysis_id",
  "gm_post_count",
  "gm_post_id",
];

/// `[name, wasm kind]` this mode needs, as one list. `memory` is always needed: every
/// framed read in the arm is a view over it. `hash` and `stages` need the whole set.
///
/// `probe` drives one export, and `--assert-zero-copy` drives none directly — it goes
/// through the SDK's own `createMotor`, which does its own export checks — so neither is
/// charged for exports it does not call. Charging for them would have refused a probe of
/// a module built without the layout registry, which is a mode that legitimately works.
export function expectedExports(mode) {
  const driven = mode === "hash" || mode === "stages"
    ? [...SHIM_EXPORTS, ...PIPELINE_EXPORTS, ...REGISTRY_EXPORTS]
    : [];
  return [["memory", "memory"], ...driven.map((name) => [name, "function"])];
}

/// The `[name, kind]` pairs in `expected` the module does not export, in `expected`'s own
/// order so the message is stable. A name exported under the wrong wasm kind (a global
/// named `gm_run`, say) counts as missing: it would fail the same way, one call later,
/// with a worse message.
export function missingExports(descriptors, expected) {
  const kinds = new Map(descriptors.map((descriptor) => [descriptor.name, descriptor.kind]));
  return expected.filter(([name, kind]) => kinds.get(name) !== kind);
}

/// Compile `wasmBytes` and instantiate it, or refuse by name.
///
/// Three refusals, in this order: an import (the arm is self-contained by design — see
/// the file header), a missing export (this file), and whatever the first framed read
/// finds later. `WebAssembly.compile` itself throws on bytes that are not a module; that
/// stays a stack, because a corrupt file is a different failure from a wrong one.
export async function loadArm(wasmBytes, mode) {
  const module = await WebAssembly.compile(wasmBytes);
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) refuse(`module imports ${imports.map((i) => i.name).join(", ")}`);
  const missing = missingExports(WebAssembly.Module.exports(module), expectedExports(mode));
  if (missing.length !== 0) {
    refuse(
      `missing ${missing.length} export(s): ${missing.map(([name]) => name).join(", ")} — ` +
        "this is not a graph_wasm build; rebuild it with " +
        "`scripts/orch/gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release`",
    );
  }
  const { exports } = await WebAssembly.instantiate(module, {});
  return exports;
}
