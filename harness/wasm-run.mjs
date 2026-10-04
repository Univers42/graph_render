// The wasm32 arm of the hash gate, under Node: plain WebAssembly.instantiate over the
// same graph_wasm.wasm the browser loads. No wasm-bindgen, no generated glue, and the
// module must import nothing — a self-contained arm tests the real shipped binary.
//
//   node harness/wasm-run.mjs <graph_wasm.wasm> hash <seeds> [--shard i/K] <stage>...
//        prints "<stage> <seed> <sha256>" for each stage in order, seeds 0..N-1.
//        `--shard i/K` runs only the seeds i, i+K, i+2K, ... — a stride, so every shard
//        carries the same mix of cheap and expensive seeds (`graph_core::gate_node_count`
//        grows with the seed number). graph-cli splits an arm's seeds this way across
//        concurrent children and merges the lines back with `hashgate/shard.rs`'s
//        `Shard::parse`, so the flag is optional: absent is `0/1`, the whole run.
//        The stage list is the module's own registry, not a literal list here (C1):
//          topology  gm_topology, the retained shim
//          layout.grid  gm_layout_grid, the retained shim — frozen since Phase 2/3 so
//            their already-green gate keeps hashing the bytes it always hashed
//          every other registered layout  the real ABI: gm_seed_ingest -> gm_alloc ->
//            gm_build -> gm_run(layout id, resolved via gm_layout_id) -> gm_snapshot_bytes,
//            over the same seed's model. C20, and the same path for any layout registered
//            after this file was written: byte-equal to the retained shim's own hash proves
//            the real ABI reaches the pipeline's bytes, not just that it agrees with
//            itself
//          transport.wasm.columnar  the real ABI over layout.grid, so the tally against
//            the shim's hash is the C20 measurement the ledger reads
//          every registered analysis  the real ABI: gm_seed_ingest -> gm_alloc ->
//            gm_build -> gm_analysis_run(index, resolved via gm_analysis_id); its framed
//            canonical-JSON face is hashed. No layout is run — an analysis reads the
//            topology alone
//          every registered POST capability  the real ABI over layout.grid, the same run
//            a POST pass reads: gm_build -> gm_run(layout.grid) -> gm_post_run(index,
//            resolved via gm_post_id) -> gm_snapshot_bytes, whose bytes are hashed. Both
//            registries are the module's own, so a capability registered after this file
//            was written is hashable by name (C1)
//        A seed count of 0 is refused (exit 2), not run: the C20 comparison over no seeds
//        proves nothing, and an exit code that reads as evidence for a comparison that
//        never ran is a vacuous pass. Half the C20 pair is refused too, or noted when the
//        invocation was a single-stage probe — see `wasm-run/hash.mjs`.
//   node harness/wasm-run.mjs <graph_wasm.wasm> probe
//        prints the D1 probe buffer as one hex line
//   node harness/wasm-run.mjs <graph_wasm.wasm> stages
//        prints every stage this arm can hash, one id per line: topology, layout.grid,
//        transport.wasm.columnar, and every layout, analysis and POST capability the
//        module's registries name, in the gate's own order. The stage list is the
//        module's, not a literal list here (C1).
//   node --experimental-strip-types harness/wasm-run.mjs <graph_wasm.wasm> --assert-zero-copy
//        proves the SDK's column views are real zero-copy aliases (C8), that a view held
//        across a memory-growing build is not silently reused stale (C10's growth hazard),
//        and measures view re-derivation cost (C11). Needs --experimental-strip-types: it
//        is the only mode that imports the TypeScript SDK; "hash" and "probe" do not and
//        do not need the flag.
//
// It reads no environment variable. The negative controls (GM_MUTATE_*) perturb the
// native arm only, so a wired mutation has to surface as divergence.
//
// Exit codes follow graph-cli: 0 ran (or passed, for --assert-zero-copy), 1 a checked
// assertion failed (--assert-zero-copy, or hash mode's C20 check below when both
// layout.grid and transport.wasm.columnar are named together), 2 could not run — a
// refusal the arm names, including a module that does not export what this mode drives.
//
// Split under `wasm-run/`, one concern per file, by the house's 300-line limit:
// `lib.mjs` the refusals, the two stage-name constants and the C20 decision; `module.mjs`
// compile/instantiate and the export-presence check; `abi.mjs` framed reads, the three
// registries and the stage table; `hash.mjs` the digests and the C20 comparison;
// `zero-copy.mjs` the SDK aliasing checks.

import { readFile } from "node:fs/promises";
import { parseSeedCount, refuse, Refusal } from "./wasm-run/lib.mjs";
import { loadArm } from "./wasm-run/module.mjs";
import { createAbi } from "./wasm-run/abi.mjs";
import { parseShard, runHash, WHOLE_SHARD } from "./wasm-run/hash.mjs";
import { assertZeroCopy } from "./wasm-run/zero-copy.mjs";

const USAGE = "usage: wasm-run.mjs <wasm> hash <seeds> [--shard i/K] <stage>... | probe | --assert-zero-copy";

/// `--shard i/K` off the front of `hash`'s stage list, and the stages that follow.
///
/// Only where the flag sits is this file's business: the flag is optional, absent means the
/// whole run, and it may appear once, before the first stage. A second `--shard`, or one
/// after a stage, is refused rather than guessed at — `graph-cli` builds this line itself
/// and always puts the flag there, so an invocation shaped differently is a hand-written
/// probe whose intent is not knowable. The parse and its four refusals are `hash.mjs`'s,
/// the same words `hashgate/shard.rs` uses, so the two sides cannot drift.
function takeShard(argv) {
  if (argv[0] !== "--shard") return { shard: WHOLE_SHARD, rest: argv };
  const value = argv[1];
  if (value === undefined || value.startsWith("--")) refuse(`--shard needs an i/K value`);
  return { shard: parseShard(value), rest: argv.slice(2) };
}

/// The arm's modes. Every one but `--assert-zero-copy` drives the module's `exports`
/// directly; that one goes through the SDK, which does its own checks.
async function run(argv) {
  const [wasmPath, mode, count, ...stages] = argv;
  if (!wasmPath || !mode) refuse(USAGE);
  const wasmBytes = await readFile(wasmPath);
  const abi = createAbi(await loadArm(wasmBytes, mode));
  if (mode === "hash") {
    const { shard, rest } = takeShard(stages);
    return runHash(abi, parseSeedCount(count), rest, shard);
  }
  if (mode === "probe") return probe(abi);
  if (mode === "stages") return listStages(abi);
  if (mode === "--assert-zero-copy") return assertZeroCopy(wasmBytes);
  return refuse(`unknown mode ${mode}`);
}

function probe(abi) {
  process.stdout.write(`${abi.probeHex()}\n`);
  return 0;
}

function listStages(abi) {
  process.stdout.write(`${abi.stageList().join("\n")}\n`);
  return 0;
}

try {
  // A zero exit code falls off the end rather than calling `process.exit(0)`: stdout is a
  // pipe under the gate, and `process.stdout.write` + `process.exit` truncates it.
  const code = await run(process.argv.slice(2));
  if (code !== 0) process.exit(code);
} catch (error) {
  if (!(error instanceof Refusal)) throw error;
  process.stderr.write(`wasm-run: ${error.message}\n`);
  process.exit(2);
}
