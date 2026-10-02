// The replicated wasm arm: one wasm instance per worker rank, each holding a full copy of
// the graph, all ranks stepping the same stage in lockstep. Inside one gathered pass a
// rank computes only its own node range, a blocking all-gather over a SharedArrayBuffer
// fills in every other rank's range, and every rank then applies the same full array — so
// every rank must end byte-identical. Model (b) alone is not enough: this has to prove the
// replicas agree with the serial build AND with each other, and to break on purpose.
//
//   node harness/wasm-replicas.mjs hash --seeds 8 --ranks 1,2,3,4,7 --layouts barnes_hut,particle_mesh
//   node harness/wasm-replicas.mjs bench --n 100000,1000000 --repeat 3 --ranks 1,2,4
//   node harness/wasm-replicas.mjs hash --ranks 2 --break        # negative control, exits 1
//
// hash mode prints one row per (layout, seed, ranks) cell, against the SERIAL artifact's own
// gm_run hash for the same (seed, layout) — the model harness/wasm-run.mjs hashes, so these
// rows compare with that arm's output. Hence hash mode ingests with gm_seed_ingest and never
// gm_seed_ingest_n; only bench mode uses --n. The two row shapes are
//   layout.force.barnes_hut seed 0 ranks 2 <sha256-hex> equal
//   layout.force.barnes_hut seed 0 ranks 2 <sha256-hex> MISMATCH(serial=<sha256-hex>)
// --break withholds one rank's contribution to the gather: a run that still agreed under it
// would prove the gather is never read, so the rows must read MISMATCH and the mode exits 1.
// Exit codes follow graph-cli: 0 every checked claim held · 1 a claim failed · 2 could not run.

