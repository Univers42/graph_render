// The wasm32 arm of the Phase 9 campaign: the same motor, the same graph, compiled to
// WebAssembly and driven through the shipped JavaScript SDK.
//
//   node harness/wasm-tick-bench.mjs --n 220,10000,100000 --repeat 5
//   node harness/wasm-tick-bench.mjs --n ... --wasm <path> --out <md> --json <path>
//   node harness/wasm-tick-bench.mjs --self-check
//
// WHAT IS MEASURED, and why it is a derived number: the wasm ABI runs a layout to
// convergence in one call (`gm_run` → the registered `Stage::run`, which is 112 ticks),
// and the ABI has no per-tick entry point. So the tick column here is `run / 112` — the
// same per-tick-equivalent the native arm derives from its own one-shot run, so the two
// arms' crossover cells are computed the same way. It is not a measured single tick and
// the report says so wherever it appears.
//
// The graph is the same `buildSyntheticModel(n)` the oracle arm and the native arm lay
// out, written as the provisional ingest document `graph-wasm`'s `ingest.rs` reads
// (version 1, every member named, no camelCase). Module init is timed on its own: at
// small N boundary crossing and instantiation dominate, and a per-tick number that hid
// them would be the dishonest way to win the N = 220 row.
//
// It reads no environment variable. Exit codes follow graph-cli: 0 ran · 1 a check the
// self-check makes failed · 2 could not run (no wasm binary, no SDK, a module that did
// not load, a refused build, an ingest document the contract would refuse).
//
// The member lists this arm writes into that document are read from the contract
// (`harness/wasm-tick-bench/contract.mjs`), not restated beside the writer: a hand copy
// checked against itself is a transcription check in name only.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { dirname, resolve } from "node:path";

import { membersFromContract } from "./wasm-tick-bench/contract.mjs";
import { documentRefusal, ingestOf } from "./wasm-tick-bench/document.mjs";

/** Ticks a settle costs: d3's `alphaDecay(0.06)` down to its `alphaMin` of 0.001. */
const SETTLE_TICKS = 112;

/** `prompt.md` §5.2's frame budget, in milliseconds. */
const FRAME_BUDGET_MS = 16.67;

/** The layout whose headline number is a tick. */
const DEFAULT_LAYOUT = "layout.force.barnes_hut";

/** The pinned six-node model, asserted on this side of the port as on the other two. */
const PINNED_N = 6;
const PINNED_NODES = 6;
const PINNED_EDGES = 9;

