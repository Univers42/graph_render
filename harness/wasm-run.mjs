// The wasm32 arm of the hash gate, under Node: plain WebAssembly.instantiate over the
// same graph_wasm.wasm the browser loads. No wasm-bindgen, no generated glue, and the
// module must import nothing — a self-contained arm tests the real shipped binary.
//
//   node harness/wasm-run.mjs <graph_wasm.wasm> hash <seeds> <stage>...
//        prints "<stage> <seed> <sha256>" for each stage in order, seeds 0..N-1.
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
//   node harness/wasm-run.mjs <graph_wasm.wasm> probe
//        prints the D1 probe buffer as one hex line
//   node harness/wasm-run.mjs <graph_wasm.wasm> stages
//        prints every stage this arm can hash, one id per line: topology, layout.grid,
//        transport.wasm.columnar, then every layout the module's registry names. The
//        stage list is the module's, not a literal list here (C1).
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
// layout.grid and transport.wasm.columnar are named together), 2 could not run.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

function fail(message) {
  process.stderr.write(`wasm-run: ${message}\n`);
  process.exit(2);
}

const [wasmPath, mode, count, ...stages] = process.argv.slice(2);
if (!wasmPath || !mode) fail("usage: wasm-run.mjs <wasm> hash <seeds> <stage>... | probe | --assert-zero-copy");

const wasmBytes = await readFile(wasmPath);
const module = await WebAssembly.compile(wasmBytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) fail(`module imports ${imports.map((i) => i.name).join(", ")}`);
const { exports } = await WebAssembly.instantiate(module, {});

