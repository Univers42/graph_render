// The TypeScript oracle arm of the Phase 9 campaign: the project's own baseline, timed.
//
//   node harness/oracle-tick-bench.mjs --n 220,10000,100000 --repeat 5
//        prints one row per n and the largest n whose median tick fits 16.67 ms.
//   node harness/oracle-tick-bench.mjs --n ... --out <md> --json <path>
//        also writes the markdown and a machine-readable row set, which is what
//        `graph-cli bench --crossover` reads to fill this arm's crossover cell.
//   node harness/oracle-tick-bench.mjs --self-check
//        runs the pinned arithmetic and the pinned six-node model, and exits non-zero on
//        the first disagreement. No d3, no graph: this mode is the one that can run
//        anywhere Node runs, which is why the native side's own test drives it.
//
// It drives `src/core/layout/forceLayout.ts:163` `tick()` headlessly: that class is
// DOM-free, its constructor `.stop()`s the simulation, and nothing here touches React,
// a canvas, a worker or a page. The graph is the same one the native arm lays out —
// `src/core/model/synthetic.ts`'s `buildSyntheticModel(n)`, which `graph-core`'s
// `synthetic.rs` is a call-for-call port of (pinned in both directions: six nodes and
// nine edges, asserted here and in `crates/graph-cli/src/bench/tests.rs`).
//
// WHAT IS AND IS NOT THE SAME ALGORITHM, stated because this is a timing arm, not a
// quality one: d3 integrates link and collide in place (Gauss-Seidel) where graph-core
// gathers them (Jacobi, devil C7), and d3 draws its coincident-point jiggle from one
// shared LCG where graph-core hashes (seed, tick, pass, i, j) (devil C8). The two settle
// to different pictures. What is shared is the tick's *work*: the same undirected edge
// set, the same frozen force set (`DEFAULT_LAYOUT_PARAMS`, cluster strength 0 because
// there are no node groups), the same golden-spiral seed at the origin, and 112 ticks —
// d3's `alphaDecay(0.06)` down to its `alphaMin` of 0.001, graph-core's `TICKS`.
//
// d3-force is resolved as ESM first, then through `createRequire`: ESM resolution
// ignores `NODE_PATH`, and the pinned tree can live outside the worktree. Where neither
// finds it this is exit 2 naming the module — an arm that cannot reach its baseline is a
// refusal, never a pass.
//
// It reads no environment variable. The only import hook is the TypeScript one: the
// engine's sources import each other without extensions (`./params`), which plain ESM
// resolution rejects, so `registerHooks` maps a relative extensionless specifier onto
// its `.ts` file. Nothing else is intercepted.