import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { Worker } from "node:worker_threads";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const DEFAULT_WASM = resolve(ROOT, "target", "wasm-replicas", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");
const DEFAULT_SERIAL = resolve(ROOT, "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");
const WORKER_URL = new URL("./wasm-replicas/worker.mjs", import.meta.url);

/// Bytes per node the shared column SAB must hold, which is how the plan's model (b) sizes
/// it. Sized to spec rather than warned about: an undersized SAB refuses the gather instead
/// of withholding one rank's slice, which would make the negative control test the wrong thing.
const SAB_BYTES_PER_NODE = 64;

/// The ranks a run may name: 1 (the arm under test), 2 and 4 (even splits), 3 and 7 (ragged remainder).
const ALLOWED_RANKS = [1, 2, 3, 4, 7];

/// Node count of the model gm_seed_ingest(seed) builds, so a hash-mode cell sizes its SAB for the model ingested.
const seedNodeCount = (seed) => 2 + (seed % 600);

/// Workers still running: one rank's failure has to stop the rest, not leave the survivors parked forever.
const liveWorkers = new Set();

/// Checked claims that failed; any is exit 1. Counted, not exited on: the rest of the grid says how wrong.
let failures = 0;

/// Three kinds of failure because the exit code IS the report. An unhandled throw would say
/// 1 for a setup problem, the one answer this harness must never give by accident.
class CouldNotRun extends Error {}
class UsageError extends CouldNotRun {}
class CheckFailed extends Error {}

/// Throw rather than exit, so the one catch at the bottom is the only place that decides an exit code.
function fail(message) {
  throw new CouldNotRun(message);
}

/// Record a failed claim: the row still prints and the grid still finishes.
function checkFailed(message) {
  failures += 1;
  process.stderr.write(`wasm-replicas: FAIL — ${message}\n`);
}

const USAGE = `usage: wasm-replicas.mjs <hash|bench> [--wasm p] [--serial p] [--seeds N]
        [--ranks 1,2,3,4,7] [--layouts a,b] [--n 100000,...] [--repeat 3] [--break]
  hash  one row per (layout, seed, ranks): replica hash vs the serial build's hash
  bench median ms per (layout, n, ranks), with speedup vs ranks=1`;

/// An integer in range. A bad number is could-not-run, not a failed claim: a silent NaN would size a SAB from nothing.
function intArg(text, flag, min, max) {
  if (!/^[0-9]+$/.test(text)) fail(`${flag} takes an integer, got ${text}`);
  const value = Number(text);
  if (value < min || value > max) fail(`${flag} takes ${min}..${max}, got ${text}`);
  return value;
}

/// A short layout name is expanded once, here, so no later code knows which end of the registry it was written at.
const forceLayoutName = (short) => (short.startsWith("layout.") ? short : `layout.force.${short}`);

function parseArgs(argv) {
  const plan = {
    mode: null, wasm: DEFAULT_WASM, serial: DEFAULT_SERIAL, seeds: 8, ranks: [...ALLOWED_RANKS],
    layouts: ["barnes_hut", "particle_mesh"], sizes: [100000], repeat: 3, breakLastRank: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (!flag.startsWith("--")) {
      if (plan.mode !== null) throw new UsageError(`two modes given (${plan.mode}, ${flag})`);
      if (flag !== "hash" && flag !== "bench") throw new UsageError(`unknown mode ${flag}`);
      plan.mode = flag;
      continue;
    }
    const value = () => {
      if (i + 1 >= argv.length) throw new UsageError(`${flag} needs a value`);
      return argv[(i += 1)];
    };
    switch (flag) {
      case "--wasm": plan.wasm = resolve(value()); break;
      case "--serial": plan.serial = resolve(value()); break;
      case "--seeds": plan.seeds = intArg(value(), flag, 1, 0xffffffff); break;
      case "--ranks": plan.ranks = [...new Set(value().split(",").map((v) => intArg(v, flag, 1, 7)))]
        .filter((r) => ALLOWED_RANKS.includes(r)); break;
      case "--layouts": plan.layouts = value().split(",").map(forceLayoutName); break;
      case "--n": plan.sizes = value().split(",").map((v) => intArg(v, flag, 1, 0xffffffff)); break;
      case "--repeat": plan.repeat = intArg(value(), flag, 1, 1000); break;
      case "--break": plan.breakLastRank = true; break;
      default: throw new UsageError(`unknown flag ${flag}`);
    }
  }
  if (plan.mode === null) throw new UsageError("no mode given: hash or bench");
  if (plan.ranks.length === 0) throw new UsageError(`--ranks takes only ${ALLOWED_RANKS.join(",")}`);
  return plan;
}

/// Read an artifact whole, refusing a missing one before any worker is spawned: a typo in
/// --wasm must read as could-not-run on the path, not as a worker that died.
async function readArtifact(path, flag) {
  try {
    return await readFile(path);
  } catch {
    return fail(`${flag}: cannot read ${path}`);
  }
}

// ------------------------------------------------------------------ serial reference arm
let serialExports = null;
let serialLayouts = null;
const serialHashes = new Map();

/// The serial artifact is the ordinary build and must import nothing: one that imported the
/// host all-gather hook would be waiting on a host that never answers.
async function openSerial(path) {
  const module = await WebAssembly.compile(await readArtifact(path, "--serial"));
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) fail(`serial artifact imports ${imports.map((i) => i.name).join(", ")}`);
  ({ exports: serialExports } = await WebAssembly.instantiate(module, {}));
}

function serialFramed(ptr) {
  if (ptr === 0) fail(`serial export returned 0 (gm_last_error ${serialExports.gm_last_error()})`);
  const len = new DataView(serialExports.memory.buffer).getUint32(ptr, true);
  return new Uint8Array(serialExports.memory.buffer, ptr + 4, len).slice();
}

/// name -> registry index from the serial module's own registry (C1), never an index frozen here.
function serialLayoutIndex(name) {
  if (serialLayouts === null) {
    const decode = new TextDecoder("utf-8", { fatal: true });
    serialLayouts = new Map();
    const total = serialExports.gm_layout_count();
    for (let i = 0; i < total; i += 1) serialLayouts.set(decode.decode(serialFramed(serialExports.gm_layout_id(i))), i);
  }
  const index = serialLayouts.get(name);
  if (index === undefined) fail(`serial artifact has no registered layout named ${name}`);
  return index;
}

/// The reference digest for one (seed, layout): the serial build's own hash of the same model,
/// cached because the grid runs each cell once per rank count.
function serialHash(seed, layout) {
  const key = `${seed}|${layout}`;
  if (serialHashes.has(key)) return serialHashes.get(key);
  const index = serialLayoutIndex(layout);
  const ingest = serialFramed(serialExports.gm_seed_ingest(seed));
  const ptr = serialExports.gm_alloc(ingest.length);
  if (ptr === 0) fail(`serial gm_alloc refused ${ingest.length} bytes (seed ${seed})`);
  new Uint8Array(serialExports.memory.buffer, ptr, ingest.length).set(ingest);
  const handle = serialExports.gm_build(ptr, ingest.length);
  serialExports.gm_free(ptr, ingest.length);
  if (handle === 0) fail(`serial gm_build refused (seed ${seed}, gm_last_error ${serialExports.gm_last_error()})`);
  const ok = serialExports.gm_run(handle, index, 0, 0);
  if (ok !== 1) fail(`serial gm_run refused (seed ${seed}, ${layout}, gm_last_error ${serialExports.gm_last_error()})`);
  const digest = createHash("sha256").update(serialFramed(serialExports.gm_snapshot_bytes(handle))).digest("hex");
  serialExports.gm_release(handle);
  serialHashes.set(key, digest);
  return digest;
}