// Exports return a pointer to [len: u32 LE][len bytes]; 0 means the motor refused.
function framed(ptr) {
  if (ptr === 0) fail("export returned 0: the motor refused (non-finite value or oversize buffer)");
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

function decodeUtf8(bytes) {
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

/// The layout the transport stage runs, and the one the retained `gm_layout_grid` shim
/// hashes. Both are this layout, named once: the shim predates this phase's real ABI and
/// is frozen so Phase 2/3's already-green gate keeps hashing the bytes it always hashed,
/// so this stage stays shim-backed while every other registered layout is hashed through
/// the real ABI. `abiSnapshotBytes` drives this id through `gm_run`/`gm_snapshot_bytes`,
/// and `hashgate`'s native side names it in `hashgate/stages.rs` as `LAYOUT`.
const SHIM_LAYOUT = "layout.grid";

/// The module's own layout registry, id -> index, read once (C1). Every layout the gate
/// hashes other than the retained shim's is resolved through here rather than through a
/// constant, so a layout registered after this file was written is hashable by name.
///
/// The one derivation, deliberately: this arm resolves a layout name in three places
/// (`layoutIndices` for the stage list, `layoutIndex` below for `gm_run`, and the
/// `hash` mode's own membership test), and three scans of `gm_layout_count` is three
/// places for the registry to be read differently.
let registry = null;
function layoutIndices() {
  if (registry === null) {
    registry = new Map();
    const total = exports.gm_layout_count();
    for (let i = 0; i < total; i += 1) {
      registry.set(decodeUtf8(framed(exports.gm_layout_id(i))), i);
    }
  }
  return registry;
}

/// The registry index of `name`, or could-not-run. `gm_run`'s `layout_id` argument is this
/// index, so a name the module does not carry is refused here rather than run as whatever
/// happens to sit at that index.
function layoutIndex(name) {
  const index = layoutIndices().get(name);
  if (index === undefined) fail(`no registered layout named ${name}`);
  return index;
}

/// The registered layout ids, in registry order: this arm's layout stage list, which is
/// the module's registry rather than a literal list here.
function hashableLayouts() {
  return [...layoutIndices().keys()];
}

/// C20: drives the *real* ABI — gm_seed_ingest's provisional-ingest text through
/// gm_alloc/gm_build/gm_run/gm_snapshot_bytes — for the gate's seed model, so its hash can
/// be asserted equal to the retained gm_layout_grid shim's (both are the binary face of
/// the same model run through the same layout).
function abiSnapshotBytes(seed, layout) {
  const ingest = framed(exports.gm_seed_ingest(seed));
  const ptr = exports.gm_alloc(ingest.length);
  if (ptr === 0) fail(`gm_alloc refused ${ingest.length} bytes (seed ${seed})`);
  new Uint8Array(exports.memory.buffer, ptr, ingest.length).set(ingest);
  const handle = exports.gm_build(ptr, ingest.length);
  exports.gm_free(ptr, ingest.length);
  if (handle === 0) fail(`gm_build refused (seed ${seed}, gm_last_error ${exports.gm_last_error()})`);
  const ok = exports.gm_run(handle, layoutIndex(layout), 0, 0);
  if (ok !== 1) fail(`gm_run refused (seed ${seed}, gm_last_error ${exports.gm_last_error()})`);
  const bytes = framed(exports.gm_snapshot_bytes(handle));
  exports.gm_release(handle);
  return bytes;
}

// The stages this arm can hash without a layout id: the two retained shims, unchanged
// since Phase 2/3 so their already-green gate keeps hashing the same bytes, and the
// transport, which is the real ABI over the shim's layout. Every other stage is a
// registered layout id, hashed through the real ABI (see the `stages` mode above).
//
// A **null prototype**, so a lookup cannot fall through to `Object.prototype`: an object
// literal would answer `STAGE_BYTES["toString"]` with `Object.prototype.toString`, and
// the arm would then *hash* that member's return value under a stage nobody ran — a
// digest of the string "[object Undefined]" and exit 0 for a stage that does not exist.
// An unknown stage is refused by name instead (exit 2, "could not run"), never hashed as
// something else; `hashgate/tests/stages/arm.rs` holds both directions.
const STAGE_BYTES = Object.assign(Object.create(null), {
  topology: (seed) => framed(exports.gm_topology(seed)),
  [SHIM_LAYOUT]: (seed) => framed(exports.gm_layout_grid(seed)),
  "transport.wasm.columnar": (seed) => abiSnapshotBytes(seed, SHIM_LAYOUT),
});

if (mode === "hash") {
  const seeds = Number.parseInt(count ?? "", 10);
  if (!/^[0-9]+$/.test(count ?? "") || seeds > 0xffffffff) fail(`bad seed count ${count}`);
  if (stages.length === 0) fail("hash needs at least one stage");
  const lines = [];
  const digestsByStage = new Map(); // stage -> [digest, ...] by seed, for the C20 check below
  for (const stage of stages) {
    // A shim-backed stage keeps its frozen hasher. Any other stage must name a layout the
    // module itself registered, resolved through gm_layout_id (C1): a new registry row
    // joins the gate with no change to this file, and a misspelt stage is refused rather
    // than silently hashed as something else.
    const bytesOf =
      STAGE_BYTES[stage] ??
      ((seed) => {
        if (!layoutIndices().has(stage)) fail(`unknown stage ${stage}: not a registered layout`);
        return abiSnapshotBytes(seed, stage);
      });
    const digests = [];
    for (let seed = 0; seed < seeds; seed += 1) {
      const digest = createHash("sha256").update(bytesOf(seed)).digest("hex");
      digests.push(digest);
      lines.push(`${stage} ${seed} ${digest}\n`);
    }
    digestsByStage.set(stage, digests);
  }
  process.stdout.write(lines.join(""));

  // C20: when both stages were asked for in the same invocation, this is the actual
  // enforced proof that the real ABI (transport.wasm.columnar) reaches the same bytes as
  // the retained hash-gate shim (layout.grid) — not just two printed digest lists a human
  // would otherwise have to compare by eye. Exits 1 on the first divergence found.
  const reference = digestsByStage.get("layout.grid");
  const transport = digestsByStage.get("transport.wasm.columnar");
  if (reference && transport) {
    for (let seed = 0; seed < seeds; seed += 1) {
      if (reference[seed] !== transport[seed]) {
        process.stderr.write(
          `wasm-run: C20 FAIL — transport.wasm.columnar diverges from layout.grid at seed ${seed} ` +
            `(layout.grid ${reference[seed]}, transport.wasm.columnar ${transport[seed]})\n`,
        );
        process.exit(1);
      }
    }
    process.stderr.write(`wasm-run: C20 ok — transport.wasm.columnar == layout.grid on all ${seeds} seeds\n`);
  }
} else if (mode === "probe") {
  if (typeof exports.gm_probe !== "function") fail("no gm_probe export: build graph-wasm with --features probe");
  process.stdout.write(`${Buffer.from(framed(exports.gm_probe())).toString("hex")}\n`);
} else if (mode === "--assert-zero-copy") {
  await assertZeroCopy();
} else if (mode === "stages") {
  // Every stage the gate can hash, one id per line: the two shim-backed ones, the
  // transport, then every layout the module's own registry names (C1). A test asserts
  // graph-cli's stage list is exactly this, so the two arms cannot drift apart.
  process.stdout.write(
    `${["topology", ...hashableLayouts(), "transport.wasm.columnar"].join("\n")}\n`,
  );
} else {
  fail(`unknown mode ${mode}`);
}

/// Plain JS builder for the provisional-ingest document (`crates/graph-wasm/src/ingest.rs`):
/// `n` isolated record nodes (no edges) — enough alone to blow past the module's initial
/// wasm memory once `n` is large, without needing edges to do it.
function ingestOf(n) {
  const nodes = [];
  for (let i = 0; i < n; i += 1) {
    nodes.push({
      id: `n${i}`,
      kind: "record",
      database_id: null,
      source: "s",
      label: "",
      group: null,
      weight: 0.5,
      version: 0,
      has_note: false,
      icon: null,
    });
  }
  return JSON.stringify({ version: 1, nodes, edges: [] });
}

async function assertZeroCopy() {
  const { createMotor, ColumnId } = await import("../crates/graph-sdk-js/src/index.ts");
  const motor = await createMotor(wasmBytes);
  let failures = 0;
  const check = (name, condition) => {
    process.stdout.write(`${condition ? "ok" : "not ok"} - ${name}\n`);
    if (!condition) failures += 1;
  };

  const small = motor.build(ingestOf(3));
  motor.layout(small, "layout.grid");

  // Epoch (C10): any motor call moves it forward, so a caller can tell a held view might
  // be stale without diffing bytes. Do this *before* planting the sentinel below: a
  // re-run recomputes layout.grid's own geometry from scratch, which would overwrite a
  // sentinel planted beforehand and make the later growth-survival check meaningless.
  const epochBefore = motor.epoch;
  motor.column(small, ColumnId.NodeY);
  motor.layout(small, "layout.grid"); // re-run: a real, observable state change
  check("a motor call moves the epoch forward (C10)", motor.epoch > epochBefore);

  // Zero-copy (C8): write a finite sentinel through the column view (not NaN — that would
  // be refused by D9 at encode time, which proves tamper detection, not aliasing), then
  // ask the motor to encode this handle's JSON face. Only a real alias into the same
  // memory the encoder reads from can make the encoded output reflect a JS-side write.
  // Planted after the last mutating call above, so nothing further on this handle
  // recomputes over it before the growth-survival check below reads it back.
  const xView = motor.column(small, ColumnId.NodeX);
  xView[1] = 918273.5;
  const json = motor.toJSON(small);
  check("a write through a column view reaches the encoded JSON face (zero-copy, C8)", json.includes("918273.5"));

  // Real memory growth (C10's growth hazard): a view held from before a much larger build
  // on this same motor must not be silently read as if it still aliased live memory once
  // that growth detaches its backing ArrayBuffer — this is the JS engine's own observable
  // behaviour (a grown WebAssembly.Memory's old ArrayBuffer is detached, byteLength 0),
  // not something this SDK has to simulate.
  const heldView = motor.column(small, ColumnId.NodeX);
  const byteLengthBefore = heldView.buffer.byteLength;
  const big = motor.build(ingestOf(200_000)); // ~200k * ~40 B/record >> the module's initial memory
  motor.layout(big, "layout.grid");
  const grew = heldView.buffer.byteLength !== byteLengthBefore;
  check("a large enough build really does grow wasm memory", grew);
  if (grew) {
    check("the pre-growth view's old buffer is detached, not silently stale (C10)", heldView.buffer.byteLength === 0);
  }
  const revived = motor.column(small, ColumnId.NodeX);
  check("re-deriving the same column after growth returns a live, non-empty view", revived.buffer.byteLength > 0 && revived.length === 3);
  check("the re-derived view still reads the value written before growth", revived[1] === 918273.5);

  // View re-derivation cost (C11): a measured number for docs/measurements/phase04-transport.md.
  const iterations = 200_000;
  const start = process.hrtime.bigint();
  for (let i = 0; i < iterations; i += 1) motor.column(big, ColumnId.NodeX);
  const elapsedNs = Number(process.hrtime.bigint() - start);
  process.stdout.write(`# view re-derivation: ${(elapsedNs / iterations).toFixed(1)} ns/call over ${iterations} calls\n`);

  motor.release(small);
  motor.release(big);
  process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
  process.exit(failures === 0 ? 0 : 1);
}
