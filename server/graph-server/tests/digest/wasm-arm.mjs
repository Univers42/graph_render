// The wasm32 arm of svc-digest (`docs/contract/service-api.md` "Verdict", condition 7): every
// entry of a digest manifest run through the real wasm artifact, one line per entry.
//
//   node server/graph-server/tests/digest/wasm-arm.mjs <graph_wasm.wasm> <manifest.json>
//
// Prints `<fixture>\t<source>\t<layout>\t<post or ->\t<digest>` per entry, in manifest order.
// The digest is the hash gate's own: SHA-256, lowercase hex, over the binary snapshot bytes
// `gm_snapshot_bytes` frames (`crates/graph-cli/src/runner.rs` `sha256_hex`). A run the motor
// refuses prints `refused:<gm_last_error>` instead, so a refusal is compared too.
//
// Exit: 0 every entry printed · 2 could not run (the module, the manifest or a fixture).
//
// New glue over the harness's existing pieces (`harness/wasm-run/{module,abi,lib}.mjs`, read,
// never edited): `wasm-run.mjs hash` drives only seeded models, POST passes only over
// `layout.grid` and never `gm_build_contract`, so it cannot hash a manifest entry.

import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createAbi } from "../../../../harness/wasm-run/abi.mjs";
import { refuse, Refusal } from "../../../../harness/wasm-run/lib.mjs";
import { loadArm } from "../../../../harness/wasm-run/module.mjs";

/// The build export each manifest source names: the query's `source=` values.
const BUILDS = { studio: "gm_build", contract: "gm_build_contract" };

function readOrRefuse(path) {
  try {
    return readFileSync(path);
  } catch (error) {
    return refuse(`cannot read ${path}: ${error.message}`);
  }
}

/// A fresh handle over `bytes`, built by the export `source` names; the caller releases it.
function handleOf(exports, bytes, source) {
  const build = exports[BUILDS[source]];
  if (typeof build !== "function") refuse(`no ${BUILDS[source] ?? `build for source ${source}`} export`);
  const ptr = exports.gm_alloc(bytes.length);
  if (ptr === 0) refuse(`gm_alloc refused ${bytes.length} bytes`);
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  const handle = build(ptr, bytes.length);
  exports.gm_free(ptr, bytes.length);
  return handle;
}

/// The entry's digest, or `refused:<code>` when the motor refuses the build, run or pass.
function digestOf(exports, abi, entry, bytes) {
  const refused = () => `refused:${exports.gm_last_error()}`;
  const handle = handleOf(exports, bytes, entry.source);
  if (handle === 0) return refused();
  try {
    if (exports.gm_run(handle, abi.layoutIndex(entry.layout), 0, 0) !== 1) return refused();
    if (entry.post !== null) {
      const index = abi.posts().get(entry.post);
      if (index === undefined) refuse(`no registered POST capability named ${entry.post}`);
      if (exports.gm_post_run(handle, index) !== 1) return refused();
    }
    const snapshot = abi.framed(exports.gm_snapshot_bytes(handle));
    return createHash("sha256").update(snapshot).digest("hex");
  } finally {
    exports.gm_release(handle);
  }
}

async function main([wasmPath, manifestPath]) {
  if (wasmPath === undefined || manifestPath === undefined) {
    refuse("usage: wasm-arm.mjs <graph_wasm.wasm> <manifest.json>");
  }
  const exports = await loadArm(readOrRefuse(wasmPath), "hash");
  const abi = createAbi(exports);
  const { entries } = JSON.parse(readOrRefuse(manifestPath).toString("utf8"));
  const fixtures = new Map();
  const lines = [];
  for (const entry of entries) {
    if (!fixtures.has(entry.fixture)) fixtures.set(entry.fixture, readOrRefuse(entry.fixture));
    const digest = digestOf(exports, abi, entry, fixtures.get(entry.fixture));
    lines.push([entry.fixture, entry.source, entry.layout, entry.post ?? "-", digest].join("\t"));
  }
  process.stdout.write(`${lines.join("\n")}\n`);
}

main(process.argv.slice(2)).catch((error) => {
  if (!(error instanceof Refusal)) throw error;
  process.stderr.write(`wasm-arm: ${error.message}\n`);
  process.exit(2);
});
