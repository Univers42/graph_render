// The replicated wasm arm: one wasm instance per worker rank, each holding a full copy of the
// graph, all ranks stepping the same stage in lockstep. Inside one gathered pass a rank computes
// only its own node range, a blocking all-gather over a SharedArrayBuffer fills in every other
// rank's range, and every rank then applies the same full array — so every rank must end
// byte-identical. Model (b) alone is not enough: this has to prove the replicas agree with the
// serial build AND with each other, and to break on purpose.
//
//   node harness/wasm-replicas.mjs hash --seeds 8 --ranks 1,2,3,4,7 --layouts barnes_hut,particle_mesh
//   node harness/wasm-replicas.mjs bench --n 100000,400000 --repeat 3 --ranks 1,2,4,8
//   node harness/wasm-replicas.mjs hash --ranks 2 --break        # negative control, exits 1
//
// hash mode prints one row per (layout, seed, ranks) cell, against the SERIAL artifact's own gm_run
// hash for the same (seed, layout) — the model harness/wasm-run.mjs hashes, so these rows compare
// with that arm's output. Hence hash mode ingests with gm_seed_ingest and never gm_seed_ingest_n;
// only bench mode uses --n. The two row shapes are
//   layout.force.barnes_hut seed 0 ranks 2 <sha256-hex> equal
//   layout.force.barnes_hut seed 0 ranks 2 <sha256-hex> MISMATCH(serial=<sha256-hex>)
// --break withholds one rank's contribution to the gather. Nothing here knows that flag changed the
// expected answer: the same comparison runs, every cell then reads MISMATCH, and MISMATCH is a
// failed claim, so the mode exits 1 — the control asserting the gather is read.
// Exit codes follow graph-cli: 0 every checked claim held · 1 a claim failed · 2 could not run.

import { readFile } from "node:fs/promises";
import { Worker } from "node:worker_threads";
import { resolve } from "node:path";
import { openSerial, serialHash } from "./wasm-replicas/worker.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const DEFAULT_WASM = resolve(ROOT, "target", "wasm-replicas", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");
const DEFAULT_SERIAL = resolve(ROOT, "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");
const WORKER_URL = new URL("./wasm-replicas/worker.mjs", import.meta.url);

/// Bytes per node the shared column SAB must hold, which is how the plan's model (b) sizes it. Sized
/// to spec, not warned about: an undersized SAB refuses the gather rather than withholding one rank's
/// slice, which would make the negative control test the wrong thing.
const SAB_BYTES_PER_NODE = 64;

/// The ranks a run may name: 1, 2, 3, 4 and 7 are the hash gate's own worker counts (1 the arm, 2 and 4 even splits, 3 and 7 ragged); 8 is the bench's widest.
const ALLOWED_RANKS = [1, 2, 3, 4, 7, 8];

/// Node count of the model gm_seed_ingest(seed) builds, so a hash cell sizes its SAB to fit it.
const seedNodeCount = (seed) => 2 + (seed % 600);

/// Workers still running: one rank's failure has to stop the rest, not leave survivors parked.
const liveWorkers = new Set();

/// Checked claims that failed; any is exit 1. Counted, not exited on: the grid says how wrong.
let failures = 0;

/// Three kinds of failure because the exit code IS the report: an unhandled throw would say 1 for a
/// setup problem, the one answer this harness must never give by accident.
class CouldNotRun extends Error {}
class UsageError extends CouldNotRun {}
class CheckFailed extends Error {}

/// Throw rather than exit, so the one catch at the bottom is the only place that decides a code.
const fail = (message) => {
  throw new CouldNotRun(message);
};

/// Record a failed claim: the row still prints and the grid still finishes.
function checkFailed(message) {
  failures += 1;
  process.stderr.write(`wasm-replicas: FAIL — ${message}\n`);
}

const USAGE = `usage: wasm-replicas.mjs <hash|bench> [--wasm p] [--serial p] [--seeds N]
        [--ranks 1,2,3,4,7] [--layouts a,b] [--n 100000,...] [--repeat 3] [--break]
  hash  one row per (layout, seed, ranks): replica hash vs the serial build's hash
  bench median ms per (layout, n, ranks): speedup vs ranks=1, the gather's share, mem`;

/// An integer in range. A bad number is could-not-run, not a failed claim: a silent NaN would
/// size a SAB from nothing.
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
    layouts: ["barnes_hut", "particle_mesh"].map(forceLayoutName), sizes: [100000], repeat: 3, breakLastRank: false,
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
      case "--ranks": plan.ranks = [...new Set(value().split(",").map((v) => intArg(v, flag, 1, 8)))]
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

/// Read an artifact whole, refusing a missing one before any worker is spawned: a typo must read on the path.
async function readArtifact(path, flag) {
  try {
    return await readFile(path);
  } catch {
    return fail(`${flag}: cannot read ${path}`);
  }
}

// --------------------------------------------------------------------- the replicated arm
/// Stop every rank still running, called when one fails: the survivors are parked in a barrier it never reaches.
function stopAllWorkers() {
  const workers = [...liveWorkers];
  liveWorkers.clear();
  workers.forEach((worker) => worker.terminate().catch(() => {}));
}

/// `job` bundles the per-rank half of the workerData contract, keeping the spawn in the parameter limit.
function spawnRank(controlSab, dataSab, job) {
  const worker = new Worker(WORKER_URL, { workerData: { ...job, serial: false, controlSab, dataSab } });
  liveWorkers.add(worker);
  return worker;
}

