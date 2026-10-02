// One rank of a replicated wasm run — model (b) of the browser-thread plan: every rank
// owns a private wasm instance holding a full copy of the graph, all ranks step the same
// stage in lockstep, and one gathered pass per rank is assembled from every rank's slice.
//
// The ABI here is the replicas artifact (`graph-wasm --features replicas`, built to
// target/wasm-replicas): the ordinary exports plus `gm_seed_ingest_n` and
// `gm_run_replica`, and exactly one import — `env.gm_host_allgather`, below.
//
//   workerData: { wasmPath, serial, rank, ranks, ingestSeed, ingestN, useN,
//                 layoutId, controlSab, dataSab, breakLastRank }
//   answers with: { rank, ms, gatherMs, hash, memBytes }   or   { rank, error }
//
// `ms` times `gm_run_replica` alone — the whole replicated pass, gather included, because
// that is the span a browser rank pays. `gatherMs` is the share of it spent in the
// blocking all-gather (both barriers and the copy back), which is what tells a flat
// speedup column apart from a real one.
//
// `memory.buffer` DETACHES whenever the motor grows its memory, so nothing here holds a
// view across a motor call: `framed` and `gm_host_allgather` both re-read the buffer.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { parentPort, workerData } from "node:worker_threads";
import { performance } from "node:perf_hooks";

const {
  wasmPath, serial, rank, ranks, ingestSeed, ingestN, useN, layoutId, controlSab, dataSab, breakLastRank,
} = workerData;

/// Typed views over the two SharedArrayBuffers the main thread owns. `ctl[0]` counts
/// arrivals and `ctl[1]` is the sense/generation; `data` is the shared f64 column every
/// rank publishes its own span into and reads the whole column back out of.
const ctl = new Int32Array(controlSab);
const data = new Uint8Array(dataSab);

/// Live module exports plus the gather accumulator. A holder rather than a closure over
/// `exports` because the import is installed BEFORE the module exists — WebAssembly's
/// import object is read during instantiation, so the binding has to resolve lazily.
const host = { exports: null, gatherMs: 0 };

/// Sense-reversing barrier, one Int32Array(2) shared by every rank: the last arrival
/// resets the counter, bumps the generation and wakes the rest. The wait is a loop with a
/// 50 ms timeout because a timed wait can return spuriously; it leaves the thread parked
/// in `Atomics.wait` between wakeups rather than spinning on the generation.
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
/// gather is correct — without it the next pass would race this one's readers.
///
/// One array parameter, not five: the wasm signature fixes five arguments and the house
/// limit is four, so the five arrive as one destructured tuple.
function allgather([ptr, elemBytes, len, lo, hi]) {
  const { exports } = host;
  const total = len * elemBytes;
  // Refuse before touching anything: a column that only some ranks filled is worse than a
  // refusal, because every rank would then read a different half-built column.
  if (total > data.byteLength) return 1;
  const withheld = breakLastRank && rank === ranks - 1;
  if (!withheld) {
    const own = new Uint8Array(exports.memory.buffer, ptr, (hi - lo) * elemBytes);
    data.set(own, lo * elemBytes);
  }
  const started = performance.now();
  barrier();
  new Uint8Array(exports.memory.buffer, ptr, total).set(data.subarray(0, total));
  barrier();
  host.gatherMs += performance.now() - started;
  return 0;
}

/// `[len: u32 LE][len bytes]` out of wasm memory, copied out whole: the buffer is re-read
/// here so a growth since the caller's last motor call cannot hand back a detached view.
/// A 0 pointer is the motor refusing, and it publishes why in `gm_last_error`.
function framed(ptr) {
  const { exports } = host;
  if (ptr === 0) throw new Error(`export returned 0: refused (gm_last_error ${exports.gm_last_error()})`);
  const len = new DataView(exports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

/// `gm_run_replica`'s `layout_id` is a registry INDEX, so the name is resolved against the
/// replicas module's own registry rather than assumed — a layout registered after this file
/// was written stays reachable, and an unregistered name is refused instead of running as
/// whatever happens to sit at that index.
function resolveLayoutIndex(exports, name) {
  const decode = new TextDecoder("utf-8", { fatal: true });
  const total = exports.gm_layout_count();
  for (let i = 0; i < total; i += 1) {
    if (decode.decode(framed(exports.gm_layout_id(i))) === name) return i;
  }
  throw new Error(`no registered layout named ${name}`);
}

/// The framed ingest document -> a private handle. `gm_alloc` -> copy -> `gm_build` ->
/// `gm_free` is the one way a model enters a module, and every rank runs it over the same
/// bytes, which is what makes the ranks' copies comparable at all.
function buildHandle(exports, doc) {
  const ptr = exports.gm_alloc(doc.length);
  if (ptr === 0) throw new Error(`gm_alloc refused ${doc.length} bytes`);
  new Uint8Array(exports.memory.buffer, ptr, doc.length).set(doc);
  const handle = exports.gm_build(ptr, doc.length);
  exports.gm_free(ptr, doc.length);
  if (handle === 0) throw new Error(`gm_build refused (gm_last_error ${exports.gm_last_error()})`);
  return handle;
}

/// The whole rank, in order: compile, instantiate with the one import, ingest, build,
/// rendezvous, run (timed alone), hash, report. The rendezvous before the timed region is
/// load balancing, not protocol — without it rank 0 would pay for every other rank's
/// ingest and the ms column would be measuring who spawned first.
async function runRank() {
  if (serial !== false) throw new Error("this driver only ever drives the replicas artifact");
  const bytes = await readFile(wasmPath);
  const module = await WebAssembly.compile(bytes);
  const { exports } = await WebAssembly.instantiate(module, { env: { gm_host_allgather: allgather } });
  host.exports = exports;
  if (typeof exports.gm_run_replica !== "function") {
    throw new Error("no gm_run_replica export: build graph-wasm with --features replicas");
  }
  const layoutIndex = resolveLayoutIndex(exports, layoutId);
  const doc = useN ? framed(exports.gm_seed_ingest_n(ingestSeed, ingestN)) : framed(exports.gm_seed_ingest(ingestSeed));
  const handle = buildHandle(exports, doc);
  barrier();
  const started = performance.now();
  const ok = exports.gm_run_replica(handle, layoutIndex, rank, ranks);
  const finished = performance.now();
  if (ok !== 1) {
    throw new Error(`gm_run_replica refused (layout ${layoutId}, gm_last_error ${exports.gm_last_error()})`);
  }
  const hash = createHash("sha256").update(framed(exports.gm_snapshot_bytes(handle))).digest("hex");
  parentPort.postMessage({ rank, ms: finished - started, gatherMs: host.gatherMs, hash, memBytes: exports.memory.buffer.byteLength });
}

/// A refusal or a throw is reported, not thrown at the thread: the parent reads one
/// message either way, and a rank that dies silently is a harness bug rather than a
/// result, so it must never look like either.
try {
  await runRank();
  parentPort.close();
} catch (error) {
  parentPort.postMessage({ rank, error: error instanceof Error ? error.message : String(error) });
}