// --------------------------------------------------------------------- the replicated arm
/// Stop every rank still running, called when one fails: the survivors are parked in the
/// barrier the failed rank will never reach, so leaving them alive hangs the mode.
function stopAllWorkers() {
  const workers = [...liveWorkers];
  liveWorkers.clear();
  workers.forEach((worker) => worker.terminate().catch(() => {}));
}

/// `job` bundles the per-rank half of the workerData contract so the spawn signature stays in the parameter limit.
function spawnRank(controlSab, dataSab, job) {
  const worker = new Worker(WORKER_URL, { workerData: { ...job, serial: false, controlSab, dataSab } });
  liveWorkers.add(worker);
  return worker;
}

/// A worker's whole life: its one report, or the reason there is none. A refusal arrives as a
/// message and is a failed claim; silence is could-not-run.
function collectReport(worker) {
  return new Promise((resolveReport, rejectReport) => {
    let settled = false;
    const settle = (finish, value) => {
      if (settled) return;
      settled = true;
      liveWorkers.delete(worker);
      finish(value);
    };
    worker.on("message", (msg) => {
      if (msg.error === undefined) settle(resolveReport, msg);
      else { stopAllWorkers(); settle(rejectReport, new CheckFailed(msg.error)); }
    });
    worker.on("error", (e) => { stopAllWorkers(); settle(rejectReport, new CouldNotRun(`rank worker: ${e.message}`)); });
    worker.on("exit", (code) => { stopAllWorkers(); settle(rejectReport, new CouldNotRun(`rank worker exited (code ${code}) silently`)); });
  });
}

/// Spawn one worker per rank over SABs sized for this cell. Fresh SABs per call: a reused
/// control generation is a barrier that can trip early, a reused data SAB carries the last
/// cell's column into this one.
async function runRanks(testCase, ranks, brk) {
  const controlSab = new SharedArrayBuffer(8);
  const dataSab = new SharedArrayBuffer(SAB_BYTES_PER_NODE * testCase.n);
  const jobs = Array.from({ length: ranks }, (_, rank) => ({
    wasmPath: testCase.wasmPath, layoutId: testCase.layout, ingestSeed: testCase.seed,
    ingestN: testCase.n, useN: testCase.useN, rank, ranks, breakLastRank: brk,
  }));
  return Promise.all(jobs.map((job) => collectReport(spawnRank(controlSab, dataSab, job))));
}

/// The slowest rank is the cell's number: a replicated run is finished when its last rank is,
/// so an average over ranks would flatter every rank count above one.
const slowestOf = (reports) => reports.reduce((slowest, report) => (report.ms > slowest.ms ? report : slowest));

// ---------------------------------------------------------------------------- hash mode
async function hashMode(plan) {
  await openSerial(plan.serial);
  const lines = [];
  let cells = 0;
  let peak = { ms: 0, gatherMs: 0, ranks: 1 };
  for (const layout of plan.layouts) {
    for (let seed = 0; seed < plan.seeds; seed += 1) {
      for (const ranks of plan.ranks) {
        const testCase = { wasmPath: plan.wasm, layout, seed, n: seedNodeCount(seed), useN: false };
        const reports = await runRanks(testCase, ranks, plan.breakLastRank);
        const slowest = slowestOf(reports);
        const rankZero = reports.find((report) => report.rank === 0);
        const reference = serialHash(seed, layout);
        const equal = rankZero.hash === reference;
        cells += 1;
        if (slowest.ms > peak.ms) peak = { ms: slowest.ms, gatherMs: slowest.gatherMs, ranks };
        lines.push(`${layout} seed ${seed} ranks ${ranks} ${rankZero.hash} ${equal ? "equal" : `MISMATCH(serial=${reference})`}\n`);
        // --break asserts the opposite, so an agreement there is the failure, not a pass.
        if (equal === plan.breakLastRank) {
          checkFailed(`${layout} seed ${seed} ranks ${ranks}: rank 0 ${equal ? "equals" : "differs from"} the serial hash ${reference}`);
        }
        // Stronger than the serial comparison: the ranks must agree with EACH OTHER, which the
        // serial hash alone would not catch if the serial model moved with them.
        if (!plan.breakLastRank && new Set(reports.map((report) => report.hash)).size !== 1) {
          checkFailed(`${layout} seed ${seed} ranks ${ranks}: the ranks disagree with each other`);
        }
      }
    }
  }
  process.stdout.write(lines.join(""));
  process.stderr.write(`wasm-replicas: hash: ${cells} cells, ${failures} failed, slowest cell ${peak.ms.toFixed(2)} ms`
    + ` (${peak.gatherMs.toFixed(2)} ms gathering, ranks ${peak.ranks})\n`);
}

