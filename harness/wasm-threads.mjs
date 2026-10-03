// Browser threads model (a) under Node (`prompts/perf-plan.md` P3): the threads artifact
// (`scripts/orch/wasm-threads.sh`) over one shared WebAssembly.Memory, instantiated here as the
// coordinator and once per helper in a worker_thread (`harness/wasm-threads-helper.mjs`).
// Node lets its main thread block, as the browser's motor Worker may; a page's main thread may not.
//
//   hash  [--seeds 8] [--n 1000,20000] [--workers 1,2,3,4,7] [--layouts barnes_hut,particle_mesh] [--break]
//     Gate seeds: gm_run_threaded on gm_seed_ingest(seed)'s model against the default artifact's
//     gm_run, the hash gate's own wasm arm. --n: gm_seed_handle(1, n) against the same module's
//     serial gm_run. --break sets flag bit 0 (the last part writes nothing), so cells must differ.
//   session [--seeds 8] [--n 20000] [--workers 1,2,3,4,7] [--layouts …] [--ticks 30] [--break]
//     The live session, both engines: --ticks ticks of gm_force_session_tick_threaded in calls of
//     10, then x, y and alpha as bytes, against gm_force_session_tick on the same graph (the
//     default artifact for gate seeds, this module for --n). --break as for hash.
//   tick  [--n 400000,1000000] [--workers 1,2,4,8] [--layouts particle_mesh] [--ticks 5]
//     A fresh session per cell: the median ms of one threaded tick over --ticks, after a warm-up.
//   bench [--n 100000,1000000] [--workers 1,2,4,8] [--layouts particle_mesh,barnes_hut] [--repeat 3]
//     The whole stage per cell, median of --repeat, as a markdown table with /proc/loadavg.
//
// Paths: --wasm (threads artifact), --serial (default artifact). Exit codes as graph-cli:
// 0 ran clean, 1 a check failed (any differing cell), 2 could not run.

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { Worker } from "node:worker_threads";

const STACK = 4 << 20;
const INITIAL_PAGES = 1024; // --initial-memory=67108864 in scripts/orch/wasm-threads.sh
const MAX_PAGES = 65536;

function fail(message) {
  process.stderr.write(`wasm-threads: ${message}\n`);
  process.exit(2);
}

function parseArgs(argv) {
  const [mode, ...rest] = argv;
  const opts = { mode, break: false };
  for (let i = 0; i < rest.length; i++) {
    const key = rest[i];
    if (key === "--break") opts.break = true;
    else if (key.startsWith("--") && i + 1 < rest.length) opts[key.slice(2)] = rest[++i];
    else fail(`unknown argument ${key}`);
  }
  if (!["hash", "session", "tick", "bench"].includes(mode)) fail("usage: wasm-threads.mjs hash|session|tick|bench [options]");
  return opts;
}

const list = (text) => (text ? text.split(",").filter(Boolean) : []);
const numbers = (text) => list(text).map(Number);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const loadavg = () => readFileSync("/proc/loadavg", "utf8").split(" ").slice(0, 3).join(" ");

/// The id -> registry index map of a module, scanned once through gm_layout_id (C1).
function registry(x, framed) {
  const ids = new Map();
  for (let i = 0; i < x.gm_layout_count(); i++) {
    ids.set(new TextDecoder().decode(framed(x.gm_layout_id(i))), i);
  }
  return (layout) => ids.get(`layout.force.${layout}`) ?? fail(`no layout.force.${layout}`);
}

/// One module's ABI surface: its exports, its memory, its framed reader and its registry.
function surface(x, memory) {
  const framed = (ptr) => {
    if (ptr === 0) fail(`export returned 0 (gm_last_error ${x.gm_last_error()})`);
    const len = new DataView(memory.buffer).getUint32(ptr, true);
    return new Uint8Array(memory.buffer, ptr + 4, len).slice();
  };
  return { x, memory, framed, index: registry(x, framed) };
}

/// gm_seed_ingest -> gm_alloc -> gm_build on `m`: the gate seed's model on a fresh handle.
function seedHandle(m, seed) {
  const text = m.framed(m.x.gm_seed_ingest(seed));
  const ptr = m.x.gm_alloc(text.length);
  if (ptr === 0) fail(`gm_alloc refused ${text.length} bytes`);
  new Uint8Array(m.memory.buffer, ptr, text.length).set(text);
  const handle = m.x.gm_build(ptr, text.length);
  m.x.gm_free(ptr, text.length);
  return handle || fail(`gm_build refused seed ${seed} (gm_last_error ${m.x.gm_last_error()})`);
}

/// Runs `run(handle)` (which must return 1) and hashes the snapshot it leaves.
function snapshotHash(m, handle, run) {
  if (run(handle) !== 1) fail(`run refused (gm_last_error ${m.x.gm_last_error()})`);
  return sha256(m.framed(m.x.gm_snapshot_bytes(handle)));
}

