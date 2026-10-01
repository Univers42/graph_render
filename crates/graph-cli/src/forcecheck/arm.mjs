// The wasm32 arm of the force gate, under Node: the same real `graph_wasm.wasm` the browser
// loads, driven through the live session ABI and nothing else. Plain `WebAssembly.instantiate`,
// no wasm-bindgen and no generated glue, and the module must import nothing — a self-contained
// arm tests the shipped binary.
//
//   node arm.mjs <graph_wasm.wasm> <seeds> <ticks>
//
// Prints `<stage> <seed> <sha256>` per seed, in the order `force-gate`'s comparator reads.
//
// The path is the real ABI end to end, and every step is one a studio takes:
//   gm_seed_ingest -> gm_alloc -> gm_build -> gm_force_session_create -> gm_force_session_tick
//   -> gm_force_session_column_ptr/_len -> the two f64 columns
// so a stage the module cannot export would fail this arm loudly (exit 2) rather than quietly
// drop out of a comparison.
//
// It reads no environment variable. The negative control (GM_MUTATE_*) perturbs the native arm
// only, so a wired mutation has to surface as a divergence.
//
// Exit codes follow graph-cli: 0 ran, 2 could not run.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

function fail(message) {
  process.stderr.write(`force-arm: ${message}\n`);
  process.exit(2);
}

const [wasmPath, count, ticksArg] = process.argv.slice(2);
if (!wasmPath || !count || !ticksArg) fail("usage: arm.mjs <wasm> <seeds> <ticks>");

// The stage id is `force-gate`'s STAGE, written here because the two are different languages;
// `force-gate`'s own unit test refuses to let the pair drift further than this one string.
const STAGE = "force.session.positions";
const X_AXIS = 0;
const Y_AXIS = 1;

const seeds = Number.parseInt(count, 10);
const ticks = Number.parseInt(ticksArg, 10);
if (!/^[0-9]+$/.test(count) || !/^[0-9]+$/.test(ticksArg)) fail(`bad counts ${count} ${ticksArg}`);

const wasmBytes = await readFile(wasmPath);
const module = await WebAssembly.compile(wasmBytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) fail(`module imports ${imports.map((i) => i.name).join(", ")}`);
const { exports } = await WebAssembly.instantiate(module, {});

for (const name of ["gm_force_session_create", "gm_force_session_tick", "gm_force_session_column_ptr"]) {
  if (typeof exports[name] !== "function") fail(`no ${name} export: the module predates the force session ABI`);
}

function refuse(what, seed) {
  fail(`${what} refused (seed ${seed}, gm_last_error ${exports.gm_last_error()})`);
}

/// `[len: u32 LE][len bytes]` at `ptr`; 0 means the motor refused.
function framed(ptr) {
  if (ptr === 0) fail("export returned 0: the motor refused");
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

/// The seed's model on a fresh handle, which the caller releases.
function buildHandle(seed) {
  const ingest = framed(exports.gm_seed_ingest(seed));
  const ptr = exports.gm_alloc(ingest.length);
  if (ptr === 0) fail(`gm_alloc refused ${ingest.length} bytes (seed ${seed})`);
  new Uint8Array(exports.memory.buffer, ptr, ingest.length).set(ingest);
  const handle = exports.gm_build(ptr, ingest.length);
  exports.gm_free(ptr, ingest.length);
  if (handle === 0) refuse("gm_build", seed);
  return handle;
}

/// One column of the session's own storage, as the bytes the native arm hashed: the `f64`s in
/// row order, little-endian, exactly as a `Vec<f64>` holds them.
function columnBytes(session, axis, seed) {
  const address = exports.gm_force_session_column_ptr(session, axis);
  const len = exports.gm_force_session_column_len(session, axis);
  if (address === 0 || len === 0) refuse(`gm_force_session_column(${axis})`, seed);
  const view = new Float64Array(exports.memory.buffer, address, len);
  // The three-argument form, because `view.buffer` is the whole `WebAssembly.Memory` — hashing
  // that would hash every byte of the module instead of the column.
  return Buffer.from(view.buffer, view.byteOffset, view.byteLength);
}

function digestFor(seed) {
  const handle = buildHandle(seed);
  // `params_len == 0` is the compiled-in defaults: the same set `LiveParams::default()` is,
  // which is the frozen force set (docs/decisions/live-force-session.md).
  const session = exports.gm_force_session_create(handle, 0, 0);
  if (session === 0) refuse("gm_force_session_create", seed);
  const status = exports.gm_force_session_tick(session, ticks);
  if (status === 0) refuse("gm_force_session_tick", seed);
  if (status !== 1 && status !== 2) fail(`gm_force_session_tick answered ${status}`);
  const hash = createHash("sha256");
  hash.update(columnBytes(session, X_AXIS, seed));
  hash.update(columnBytes(session, Y_AXIS, seed));
  const digest = hash.digest("hex");
  if (exports.gm_force_session_release(session) !== 1) fail("gm_force_session_release refused");
  exports.gm_release(handle);
  return digest;
}

const lines = [];
for (let seed = 0; seed < seeds; seed += 1) {
  lines.push(`${STAGE} ${seed} ${digestFor(seed)}\n`);
}
process.stdout.write(lines.join(""));