// --------------------------------------------------------------------------- bench mode
async function loadavg() {
  try {
    return (await readFile("/proc/loadavg", "utf8")).split("\n")[0].trim();
  } catch {
    return "unavailable";
  }
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

/// One bench cell: `repeat` spawns of `ranks` ranks, the slowest rank of each spawn, then the
/// median. Median, not mean: a repeat that shared the machine is a real observation and should
/// not by itself move the cell.
async function benchCell(testCase, ranks, repeat) {
  const slowest = [];
  let last = [];
  for (let i = 0; i < repeat; i += 1) {
    last = await runRanks(testCase, ranks, false);
    slowest.push(slowestOf(last).ms);
  }
  const bytes = last.reduce((sum, report) => sum + report.memBytes, 0);
  return { ms: median(slowest), hash: last.find((report) => report.rank === 0).hash, memMiB: bytes / (1024 * 1024) };
}

async function benchMode(plan) {
  const loadStart = await loadavg();
  const lines = ["| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB |\n", "| --- | --- | --- | --- | --- | --- | --- |\n"];
  const ran = [];
  for (const layout of plan.layouts) {
    for (const n of plan.sizes) {
      const testCase = { wasmPath: plan.wasm, layout, seed: 0, n, useN: true };
      // The ranks-1 cell is measured FIRST whatever its position in the list: it is the speedup
      // baseline, and measuring it once keeps it off an already-warm machine.
      const baseline = plan.ranks.includes(1) ? await benchCell(testCase, 1, plan.repeat) : null;
      for (const ranks of plan.ranks) {
        const cell = ranks === 1 ? baseline : await benchCell(testCase, ranks, plan.repeat);
        // "-" rather than a fabricated 1.00 with no baseline requested: a number nobody
        // measured would read as one they did.
        const speedup = baseline === null ? "-" : (ranks === 1 ? "1.00" : (baseline.ms / cell.ms).toFixed(2));
        const equal = baseline === null ? "-" : (cell.hash === baseline.hash ? "yes" : "no");
        lines.push(`| ${layout} | ${n} | ${ranks} | ${cell.ms.toFixed(2)} | ${speedup} | ${equal} | ${cell.memMiB.toFixed(1)} |\n`);
      }
      ran.push(`${layout} n=${n}`);
    }
  }
  lines.push(`\nloadavg start: ${loadStart}\nloadavg end: ${await loadavg()}\n`, `cells ran: ${ran.join("; ")}\n`);
  if (!plan.ranks.includes(1)) lines.push("no ranks-1 cell requested, so the speedup column is unmeasured\n");
  process.stdout.write(lines.join(""));
}

// ---------------------------------------------------------------------------------- main
async function main() {
  try {
    const plan = parseArgs(process.argv.slice(2));
    await readArtifact(plan.wasm, "--wasm");
    if (plan.mode === "hash") await hashMode(plan);
    else await benchMode(plan);
    process.exitCode = failures === 0 ? 0 : 1;
  } catch (error) {
    stopAllWorkers();
    // Every thrown path is could-not-run: no JS stack, and never exit 1 by accident.
    const code = error instanceof CheckFailed ? 1 : 2;
    process.stderr.write(`${code === 1 ? "wasm-replicas: FAIL — " : "wasm-replicas: could not run: "}${error.message}\n`);
    if (error instanceof UsageError) process.stderr.write(`${USAGE}\n`);
    process.exitCode = code;
  }
}

await main();