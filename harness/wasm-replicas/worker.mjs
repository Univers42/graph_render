// Both halves of the replicas harness's host side, in one module because they are one
// subject: what a host owes a wasm motor that either gathers over a SharedArrayBuffer or
// does not gather at all.
//
//   as a WORKER ENTRY (the default when parentPort is set) — one rank of a replicated run,
//   model (b) of the browser-thread plan: every rank owns a private wasm instance holding a
//   full copy of the graph, all ranks step the same stage in lockstep, and one gathered pass
//   per rank is assembled from every rank's slice. The artifact is the replicas build
//   (`graph-wasm --features replicas`, into target/wasm-replicas): the ordinary exports plus
//   `gm_seed_ingest_n` and `gm_run_replica`, and exactly one import — `env.gm_host_allgather`.
//
//     workerData: { wasmPath, serial, rank, ranks, ingestSeed, ingestN, useN,
//                   layoutId, controlSab, dataSab, breakLastRank }
//     answers with: { rank, ms, gatherMs, hash, memBytes }   or   { rank, error }
//
//     `ms` times `gm_run_replica` alone — the whole replicated pass, gather included,
//     because that is the span a browser rank pays. `gatherMs` is the share of it spent in
//     the blocking all-gather (both barriers and the copy back), which is what tells a flat
//     speedup column apart from a real one.
//
//   as an IMPORTED MODULE — `openSerial` / `serialHash`, the serial reference arm: the
//   ordinary build of the same crate, run in THIS process with no workers at all, which is
//   the arm every replica hash is compared against. It lives here so both halves read one
//   file's notion of how to instantiate a graph-wasm module and frame its returns.
//
// `memory.buffer` DETACHES whenever the motor grows its memory, so nothing here holds a view
// across a motor call: `framed` and `gm_host_allgather` both re-read the buffer.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { parentPort, workerData } from "node:worker_threads";
import { performance } from "node:perf_hooks";

/// False when this module is imported by the main process rather than spawned: the driver
/// at the bottom has no thread to report to, and the serial arm has no workerData to read.
const inWorker = parentPort !== null;
const { wasmPath, rank, ranks, ingestSeed, ingestN, useN, layoutId, controlSab, dataSab, breakLastRank } =
  inWorker ? workerData : {};

/// Typed views over the two SharedArrayBuffers the main thread owns. `ctl[0]` counts arrivals
/// and `ctl[1]` is the sense/generation; `data` is the shared f64 column every rank publishes
/// its own span into and reads the whole column back out of. Empty off-thread, where the
/// barrier and the gather are never reached.
const ctl = inWorker ? new Int32Array(controlSab) : new Int32Array(0);
const data = inWorker ? new Uint8Array(dataSab) : new Uint8Array(0);

/// Live module exports plus the gather accumulator. A holder rather than a closure over
/// `exports` because the import is installed BEFORE the module exists — WebAssembly's import
/// object is read during instantiation, so the binding has to resolve lazily.
const host = { exports: null, gatherMs: 0 };

/// Sense-reversing barrier, one Int32Array(2) shared by every rank: the last arrival resets
/// the counter, bumps the generation and wakes the rest. The wait is a loop with a 50 ms
/// timeout because a timed wait can return spuriously; it leaves the thread parked in
/// `Atomics.wait` between wakeups rather than spinning on the generation.
function barrier() {
  const gen = Atomics.load(ctl, 1);
  const arrived = Atomics.add(ctl, 0, 1) + 1;
  if (arrived === ranks) {
    Atomics.store(ctl, 0, 0);
    Atomics.add(ctl, 1, 1);
    Atomics.notify(ctl, 1);
  } else {
    while (Atomics.load(ctl, 1) === gen) Atomics.wait(ctl, 1, gen, 50);
  }
}