/// A worker's whole life: its one report, or the reason there is none — a refusal is a failed claim,
/// silence is could-not-run.
function collectReport(worker) {
  return new Promise((resolveReport, rejectReport) => {
    let settled = false;
    const settle = (finish, value) => {
      if (settled) return;
      settled = true;
      liveWorkers.delete(worker);
      finish(value);
    };
    /// A rank that stops reporting takes the others down with it — they are parked in a barrier it
    /// never reaches. Guarded on `settled`: a reported rank's exit is the normal end of a rank, and
    /// tearing the still-reporting ranks down then would invent a failure from the fastest finishing.
    const died = (error) => {
      if (settled) return;
      stopAllWorkers();
      settle(rejectReport, error);
    };
    worker.on("message", (msg) => {
      if (msg.error === undefined) settle(resolveReport, msg);
      // `setup` is the rank saying "you asked for something that does not exist", which is
      // could-not-run; anything else it refused is a claim this run failed to establish.
      else died(msg.setup === true ? new CouldNotRun(msg.error) : new CheckFailed(msg.error));
    });
    worker.on("error", (e) => died(new CouldNotRun(`rank worker: ${e.message}`)));
    worker.on("exit", (code) => died(new CouldNotRun(`rank worker exited (code ${code}) silently`)));
  });
}

/// Spawn one worker per rank over SABs sized for this cell. Fresh SABs per call: a reused control
/// generation is a barrier that can trip early, a reused data SAB carries the last cell's column in.
async function runRanks(testCase, ranks, brk) {
  const controlSab = new SharedArrayBuffer(8);
  const dataSab = new SharedArrayBuffer(SAB_BYTES_PER_NODE * testCase.n);
  const jobs = Array.from({ length: ranks }, (_, rank) => ({
    wasmPath: testCase.wasmPath, layoutId: testCase.layout, ingestSeed: testCase.seed,
    ingestN: testCase.n, useN: testCase.useN, rank, ranks, breakLastRank: brk,
  }));
  return Promise.all(jobs.map((job) => collectReport(spawnRank(controlSab, dataSab, job))));
}

/// The slowest rank is the cell's number: a run is finished when its last rank is, so an average
/// over ranks would flatter every rank count above one.
const slowestOf = (reports) => reports.reduce((s, report) => (report.ms > s.ms ? report : s));

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
        // One rule, both modes: rank 0 must equal the serial build. Under --break it does not, every
        // cell fails, and the mode exits 1 — the control asserting the gather is read at all by making
        // the ordinary check go red.
        if (!equal) checkFailed(`${layout} seed ${seed} ranks ${ranks}: rank 0 differs from the serial hash ${reference}`);
        // Stronger than the serial comparison, and it holds under --break too: every rank reads back
        // the same shared column, so a withheld rank makes them all wrong the same way rather than
        // making them disagree. What --break must break is the serial comparison, not this one.
        if (new Set(reports.map((report) => report.hash)).size !== 1) {
          checkFailed(`${layout} seed ${seed} ranks ${ranks}: the ranks disagree with each other`);
        }
      }
    }
  }
  process.stdout.write(lines.join(""));
  process.stderr.write(`wasm-replicas: hash: ${cells} cells, ${failures} failed, slowest cell ${peak.ms.toFixed(2)} ms`
    + ` (${peak.gatherMs.toFixed(2)} ms gathering, ranks ${peak.ranks})\n`);
  process.stderr.write(`wasm-replicas: hash: --break ${plan.breakLastRank ? "ON" : "off"}, so ${failures === 0 ? "the run agrees" : "the run disagrees"}\n`);
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

/// One bench cell: `repeat` spawns of `ranks` ranks, the slowest rank of each spawn, then the median.
/// Median, not mean: a repeat that shared the machine should not by itself move the cell. The gather
/// share comes off that slowest rank, the gather being where the extra ranks spend their time.
async function benchCell(testCase, ranks, repeat) {
  const slowest = [];
  let last = [];
  for (let i = 0; i < repeat; i += 1) {
    last = await runRanks(testCase, ranks, false);
    slowest.push(slowestOf(last));
  }
  const mid = median(slowest);
  const bytes = last.reduce((sum, report) => sum + report.memBytes, 0);
  const gather = median(slowest.map((report) => report.gatherMs));
  return { ms: mid.ms, gatherPct: (100 * gather) / mid.ms, hash: last.find((r) => r.rank === 0).hash, memMiB: bytes / (1024 * 1024) };
}

async function benchMode(plan) {
  const loadStart = await loadavg();
  const lines = ["| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB | gather % |\n", "| --- | --- | --- | --- | --- | --- | --- | --- |\n"];
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
        lines.push(`| ${layout} | ${n} | ${ranks} | ${cell.ms.toFixed(2)} | ${speedup} | ${equal} | ${cell.memMiB.toFixed(1)} | ${cell.gatherPct.toFixed(1)} |\n`);
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
    // Every thrown path is could-not-run: no JS stack, never exit 1 by accident.
    const code = error instanceof CheckFailed ? 1 : 2;
    process.stderr.write(`${code === 1 ? "wasm-replicas: FAIL — " : "wasm-replicas: could not run: "}${error.message}\n`);
    if (error instanceof UsageError) process.stderr.write(`${USAGE}\n`);
    process.exitCode = code;
  }
}

await main();