async function loadThreads(path) {
  const module = await WebAssembly.compile(await readFile(path));
  const memory = new WebAssembly.Memory({ initial: INITIAL_PAGES, maximum: MAX_PAGES, shared: true });
  const instance = await WebAssembly.instantiate(module, { env: { memory } });
  return { module, m: surface(instance.exports, memory) };
}

async function loadSerial(path) {
  const { instance } = await WebAssembly.instantiate(await readFile(path), {});
  return surface(instance.exports, instance.exports.memory);
}

/// Starts `count` helpers and returns once every one has joined the pool.
async function startHelpers(module, m, count) {
  const { x } = m;
  if (x.__tls_align.value > 16) fail(`TLS alignment ${x.__tls_align.value} exceeds gm_thread_block's 16`);
  const tlsSize = Math.max(x.__tls_size.value, 16);
  const helpers = [];
  for (let i = 0; i < count; i++) {
    const stack = x.gm_thread_block(STACK);
    const tls = x.gm_thread_block(tlsSize);
    if (stack === 0 || tls === 0) fail("gm_thread_block refused a helper's stack or TLS");
    const workerData = { module, memory: m.memory, stackTop: (stack + STACK) & ~15, tls };
    const helper = new Worker(new URL("./wasm-threads-helper.mjs", import.meta.url), { workerData });
    helper.on("error", (error) => fail(`helper ${i}: ${error.message}`));
    helpers.push(new Promise((resolve) => helper.on("exit", resolve)));
  }
  while (x.gm_thread_count() < count) await new Promise((resolve) => setTimeout(resolve, 2));
  return helpers;
}

/// Every threaded cell against its serial reference: the gate seeds against the default
/// artifact, --n against this module's own serial path. `arm` is what one cell runs.
function differential(opts, m, serial, arm) {
  const flags = opts.break ? 1 : 0;
  const workers = numbers(opts.workers ?? "1,2,3,4,7");
  let differing = 0;
  const cell = (label, kind, reference, handle) => {
    for (const w of workers) {
      const same = arm.threaded(m, handle, kind, { workers: w, flags }) === reference;
      if (!same) differing++;
      console.log(`${label} ${kind} workers ${w}: ${same ? "equal" : "DIFFERS"}`);
    }
  };
  for (let seed = 0; seed < Number(opts.seeds ?? 8); seed++) {
    for (const kind of arm.kinds) {
      const ref = seedHandle(serial, seed);
      const reference = arm.serial(serial, ref, kind);
      serial.x.gm_release(ref);
      const handle = seedHandle(m, seed);
      cell(`seed ${seed}`, kind, reference, handle);
      m.x.gm_release(handle);
    }
  }
  for (const n of numbers(opts.n)) {
    for (const kind of arm.kinds) {
      const handle = m.x.gm_seed_handle(1, n) || fail(`gm_seed_handle refused n ${n}`);
      cell(`n ${n}`, kind, arm.serial(m, handle, kind), handle);
      m.x.gm_release(handle);
    }
  }
  console.log(`${differing} differing cell(s)${opts.break ? " (--break: expected > 0)" : ""}`);
  return differing === 0 ? 0 : 1;
}

/// The whole layout: a snapshot hash of gm_run_threaded against gm_run.
const layoutArm = (opts) => ({
  kinds: list(opts.layouts ?? "barnes_hut,particle_mesh"),
  serial: (mod, handle, layout) => snapshotHash(mod, handle, (h) => mod.x.gm_run(h, mod.index(layout), 0, 0)),
  threaded: (mod, handle, layout, { workers, flags }) =>
    snapshotHash(mod, handle, (h) => mod.x.gm_run_threaded(h, mod.index(layout), workers, flags)),
});

const CREATE = { barnes_hut: "gm_force_session_create", particle_mesh: "gm_force_session_create_mesh" };

function createSession(mod, handle, engine) {
  const create = mod.x[CREATE[engine] ?? fail(`no session engine ${engine}`)];
  return create(handle, 0, 0) || fail(`${engine} session refused (gm_last_error ${mod.x.gm_last_error()})`);
}

/// A fresh `engine` session over `handle`, `ticks` ticks through `tick(session, count)` in calls
/// of 10, then x, y and alpha hashed. Columns are read after the last call: a mesh tick moves them.
function sessionHash(mod, handle, engine, { ticks, tick }) {
  const session = createSession(mod, handle, engine);
  for (let done = 0; done < ticks; done += 10) {
    if (tick(session, Math.min(10, ticks - done)) === 0) fail(`tick refused (gm_last_error ${mod.x.gm_last_error()})`);
  }
  const hash = createHash("sha256");
  for (const axis of [0, 1]) {
    const ptr = mod.x.gm_force_session_column_ptr(session, axis);
    const len = mod.x.gm_force_session_column_len(session, axis);
    hash.update(new Uint8Array(mod.memory.buffer, ptr, len * 8).slice());
  }
  hash.update(new Float64Array([mod.x.gm_force_session_alpha(session)]));
  mod.x.gm_force_session_release(session);
  return hash.digest("hex");
}