/// The replicas ABI's single host import, in the order the motor requires: publish this
/// rank's span, barrier, overwrite wasm memory with the whole column, barrier again so no
/// rank republishes before every rank has read. The last barrier is the whole reason the
/// gather is correct — without it the next pass would race this one's readers, and every
/// instance would apply a different column.
///
/// One rest parameter, not five named ones: the house limit is four parameters and this import
/// is fixed at five by the wasm signature. A host function is called with its arguments
/// positionally, never as a single array, so they are spread here and unpacked below.
function allgather(...args) {
  // A wasm i32 reaches JS signed: an address past 2 GiB would be negative without `>>> 0` (C9).
  const [ptr, elemBytes, len, lo, hi] = args.map((arg) => arg >>> 0);
  const { exports } = host;
  const total = len * elemBytes;
  // Refuse before touching anything: a column only some ranks filled is worse than a refusal,
  // because every instance would then read a different half-built column.
  if (total > data.byteLength) return 1;
  if (!(breakLastRank && rank === ranks - 1)) {
    // `ptr` is the base of the WHOLE column, so this rank's own span starts at
    // `ptr + lo * elemBytes` — reading from `ptr` would republish whichever outputs happen
    // to sit at the front of the buffer, and every rank above one would then fill the
    // column with the same leading elements and zeros behind them.
    const own = new Uint8Array(exports.memory.buffer, ptr + lo * elemBytes, (hi - lo) * elemBytes);
    data.set(own, lo * elemBytes);
  }
  const started = performance.now();
  barrier();
  new Uint8Array(exports.memory.buffer, ptr, total).set(data.subarray(0, total));
  barrier();
  host.gatherMs += performance.now() - started;
  return 0;
}

/// `[len: u32 LE][len bytes]` out of the module's wasm memory, copied out whole: the buffer is
/// re-read here so a growth since the caller's last motor call cannot hand back a detached
/// view. A 0 pointer is the motor refusing, and it publishes why in `gm_last_error`.
function framed(exports, result) {
  const ptr = result >>> 0;
  if (ptr === 0) throw new Error(`export returned 0: refused (gm_last_error ${exports.gm_last_error()})`);
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

/// A misconfigured run rather than a wrong result: a bad `--layouts` name, or an artifact
/// that is not the replicas build. Reported as could-not-run (exit 2), never as a failed
/// claim — "the layout you asked for does not exist" is not evidence about any bytes.
class SetupError extends Error {}

/// `gm_run_replica`'s `layout_id` is a registry INDEX, so the name is resolved against the
/// module's own registry rather than assumed — a layout registered after this file was
/// written stays reachable, and an unregistered name is refused instead of running as
/// whatever happens to sit at that index.
function layoutIndex(exports, name) {
  const decode = new TextDecoder("utf-8", { fatal: true });
  const total = exports.gm_layout_count();
  for (let i = 0; i < total; i += 1) {
    if (decode.decode(framed(exports, exports.gm_layout_id(i))) === name) return i;
  }
  throw new SetupError(`no registered layout named ${name}`);
}

/// The framed ingest document -> a private handle. `gm_alloc` -> copy -> `gm_build` ->
/// `gm_free` is the one way a model enters a module, and every rank runs it over the same
/// bytes, which is what makes the ranks' copies comparable at all.
function buildHandle(exports, doc) {
  const ptr = exports.gm_alloc(doc.length) >>> 0;
  if (ptr === 0) throw new Error(`gm_alloc refused ${doc.length} bytes`);
  new Uint8Array(exports.memory.buffer, ptr, doc.length).set(doc);
  const handle = exports.gm_build(ptr, doc.length);
  exports.gm_free(ptr, doc.length);
  if (handle === 0) throw new Error(`gm_build refused (gm_last_error ${exports.gm_last_error()})`);
  return handle;
}

/// The whole rank, in order: compile, instantiate with the one import, ingest, build,
/// rendezvous, run (timed alone), hash, report. The rendezvous before the timed region is load
/// balancing, not protocol — without it rank 0 would pay for every other rank's ingest and the
/// ms column would be measuring who spawned first.
async function runRank() {
  const bytes = await readFile(wasmPath);
  const { exports } = await WebAssembly.instantiate(await WebAssembly.compile(bytes), {
    env: { gm_host_allgather: allgather },
  });
  host.exports = exports;
  if (typeof exports.gm_run_replica !== "function") {
    throw new SetupError("no gm_run_replica export: build graph-wasm with --features replicas");
  }
  const index = layoutIndex(exports, layoutId);
  const doc = useN
    ? framed(exports, exports.gm_seed_ingest_n(ingestSeed, ingestN))
    : framed(exports, exports.gm_seed_ingest(ingestSeed));
  const handle = buildHandle(exports, doc);
  barrier();
  const started = performance.now();
  const ok = exports.gm_run_replica(handle, index, rank, ranks);
  const finished = performance.now();
  if (ok !== 1) throw new Error(`gm_run_replica refused (layout ${layoutId}, gm_last_error ${exports.gm_last_error()})`);
  const hash = createHash("sha256").update(framed(exports, exports.gm_snapshot_bytes(handle))).digest("hex");
  return { rank, ms: finished - started, gatherMs: host.gatherMs, hash, memBytes: exports.memory.buffer.byteLength };
}

// ---------------------------------------------------------- the serial reference arm
// The ordinary build, in this process, with no workers and no gather: the arm every replica
// hash is compared against. It must import NOTHING — a serial artifact that imported the
// host all-gather would be waiting on a host that never answers, so the check is here rather
// than left to the reader to remember.
let serial = null;
const serialLayouts = new Map();
const serialHashes = new Map();

/// Instantiate the serial artifact once. Any refusal is a setup problem, so it throws as a
/// plain Error and the caller's own exit-code rule turns it into "could not run" (2).
export async function openSerial(path) {
  const module = await WebAssembly.compile(await readFile(path));
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) throw new Error(`serial artifact imports ${imports.map((i) => i.name).join(", ")}`);
  serial = (await WebAssembly.instantiate(module, {})).exports;
}

