// The wasm32 arm of the hash gate, under Node: plain WebAssembly.instantiate over the
// same graph_wasm.wasm the browser loads. No wasm-bindgen, no generated glue, and the
// module must import nothing — a self-contained arm tests the real shipped binary.
//
//   node harness/wasm-run.mjs <graph_wasm.wasm> hash <seeds> <stage>...
//        prints "<stage> <seed> <sha256>" for each stage in order, seeds 0..N-1;
//        stages: topology (gm_topology), layout.grid (gm_layout_grid),
//        layout.tree.tidy (gm_layout_tree_tidy),
//        layout.treemap.squarified (gm_layout_treemap_squarified),
//        layout.circular.radial (gm_layout_circular_radial),
//        layout.packing.circle (gm_layout_packing_circle),
//        layout.spectral (gm_layout_spectral), layout.mds.pivot (gm_layout_mds_pivot),
//        layout.dag.sugiyama (gm_layout_dag_sugiyama)
//   node harness/wasm-run.mjs <graph_wasm.wasm> probe
//        prints the D1 probe buffer as one hex line
//
// It reads no environment variable. The negative controls (GM_MUTATE_*) perturb the
// native arm only, so a wired mutation has to surface as divergence.
//
// Exit codes follow graph-cli: 0 ran, 2 could not run.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

function fail(message) {
  process.stderr.write(`wasm-run: ${message}\n`);
  process.exit(2);
}

const [wasmPath, mode, count, ...stages] = process.argv.slice(2);
if (!wasmPath || !mode) fail("usage: wasm-run.mjs <wasm> hash <seeds> <stage>... | probe");

const module = await WebAssembly.compile(await readFile(wasmPath));
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) fail(`module imports ${imports.map((i) => i.name).join(", ")}`);
const { exports } = await WebAssembly.instantiate(module, {});

// Exports return a pointer to [len: u32 LE][len bytes]; 0 means the motor refused.
function framed(ptr) {
  if (ptr === 0) fail("export returned 0: the motor refused (non-finite value or oversize buffer)");
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

const STAGE_EXPORTS = {
  topology: "gm_topology",
  "layout.grid": "gm_layout_grid",
  "layout.tree.tidy": "gm_layout_tree_tidy",
  "layout.treemap.squarified": "gm_layout_treemap_squarified",
  "layout.circular.radial": "gm_layout_circular_radial",
  "layout.packing.circle": "gm_layout_packing_circle",
  "layout.spectral": "gm_layout_spectral",
  "layout.mds.pivot": "gm_layout_mds_pivot",
  "layout.dag.sugiyama": "gm_layout_dag_sugiyama",
};

if (mode === "hash") {
  const seeds = Number.parseInt(count ?? "", 10);
  if (!/^[0-9]+$/.test(count ?? "") || seeds > 0xffffffff) fail(`bad seed count ${count}`);
  if (stages.length === 0) fail("hash needs at least one stage");
  const lines = [];
  for (const stage of stages) {
    const run = exports[STAGE_EXPORTS[stage]];
    if (!Object.hasOwn(STAGE_EXPORTS, stage) || typeof run !== "function") fail(`unknown stage ${stage}`);
    for (let seed = 0; seed < seeds; seed += 1) {
      const digest = createHash("sha256").update(framed(run(seed))).digest("hex");
      lines.push(`${stage} ${seed} ${digest}\n`);
    }
  }
  process.stdout.write(lines.join(""));
} else if (mode === "probe") {
  if (typeof exports.gm_probe !== "function") fail("no gm_probe export: build graph-wasm with --features probe");
  process.stdout.write(`${Buffer.from(framed(exports.gm_probe())).toString("hex")}\n`);
} else {
  fail(`unknown mode ${mode}`);
}