const sessionArm = (opts) => {
  const ticks = Number(opts.ticks ?? 30);
  return {
    kinds: list(opts.layouts ?? "barnes_hut,particle_mesh"),
    serial: (mod, handle, engine) => sessionHash(mod, handle, engine, { ticks, tick: mod.x.gm_force_session_tick }),
    threaded: (mod, handle, engine, { workers, flags }) =>
      sessionHash(mod, handle, engine, { ticks, tick: (s, k) => mod.x.gm_force_session_tick_threaded(s, k, workers, flags) }),
  };
};

/// Milliseconds of each of `ticks` single threaded ticks on a fresh session, after one warm-up.
function timeTicks(m, { handle, engine }, workers, ticks) {
  const session = createSession(m, handle, engine);
  m.x.gm_force_session_tick_threaded(session, 1, workers, 0);
  const times = [];
  for (let t = 0; t < ticks; t++) {
    const start = performance.now();
    if (m.x.gm_force_session_tick_threaded(session, 1, workers, 0) === 0) fail(`tick refused (gm_last_error ${m.x.gm_last_error()})`);
    times.push(performance.now() - start);
  }
  m.x.gm_force_session_release(session);
  return times;
}

function tickMode(opts, m) {
  const workers = numbers(opts.workers ?? "1,2,4,8");
  console.log(`load start ${loadavg()}`);
  console.log("| engine | n | workers | median ms/tick | min ms | max ms | speed-up |\n|---|---|---|---|---|---|---|");
  for (const engine of list(opts.layouts ?? "particle_mesh")) {
    for (const n of numbers(opts.n ?? "400000,1000000")) {
      const handle = m.x.gm_seed_handle(1, n) || fail(`gm_seed_handle refused n ${n}`);
      let base;
      for (const w of workers) {
        const times = timeTicks(m, { handle, engine }, w, Number(opts.ticks ?? 5));
        const ms = median(times);
        base ??= ms;
        const row = [ms, Math.min(...times), Math.max(...times)].map((v) => v.toFixed(1));
        console.log(`| ${engine} | ${n} | ${w} | ${row.join(" | ")} | ${(base / ms).toFixed(2)}× |`);
      }
      m.x.gm_release(handle);
    }
  }
  console.log(`load end ${loadavg()}`);
  return 0;
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function benchCell(m, handle, layout, workers, repeat) {
  const times = [];
  let hash = "";
  for (let r = 0; r < repeat; r++) {
    const start = performance.now();
    hash = snapshotHash(m, handle, (h) => m.x.gm_run_threaded(h, m.index(layout), workers, 0));
    times.push(performance.now() - start);
  }
  return { ms: median(times), min: Math.min(...times), max: Math.max(...times), hash };
}

function benchMode(opts, m) {
  const workers = numbers(opts.workers ?? "1,2,4,8");
  const repeat = Number(opts.repeat ?? 3);
  let differing = 0;
  console.log(`load start ${loadavg()}`);
  console.log("| layout | n | workers | median ms | min ms | max ms | speed-up | equal | memory MiB |");
  console.log("|---|---|---|---|---|---|---|---|---|");
  for (const layout of list(opts.layouts ?? "particle_mesh,barnes_hut")) {
    for (const n of numbers(opts.n ?? "100000,1000000")) {
      const handle = m.x.gm_seed_handle(1, n) || fail(`gm_seed_handle refused n ${n}`);
      let base;
      for (const w of workers) {
        const cell = benchCell(m, handle, layout, w, repeat);
        base ??= cell;
        if (cell.hash !== base.hash) differing++;
        const mib = (m.memory.buffer.byteLength / 2 ** 20).toFixed(0);
        const row = [layout, n, w, cell.ms, cell.min, cell.max].map((v) => (typeof v === "number" && !Number.isInteger(v) ? v.toFixed(1) : v));
        console.log(`| ${row.join(" | ")} | ${(base.ms / cell.ms).toFixed(2)}× | ${cell.hash === base.hash} | ${mib} |`);
      }
      m.x.gm_release(handle);
    }
  }
  console.log(`load end ${loadavg()} · repeat ${repeat}`);
  return differing === 0 ? 0 : 1;
}

const opts = parseArgs(process.argv.slice(2));
const { module, m } = await loadThreads(opts.wasm ?? "target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm");
const compares = ["hash", "session"].includes(opts.mode);
const workers = numbers(opts.workers ?? (compares ? "1,2,3,4,7" : "1,2,4,8"));
const helpers = await startHelpers(module, m, Math.max(...workers) - 1);
const serialPath = opts.serial ?? "target/wasm32-unknown-unknown/release/graph_wasm.wasm";
const arm = opts.mode === "hash" ? layoutArm : sessionArm;
const code = compares
  ? differential(opts, m, await loadSerial(serialPath), arm(opts))
  : (opts.mode === "tick" ? tickMode : benchMode)(opts, m);
m.x.gm_thread_close();
await Promise.all(helpers);
process.exit(code);