const ROOT = resolve(import.meta.dirname, "..");
const DEFAULT_WASM = resolve(ROOT, "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

function fail(message) {
  process.stderr.write(`wasm-tick-bench: could not run: ${message}\n`);
  process.exit(2);
}

/** `--wasm`, `--layout`, `--n`, `--repeat`, `--budget-ms`, `--out`, `--json`, `--self-check`. */
function parseArgs(argv) {
  const plan = {
    wasm: DEFAULT_WASM,
    layout: DEFAULT_LAYOUT,
    sizes: [220, 10_000, 100_000],
    repeat: 5,
    budgetMs: FRAME_BUDGET_MS,
    out: null,
    json: null,
    selfCheck: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const [flag, inline] = argv[i].split("=");
    const value = () => inline ?? argv[(i += 1)];
    if (flag === "--self-check") plan.selfCheck = true;
    else if (flag === "--wasm") plan.wasm = resolve(value());
    else if (flag === "--layout") plan.layout = value();
    else if (flag === "--n") plan.sizes = value().split(",").map(Number);
    else if (flag === "--repeat") plan.repeat = Number(value());
    else if (flag === "--budget-ms") plan.budgetMs = Number(value());
    else if (flag === "--out") plan.out = value();
    else if (flag === "--json") plan.json = value();
    else fail(`unknown argument ${flag}`);
  }
  if (plan.sizes.some((n) => !Number.isInteger(n) || n < 2)) fail("--n takes integers of at least 2");
  if (!Number.isInteger(plan.repeat) || plan.repeat < 1) fail("--repeat takes a positive integer");
  return plan;
}

/** The median, the mean of the two central values when the count is even. */
function median(values) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

/** The largest `(n, ms)` that fits `budgetMs`; exactly at the budget still fits. */
function largestFitting(samples, budgetMs) {
  let largest = null;
  for (const [n, ms] of samples) {
    if (ms > budgetMs) continue;
    largest = largest === null || n > largest ? n : largest;
  }
  return largest;
}

/**
 * The engine's own `.ts` sources import each other without extensions (`./model`), which
 * plain ESM resolution rejects. This is the only import hook: a relative extensionless
 * specifier onto its `.ts` file. Nothing else is intercepted.
 */
function registerTypeScript() {
  registerHooks({
    resolve(specifier, context, next) {
      if (specifier.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(specifier)) {
        for (const extension of [".ts", "/index.ts"]) {
          try {
            return next(specifier + extension, context);
          } catch {
            // Not this one; try the next shape.
          }
        }
      }
      return next(specifier, context);
    },
  });
}

/** One `(n, build ms, run ms, tick ms)` row: `repeat` builds and `repeat` layouts. */
function measureArm(motor, document, n, repeat, layout) {
  const builds = [];
  const runs = [];
  let handle = 0;
  for (let run = 0; run < repeat; run += 1) {
    const startedBuild = performance.now();
    const built = motor.build(document);
    builds.push(performance.now() - startedBuild);
    if (handle !== 0) motor.release(handle);
    handle = built;
    const started = performance.now();
    motor.layout(handle, layout);
    runs.push(performance.now() - started);
  }
  const runMs = median(runs);
  const row = { n, nodes: n, build_ms: median(builds), run_ms: runMs, tick_ms: runMs / SETTLE_TICKS, settle_ms: runMs };
  motor.release(handle);
  return row;
}

/** The pinned model, the contract's member lists and the pinned arithmetic. */
async function selfCheck() {
  const failures = [];
  const check = (name, got, want) => {
    const same = JSON.stringify(got) === JSON.stringify(want);
    process.stdout.write(`${same ? "ok" : "not ok"} - ${name}\n`);
    if (!same) failures.push(`${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
  };
  check("median of an odd count is the middle value", median([3, 1, 2]), 2);
  check("median of an even count is the mean of the middle two", median([4, 1, 3, 2]), 2.5);
  check("the settle is 112 ticks", SETTLE_TICKS, 112);
  const ladder = [
    [220, 1],
    [10_000, 8],
    [100_000, 40],
  ];
  check("the crossover is the largest n that fits", largestFitting(ladder, 16.67), 10_000);
  check("no n fits when every one is over", largestFitting(ladder, 0.5), null);
  const members = membersFromContract(ROOT);
  const { buildSyntheticModel } = await import("../src/core/model/synthetic.ts");
  const model = buildSyntheticModel(PINNED_N);
  check("the pinned model's node count", model.nodes.length, PINNED_NODES);
  check("the pinned model's edge count", model.edges.length, PINNED_EDGES);
  const document = ingestOf(model);
  check("the ingest document's version", document.version, 1);
  check("a node record carries exactly the contract's members", Object.keys(document.nodes[0]), members.node);
  check("an edge record carries exactly the contract's members", Object.keys(document.edges[0]), members.edge);
  check("the ingest document is one gm_build would accept", documentRefusal(document, members), null);
  check("the ingest document round-trips its own counts", [document.nodes.length, document.edges.length], [PINNED_NODES, PINNED_EDGES]);
  process.stdout.write(failures.length === 0 ? "self-check ok\n" : `self-check FAILED: ${failures.join("; ")}\n`);
  process.exit(failures.length === 0 ? 0 : 1);
}

/** The arm's markdown, including the derived tick column and the init it does not hide. */
function markdown(plan, rows, initMs, runtime) {
  const lines = [
    "# Phase 9 — wasm32 arm (graph-core compiled to wasm32, driven through the JS SDK)",
    "",
    `node ${runtime} · ${plan.layout} · repeat ${plan.repeat} · module init ${initMs.toFixed(1)} ms (once, not per tick)`,
    "",
    "`tick ms` is `run ms / 112`: the ABI's `gm_run` settles the layout in one call, so a",
    "single tick is not observable from outside. It is the same per-tick-equivalent the",
    "native arm derives from its own one-shot run, so the two crossover cells are computed",
    "one way. It is not a measured single tick.",
    "",
    "| n | build ms | run ms (112 ticks) | tick ms (derived) |",
    "|---:|---:|---:|---:|",
  ];
  for (const row of rows) {
    lines.push(`| ${row.n} | ${row.build_ms.toFixed(2)} | ${row.run_ms.toFixed(3)} | ${row.tick_ms.toFixed(3)} |`);
  }
  lines.push("", `Largest n whose derived tick fits ${plan.budgetMs} ms: **${largestFitting(rows.map((r) => [r.n, r.tick_ms]), plan.budgetMs) ?? "none"}**`, "");
  return lines.join("\n");
}

/**
 * Why `motor` is not a motor this arm can time, or `null` when it is.
 * `Motor.create` never throws — it returns a degraded Motor carrying the load error
 * (`crates/graph-sdk-js/src/index.ts:60-71`), so an existence check alone let an
 * unloadable module through and the failure arrived later as an uncaught
 * `WasmUnavailableError` from `layouts()`: exit 1 with a stack, not the documented 2
 * naming the module. `#loadError` is private, so the message is read by making the one
 * call that rethrows it.
 */
function motorRefusal(motor, plan) {
  if (motor.available) return null;
  let why = "the SDK reported no reason";
  try {
    motor.layouts();
  } catch (error) {
    why = error && error.message ? error.message : String(error);
  }
  return `${plan.wasm} did not load, so no layout can be timed through it: ${why}`;
}

async function main() {
  const plan = parseArgs(process.argv.slice(2));
  registerTypeScript();
  if (plan.selfCheck) return selfCheck();
  if (!existsSync(plan.wasm)) fail(`${plan.wasm} does not exist (cargo build -p graph-wasm --target wasm32-unknown-unknown --release)`);
  const { createMotor } = await import("../crates/graph-sdk-js/src/index.ts");
  const { buildSyntheticModel } = await import("../src/core/model/synthetic.ts");
  const started = performance.now();
  const motor = await createMotor(readFileSync(plan.wasm));
  const initMs = performance.now() - started;
  const unusable = motorRefusal(motor, plan);
  if (unusable !== null) fail(unusable);
  if (!motor.layouts().includes(plan.layout)) fail(`the module does not register ${plan.layout}`);
  const members = membersFromContract(ROOT);
  const rows = [];
  for (const n of plan.sizes) {
    const document = ingestOf(buildSyntheticModel(n));
    const refusal = documentRefusal(document, members);
    if (refusal !== null) fail(`n = ${n}: ${refusal}`);
    rows.push(measureArm(motor, JSON.stringify(document), n, plan.repeat, plan.layout));
  }
  const text = markdown(plan, rows, initMs, process.version);
  process.stdout.write(text);
  if (plan.out) write(plan.out, text);
  if (plan.json) {
    write(plan.json, `${JSON.stringify({ arm: "wasm32", runtime: process.version, layout: plan.layout, budget_ms: plan.budgetMs, repeat: plan.repeat, settle_ticks: SETTLE_TICKS, init_ms: initMs, rows }, null, 2)}\n`);
  }
}

function write(path, text) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

// The header's own table promises 2 for "could not run"; a failed dynamic import or an
// unwritable `--out` is that, not "ran and failed".
try {
  await main();
} catch (error) {
  fail(error && error.message ? error.message : String(error));
}