import { mkdirSync, writeFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { dirname, resolve } from "node:path";

/** Ticks a settle costs: d3's `alphaDecay(0.06)` down to its `alphaMin` of 0.001. */
const SETTLE_TICKS = 112;

/** `prompt.md` §5.2's frame budget, in milliseconds. */
const FRAME_BUDGET_MS = 16.67;

/** The alpha decay both arms run at; `ALPHA_MIN` is d3's own default. */
const ALPHA_DECAY = 0.06;
const ALPHA_MIN = 0.001;

/** The pinned six-node model: the graph both arms lay out, asserted in both arms. */
const PINNED_N = 6;
const PINNED_NODES = 6;
const PINNED_EDGES = 9;

const ROOT = resolve(import.meta.dirname, "..");

function fail(message) {
  process.stderr.write(`oracle-tick-bench: could not run: ${message}\n`);
  process.exit(2);
}

/** `--n`, `--repeat`, `--budget-ms`, `--out`, `--json`, `--self-check`. */
function parseArgs(argv) {
  const plan = { sizes: [220, 10_000, 100_000], repeat: 5, budgetMs: FRAME_BUDGET_MS, out: null, json: null, selfCheck: false };
  for (let i = 0; i < argv.length; i += 1) {
    const [flag, inline] = argv[i].split("=");
    const value = () => inline ?? argv[(i += 1)];
    if (flag === "--self-check") plan.selfCheck = true;
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
  const fitting = samples.filter(([, ms]) => ms <= budgetMs).map(([n]) => n);
  return fitting.length === 0 ? null : Math.max(...fitting);
}

/** The engine's own `.ts` sources import each other without extensions. */
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

/** d3-force, or the refusal that names it. */
async function loadD3Force() {
  try {
    return await import("d3-force");
  } catch (esm) {
    const { createRequire } = await import("node:module");
    try {
      return createRequire(import.meta.url)("d3-force");
    } catch (cjs) {
      fail(`d3-force is not resolvable (${esm.message}; via createRequire: ${cjs.message})`);
    }
  }
}

/**
 * The engine's model as `ForceLayout`'s links: the undirected, deduplicated,
 * self-loop-free edge set graph-core's `simple_graph` builds, oriented low-index first,
 * keeping the first strength seen in edge order.
 */
function linksOf(model) {
  const index = new Map(model.nodes.map((node, i) => [node.id, i]));
  const seen = new Set();
  const links = [];
  for (const edge of model.edges) {
    const a = index.get(edge.source);
    const b = index.get(edge.target);
    if (a === undefined || b === undefined || a === b) continue;
    const key = `${Math.min(a, b)}:${Math.max(a, b)}`;
    if (seen.has(key)) continue;
    seen.add(key);
    links.push({ source: Math.min(a, b), target: Math.max(a, b), strength: edge.strength });
  }
  return links;
}

/** One `(n, tick ms, settle ms, links)` row: `repeat` layouts of 112 timed ticks each. */
function measureArm(ForceLayout, params, model, n, repeat) {
  const links = linksOf(model);
  const ticks = [];
  let settle = 0;
  for (let run = 0; run < repeat; run += 1) {
    const layout = new ForceLayout({
      count: n,
      links,
      width: 0,
      height: 0,
      params,
    });
    for (let t = 0; t < SETTLE_TICKS; t += 1) {
      const started = performance.now();
      layout.tick();
      const ms = performance.now() - started;
      ticks.push(ms);
      settle += ms;
    }
  }
  return { n, nodes: n, links: links.length, tick_ms: median(ticks), settle_ms: settle / repeat };
}

/** The campaign's own arithmetic and the pinned model, with no d3 and no graph at 100k. */
async function selfCheck() {
  const failures = [];
  const check = (name, got, want) => {
    const same = Object.is(got, want);
    process.stdout.write(`${same ? "ok" : "not ok"} - ${name}\n`);
    if (!same) failures.push(`${name}: got ${got}, want ${want}`);
  };
  check("median of an odd count is the middle value", median([3, 1, 2]), 2);
  check("median of an even count is the mean of the middle two", median([4, 1, 3, 2]), 2.5);
  check("median of nothing is zero", median([]), 0);
  const ladder = [
    [220, 1],
    [10_000, 8],
    [100_000, 40],
  ];
  check("the crossover is the largest n that fits", largestFitting(ladder, 16.67), 10_000);
  check("exactly at the budget still fits", largestFitting(ladder, 8), 10_000);
  check("no n fits when every one is over", largestFitting(ladder, 0.5), null);
  check("an empty ladder has no crossover", largestFitting([], 16.67), null);
  check("the settle is 112 ticks", SETTLE_TICKS, 112);
  check("112 ticks is the first count below alphaMin", alphaAfter(SETTLE_TICKS) < ALPHA_MIN, true);
  check("111 ticks is not yet below alphaMin", alphaAfter(SETTLE_TICKS - 1) >= ALPHA_MIN, true);
  const { buildSyntheticModel } = await import("../src/core/model/synthetic.ts");
  const model = buildSyntheticModel(PINNED_N);
  check("the pinned model's node count", model.nodes.length, PINNED_NODES);
  check("the pinned model's edge count", model.edges.length, PINNED_EDGES);
  process.stdout.write(failures.length === 0 ? "self-check ok\n" : `self-check FAILED: ${failures.join("; ")}\n`);
  process.exit(failures.length === 0 ? 0 : 1);
}

/** `(1 - ALPHA_DECAY)^k`: where the simulation's alpha is after `k` ticks. */
function alphaAfter(k) {
  let alpha = 1;
  for (let i = 0; i < k; i += 1) alpha *= 1 - ALPHA_DECAY;
  return alpha;
}

/** The arm's markdown, losses included: the rows are what they are. */
function markdown(plan, rows, runtime) {
  const lines = [
    "# Phase 9 — TypeScript oracle arm (d3-force, this repo's own `tick()`)",
    "",
    `node ${runtime} · repeat ${plan.repeat} · ${SETTLE_TICKS} ticks per layout · median over every timed tick`,
    "",
    "Same graph, same frozen force set and same golden-spiral seed as the native arm; not the",
    "same algorithm (d3 integrates link/collide in place, graph-core gathers them), so this is a",
    "timing arm, not a quality one. Settle is the mean of the whole 112-tick layout.",
    "",
    "| n | links | tick ms (median) | settle ms |",
    "|---:|---:|---:|---:|",
  ];
  for (const row of rows) lines.push(`| ${row.n} | ${row.links} | ${row.tick_ms.toFixed(3)} | ${row.settle_ms.toFixed(1)} |`);
  lines.push("", `Largest n whose median tick fits ${plan.budgetMs} ms: **${largestFitting(rows.map((r) => [r.n, r.tick_ms]), plan.budgetMs) ?? "none"}**`, "");
  return lines.join("\n");
}

async function main() {
  const plan = parseArgs(process.argv.slice(2));
  registerTypeScript();
  if (plan.selfCheck) return selfCheck();
  const d3 = await loadD3Force();
  if (typeof d3.forceSimulation !== "function") fail("d3-force resolved to something that is not d3-force");
  const [{ ForceLayout }, { DEFAULT_LAYOUT_PARAMS }, { buildSyntheticModel }] = await Promise.all([
    import("../src/core/layout/forceLayout.ts"),
    import("../src/core/layout/params.ts"),
    import("../src/core/model/synthetic.ts"),
  ]);
  // Cluster strength 0: the frozen force set has no cluster force, and there are no
  // node groups here, so this states the comparison rather than leaving it to defaults.
  const params = { ...DEFAULT_LAYOUT_PARAMS, clusterStrength: 0 };
  const rows = [];
  for (const n of plan.sizes) rows.push(measureArm(ForceLayout, params, buildSyntheticModel(n), n, plan.repeat));
  const text = markdown(plan, rows, process.version);
  process.stdout.write(text);
  if (plan.out) write(plan.out, text);
  if (plan.json) {
    write(plan.json, `${JSON.stringify({ arm: "typescript-oracle", runtime: process.version, budget_ms: plan.budgetMs, repeat: plan.repeat, settle_ticks: SETTLE_TICKS, rows }, null, 2)}\n`);
  }
}

function write(path, text) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

await main();