/// name -> registry index from the serial module's own registry (C1), never an index frozen here.
function serialLayoutIndex(name) {
  if (serialLayouts.size === 0) {
    const decode = new TextDecoder("utf-8", { fatal: true });
    for (let i = 0; i < serial.gm_layout_count(); i += 1) {
      serialLayouts.set(decode.decode(framed(serial, serial.gm_layout_id(i))), i);
    }
  }
  const index = serialLayouts.get(name);
  if (index === undefined) throw new Error(`serial artifact has no registered layout named ${name}`);
  return index;
}

/// The reference digest for one (seed, layout): the serial build's own hash of the same model,
/// cached because the grid runs each cell once per rank count. `gm_release` before the digest
/// is returned, so a long grid is not holding one handle per (seed, layout) alive.
export function serialHash(seed, layout) {
  const key = `${seed}|${layout}`;
  if (serialHashes.has(key)) return serialHashes.get(key);
  const ingest = framed(serial, serial.gm_seed_ingest(seed));
  const handle = buildHandle(serial, ingest);
  if (serial.gm_run(handle, serialLayoutIndex(layout), 0, 0) !== 1) {
    throw new Error(`serial gm_run refused (seed ${seed}, ${layout}, gm_last_error ${serial.gm_last_error()})`);
  }
  const digest = createHash("sha256").update(framed(serial, serial.gm_snapshot_bytes(handle))).digest("hex");
  serial.gm_release(handle);
  serialHashes.set(key, digest);
  return digest;
}

// A refusal or a throw is reported, not thrown at the thread: the parent reads one message
// either way, and a rank that dies silently is a harness bug rather than a result, so it must
// never look like either.
if (inWorker) {
  try {
    parentPort.postMessage(await runRank());
    parentPort.close();
  } catch (error) {
    const setup = error instanceof SetupError;
    parentPort.postMessage({ rank, error: error instanceof Error ? error.message : String(error), setup });
  }
}
