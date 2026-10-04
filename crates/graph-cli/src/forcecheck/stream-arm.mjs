// The wasm32 arm of the force gate's **stream** stage, under Node: the same real
// `graph_wasm.wasm` the browser loads, driven through the two growth exports end to end and
// nothing else. Plain `WebAssembly.instantiate`, no wasm-bindgen and no generated glue, and
// the module must import nothing — a self-contained arm tests the shipped binary.
//
//   node stream-arm.mjs <graph_wasm.wasm> <ticks> <fixture.jsonl>...
//
// Prints `<stage> <fixture> <batch> <sha256>` per batch, the order `force-gate`'s stream
// comparator reads. The fixture name is the file's stem, so a line names the file a reader
// can open.
//
// The path is the real ABI per batch, and every step is one a studio takes:
//   gm_alloc -> gm_graph_extend -> gm_free -> gm_force_session_grow -> gm_force_session_tick
//   -> gm_force_session_column_ptr/_len -> the two f64 columns
// so a stage the module cannot export fails this arm loudly (exit 2) rather than quietly
// dropping out of a comparison, and a refusal is reported with the module's own last error.
//
// It reads no environment variable. The negative control (GM_MUTATE_DROP_DELTA) drops a
// batch on the native arm only, so a wired mutation has to surface as a divergence.

import { readFile } from "node:fs/promises";
import { basename } from "node:path";
import { createHash } from "node:crypto";

function fail(message) {
  process.stderr.write(`force-stream-arm: ${message}\n`);
  process.exit(2);
}

const [wasmPath, ticksArg, ...fixtures] = process.argv.slice(2);
if (!wasmPath || !ticksArg || fixtures.length === 0) {
  fail("usage: stream-arm.mjs <wasm> <ticks> <fixture paths...>");
}

// The stage id is `force-gate`'s stream stage, written here because the two are different
// languages; `force-gate`'s own unit test refuses to let the pair drift further than this
// one string.
const STAGE = "force.session.stream";
const X_AXIS = 0;
const Y_AXIS = 1;

if (!/^[0-9]+$/.test(ticksArg)) fail(`bad tick count ${ticksArg}`);
const ticks = Number.parseInt(ticksArg, 10);

const wasmBytes = await readFile(wasmPath);
const module = await WebAssembly.compile(wasmBytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) fail(`module imports ${imports.map((i) => i.name).join(", ")}`);
const { exports } = await WebAssembly.instantiate(module, {});

// The two exports this stage exists to test, checked by name at load: a module that predates
// them would otherwise fail at the first call with a `TypeError`, which says nothing about
// which export is missing.
for (const name of [
  "gm_graph_extend",
  "gm_force_session_grow",
  "gm_force_session_create",
  "gm_force_session_tick",
  "gm_force_session_column_ptr",
  "gm_force_session_release",
]) {
  if (typeof exports[name] !== "function") {
    fail(`no ${name} export: the module predates the growth ABI`);
  }
}

function refuse(what, fixture, batch) {
  fail(
    `${what} refused (${fixture} batch ${batch}, gm_last_error ${exports.gm_last_error()})`,
  );
}

/// One column of the session's own storage, as the bytes the native arm hashed: the `f64`s in
/// row order, little-endian, exactly as a `Vec<f64>` holds them.
function columnBytes(session, axis, fixture, batch) {
  const address = exports.gm_force_session_column_ptr(session, axis);
  const len = exports.gm_force_session_column_len(session, axis);
  if (address === 0 || len === 0) refuse(`gm_force_session_column(${axis})`, fixture, batch);
  const view = new Float64Array(exports.memory.buffer, address, len);
  // The three-argument form, because `view.buffer` is the whole `WebAssembly.Memory` —
  // hashing that would hash every byte of the module instead of the column.
  return Buffer.from(view.buffer, view.byteOffset, view.byteLength);
}

/// `x` then `y`, hashed, as the native arm does.
function digest(session, fixture, batch) {
  const hash = createHash("sha256");
  hash.update(columnBytes(session, X_AXIS, fixture, batch));
  hash.update(columnBytes(session, Y_AXIS, fixture, batch));
  return hash.digest("hex");
}

/// Hands `bytes` to the module through `gm_alloc`, calls `body(ptr, len)`, frees, and returns
/// whatever `body` answered. One helper so every batch's copy-never-free dance is written once.
function withAlloc(len, what, fixture, batch, body) {
  const ptr = exports.gm_alloc(len);
  if (ptr === 0) refuse(`gm_alloc(${len})`, fixture, batch);
  new Uint8Array(exports.memory.buffer, ptr, len).set(bytes);
  const answer = body(ptr, len);
  exports.gm_free(ptr, len);
  return answer;
}

/// The fixture's documents, one per non-empty line, in order.
async function documents(path) {
  const raw = await readFile(path);
  const out = [];
  for (const line of raw.toString("utf8").split("\n")) {
    if (line.length > 0) out.push(Buffer.from(line, "utf8"));
  }
  if (out.length === 0) fail(`${path}: no lines`);
  return out;
}

/// The stage's lines for one fixture: build line 0, then extend and grow per batch.
async function linesFor(path) {
  const fixture = basename(path, ".jsonl");
  const docs = await documents(path);
  const handle = withAlloc(docs[0].length, "gm_alloc", fixture, 0, (ptr, len) =>
    exports.gm_build(ptr, len),
  );
  if (handle === 0) refuse("gm_build", fixture, 0);
  // `params_len == 0` is the compiled-in defaults: the same set `LiveParams::default()` is.
  const session = exports.gm_force_session_create(handle, 0, 0);
  if (session === 0) refuse("gm_force_session_create", fixture, 0);

  const out = [`${STAGE} ${fixture} 0 `];
  for (let batch = 0; batch < docs.length; batch += 1) {
    if (batch > 0) {
      const grown = withAlloc(docs[batch].length, "gm_alloc", fixture, batch, (ptr, len) =>
        exports.gm_graph_extend(handle, ptr, len),
      );
      if (grown !== 1) refuse("gm_graph_extend", fixture, batch);
      if (exports.gm_force_session_grow(session, handle) !== 1) {
        refuse("gm_force_session_grow", fixture, batch);
      }
    }
    const status = exports.gm_force_session_tick(session, ticks);
    if (status === 0) refuse("gm_force_session_tick", fixture, batch);
    if (status !== 1 && status !== 2) fail(`gm_force_session_tick answered ${status}`);
    out[batch] = `${out[batch]}${digest(session, fixture, batch)}\n`;
  }

  if (exports.gm_force_session_release(session) !== 1) fail("gm_force_session_release refused");
  exports.gm_release(handle);
  return out.join("");
}

const lines = [];
for (const path of fixtures) {
  lines.push(await linesFor(path));
}
process.stdout.write(lines.join(""));