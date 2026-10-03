// `--assert-zero-copy` mode: the SDK's column views are real zero-copy aliases (C8), a
// view held across a memory-growing build is not silently reused stale (C10's growth
// hazard), and the cost of re-deriving a view (C11). Split out of `harness/wasm-run.mjs`
// by the house's 300-line limit, and it is the only file here that imports the TypeScript
// SDK — which is why it is the only mode that needs `--experimental-strip-types`.
//
// The call order is load-bearing and unchanged from the single-file arm: the epoch check
// must come before the sentinel is planted, the sentinel before the big build, and the big
// build after the pre-growth view is taken. Each section below says which of those it owns.

/// Plain JS builder for the provisional-ingest document (`crates/graph-wasm/src/ingest.rs`):
/// `n` isolated record nodes (no edges) — enough alone to blow past the module's initial
/// wasm memory once `n` is large, without needing edges to do it.
export function ingestOf(n) {
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

/// One `ok`/`not ok` line, counted. The counter is a named owner rather than a closure
/// variable so each check section can be its own function.
function tally(state) {
  return (name, condition) => {
    process.stdout.write(`${condition ? "ok" : "not ok"} - ${name}\n`);
    if (!condition) state.failures += 1;
  };
}

/// Run the mode over the module's bytes and return the exit code: 0 every check passed,
/// 1 at least one did not. The measured number lands between the checks, so the read order
/// of stdout is the order the checks ran in.
export async function assertZeroCopy(wasmBytes) {
  const { createMotor, ColumnId } = await import("../../crates/graph-sdk-js/src/index.ts");
  const motor = await createMotor(wasmBytes);
  const state = { failures: 0 };
  const check = tally(state);
  const small = motor.build(ingestOf(3));
  motor.layout(small, "layout.grid");
  checkEpoch(check, motor, small, ColumnId);
  checkAliasing(check, motor, small, ColumnId);
  const big = checkGrowthSurvival(check, motor, small, ColumnId);
  checkRevival(check, motor, small, ColumnId);
  measureRedraw(motor, big, ColumnId);
  motor.release(small);
  motor.release(big);
  process.stdout.write(`# ${state.failures === 0 ? "pass" : `${state.failures} failed`}\n`);
  return state.failures === 0 ? 0 : 1;
}

/// Epoch (C10): any motor call moves it forward, so a caller can tell a held view might be
/// stale without diffing bytes. Done *before* the sentinel below: a re-run recomputes
/// layout.grid's own geometry from scratch, which would overwrite a sentinel planted
/// beforehand and make the growth-survival check meaningless.
function checkEpoch(check, motor, small, ColumnId) {
  const before = motor.epoch;
  motor.column(small, ColumnId.NodeY);
  motor.layout(small, "layout.grid"); // re-run: a real, observable state change
  check("a motor call moves the epoch forward (C10)", motor.epoch > before);
}

/// Zero-copy (C8): write a finite sentinel through the column view (not NaN — that would be
/// refused by D9 at encode time, which proves tamper detection, not aliasing), then ask the
/// motor to encode this handle's JSON face. Only a real alias into the same memory the
/// encoder reads from can make the encoded output reflect a JS-side write. Planted after
/// the last mutating call above, so nothing further on this handle recomputes over it
/// before the growth-survival check reads it back.
function checkAliasing(check, motor, small, ColumnId) {
  const view = motor.column(small, ColumnId.NodeX);
  view[1] = 918273.5;
  check(
    "a write through a column view reaches the encoded JSON face (zero-copy, C8)",
    motor.toJSON(small).includes("918273.5"),
  );
}

/// Real memory growth (C10's growth hazard): a view held from before a much larger build on
/// this same motor must not be silently read as if it still aliased live memory once that
/// growth detaches its backing ArrayBuffer — this is the JS engine's own observable
/// behaviour (a grown WebAssembly.Memory's old ArrayBuffer is detached, byteLength 0), not
/// something this SDK has to simulate. Returns the big handle the two sections after this
/// one need; it is built here because this is the section whose growth they are about.
function checkGrowthSurvival(check, motor, small, ColumnId) {
  const heldView = motor.column(small, ColumnId.NodeX);
  const byteLengthBefore = heldView.buffer.byteLength;
  const big = motor.build(ingestOf(200_000)); // ~200k * ~40 B/record >> the initial memory
  motor.layout(big, "layout.grid");
  const grew = heldView.buffer.byteLength !== byteLengthBefore;
  check("a large enough build really does grow wasm memory", grew);
  if (grew) {
    check(
      "the pre-growth view's old buffer is detached, not silently stale (C10)",
      heldView.buffer.byteLength === 0,
    );
  }
  return big;
}

/// The re-derived view is live, non-empty, and still reads the value written before the
/// growth — the other half of the growth hazard, since a detached buffer reads as stale
/// only if the SDK hands the same one back.
function checkRevival(check, motor, small, ColumnId) {
  const revived = motor.column(small, ColumnId.NodeX);
  check(
    "re-deriving the same column after growth returns a live, non-empty view",
    revived.buffer.byteLength > 0 && revived.length === 3,
  );
  check("the re-derived view still reads the value written before growth", revived[1] === 918273.5);
}

/// View re-derivation cost (C11): a measured number for docs/measurements/phase04-transport.md.
function measureRedraw(motor, big, ColumnId) {
  const iterations = 200_000;
  const start = process.hrtime.bigint();
  for (let i = 0; i < iterations; i += 1) motor.column(big, ColumnId.NodeX);
  const elapsedNs = Number(process.hrtime.bigint() - start);
  const per = (elapsedNs / iterations).toFixed(1);
  process.stdout.write(`# view re-derivation: ${per} ns/call over ${iterations} calls\n`);
}
