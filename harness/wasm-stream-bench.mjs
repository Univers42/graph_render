// The wasm32 arm of the P4 delta measurement: the same stream file the native arm replays
// (`graph-cli tick --stream <BATCH> --from <path.jsonl>`), driven through the shipped
// JavaScript SDK instead of through the Rust bench.
//
//   node harness/wasm-stream-bench.mjs --from target/bench/s.jsonl --engine barnes_hut
//   node harness/wasm-stream-bench.mjs --from ... --engine particle_mesh --path columns
//
// WHAT IS MEASURED, per batch, exactly as the native arm measures it: `extend` and `grow` in
// two timers, neither containing the other, with `tick(1)` after each batch and `tick(10)`
// after line 0 both outside them. `JSON.parse` of a line is untimed, the way reading the line
// off disk is untimed natively. One asymmetry stays inside `extend` on both paths: the SDK's own
// encoding, `JSON.stringify` under `--path json` and `encodeBatch` under `--path columns`. The
// native arm hands `service::extend` the bytes it read and `service::extend_columns` the bytes a
// Rust encoder wrote, both untimed, so this arm is the only one that pays a host's encoder —
// which is the point of A1 in `docs/decisions/extend-columns.md`, and why the `columns` columns
// are not "the native columns minus the walk".
//
// The structure snapshot the studio rebuilds is timed once at the end: `run(handle,
// "layout.random")` and `toBytes(handle)` at the final size.
//
// `--split` is a fourth mode and is valid only with `--path columns`, because `encodeBatch` is
// the only thing in the JS half this arm can put in a timer of its own: each batch then runs it
// once, under its own timer, discards the bytes, and only then runs the unchanged timed
// `extendColumns`, so `extend - encode` is the copy into linear memory plus the wasm32 motor.
// An `encode` column is printed beside `extend` and `grow`. The default output is unchanged.
//
// It reads no environment variable. Exit codes follow graph-cli: 0 ran · 2 could not run (no
// wasm binary, no SDK, a module that did not load, a stream with no first line, a line the
// ingest reader refuses, a layout the module does not register, `--split` without
// `--path columns`).
//
// Caveat: a p95 over ten batches is one interpolated value, not a tail, on both sides of the
// comparison. `quantile` here is the same `R7` rule as `stream/stats.rs`, because the two
// arms' columns are comparable only if the three statistics are computed one way.

import { splitTables, timeEncode } from "./stream-split.mjs";
import { createReadStream, existsSync, readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { resolve } from "node:path";
import { createInterface } from "node:readline";

const ROOT = resolve(import.meta.dirname, "..");
const DEFAULT_WASM = resolve(ROOT, "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

/** Ticks after line 0, untimed, matching the native arm's `WARM_TICKS`. */
const WARM_TICKS = 10;

/** The stage the studio rebuilds from, timed once at the final size. */
const SNAPSHOT_LAYOUT = "layout.random";

/** The same headers the native arm prints, so the columns line up row for row. */
const BATCH_HEADER = "| batch | nodes after | extend ms | grow ms | sum ms |\n|---:|---:|---:|---:|---:";
const SUMMARY_HEADER = "| layout | batch | n | extend median ms | grow median ms | sum median ms | sum p95 ms | sum max ms | load start | load end |\n|---|---|---|---|---|---|---|---|---|---|";

const ENGINE_IDS = { barnes_hut: "layout.force.barnes_hut", particle_mesh: "layout.force.particle_mesh" };

function fail(message) {
  process.stderr.write(`wasm-stream-bench: could not run: ${message}\n`);
  process.exit(2);
}

/** The paths this arm can replay a stream through, and nothing else: an unknown value is a
 *  refusal, not a default, because the two print different columns and a typo would otherwise
 *  be read as a measurement. */
const PATHS = ["json", "columns"];

/** `--from`, `--engine`, `--wasm`, `--path`, `--split`. Nothing else: an unknown flag is a
 *  refusal, not a default. */
function parseArgs(argv) {
  const plan = { wasm: DEFAULT_WASM, from: null, engine: "barnes_hut", path: "json", split: false };
  for (let i = 0; i < argv.length; i += 1) {
    const [flag, inline] = argv[i].split("=");
    const value = () => inline ?? argv[(i += 1)];
    if (flag === "--from") plan.from = value();
    else if (flag === "--wasm") plan.wasm = resolve(value());
    else if (flag === "--engine") plan.engine = value();
    else if (flag === "--path") plan.path = value();
    else if (flag === "--split") plan.split = true;
    else fail(`unknown argument ${flag}`);
  }
  if (plan.from === null) fail("--from <path.jsonl> is the stream to replay");
  if (!existsSync(plan.from)) fail(`${plan.from} does not exist`);
  if (!existsSync(plan.wasm)) fail(`${plan.wasm} does not exist (cargo build -p graph-wasm --release --target wasm32-unknown-unknown)`);
  if (!(plan.engine in ENGINE_IDS)) fail(`--engine takes ${Object.keys(ENGINE_IDS).join(" or ")}`);
  if (!PATHS.includes(plan.path)) fail(`--path takes ${PATHS.join(" or ")}`);
  // A split needs an encoder to split off, and `JSON.stringify` is inside `extend` in a way
  // `encodeBatch` is not: it is one call the SDK makes, not a walk the host wrote.
  if (plan.split && plan.path !== "columns") fail("--split is only defined with --path columns");
  return plan;
}

/** The median, the mean of the two central values when the count is even. */
function median(values) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

/** The `q` quantile, interpolating between order statistics — `stream/stats.rs`'s rule. */
function quantile(values, q) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const at = Math.min(Math.max(q, 0), 1) * (sorted.length - 1);
  const low = Math.floor(at);
  const high = Math.ceil(at);
  return sorted[low] + (sorted[high] - sorted[low]) * (at - low);
}

function max(values) {
  return values.length === 0 ? 0 : Math.max(...values);
}

/** `/proc/loadavg`'s first three fields, or `unavailable` where there is no `/proc`. */
function loadavg() {
  try {
    return readFileSync("/proc/loadavg", "utf8").split(/\s+/).slice(0, 3).join(" ");
  } catch {
    return "unavailable";
  }
}

/**
 * The engine's own `.ts` sources import each other without extensions (`./model`), which
 * plain ESM resolution rejects. This is `wasm-tick-bench.mjs`'s hook unchanged: two arms that
 * load the SDK two ways are two arms that can disagree about it.
 */
function registerTypeScript() {
  registerHooks({
    resolve(specifier, context, next) {
      if (specifier.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(specifier)) {
        for (const extension of [".ts", "/index.ts"]) {
          try {
            return next(specifier + extension, context);
          } catch {
            /* not this one; try the next shape */
          }
        }
      }
      return next(specifier, context);
    },
  });
}

/** Why `motor` is not a motor this arm can time, or `null` when it is. */
function motorRefusal(motor, plan) {
  if (motor.available) return null;
  let why = "the SDK reported no reason";
  try {
    motor.layouts();
  } catch (error) {
    why = error && error.message ? error.message : String(error);
  }
  return `${plan.wasm} did not load, so no session can be timed through it: ${why}`;
}

/**
 * The stream's lines, one at a time, so a 700 MB file is never held whole. The awaits sit
 * outside every timer, which is the same position a line's `read_line` takes natively.
 */
async function* lines(path) {
  const stream = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
  for await (const line of stream) {
    if (line.length > 0) yield line;
  }
}

/**
 * One batch: `extend`, then `grow`, each in its own timer, then one untimed `tick(1)`.
 *
 * `--path json` times `motor.extend` and `--path columns` times `motor.extendColumns`, and the
 * SDK's own encoding is **inside** the `extend` timer in both: `JSON.stringify` on one side,
 * `encodeBatch` on the other. That is deliberate and it is the asymmetry this bench exists to
 * measure (A1 in `docs/decisions/extend-columns.md`): a host pays whatever its SDK's encoder
 * costs, so a columnar `extend` that excluded the encode would flatter the wasm arm by exactly
 * the number A1 asks about. The native arm cannot be read the same way — it hands
 * `service::extend_columns` bytes written by a Rust encoder outside its timer — so the two
 * `columns` columns differ by that encode and the two `json` columns differ by `JSON.stringify`.
 *
 * Caveat: `tick(1)` runs after both timers close, so this measures what the contract names
 * and not what a host's frame costs — a host that ticked per batch would pay a tick per
 * 10 000 nodes that no column here shows.
 */
function batch(ctx, parsed) {
  const { motor, handle, session } = ctx;
  const at = { nodes: parsed.nodes, edges: parsed.edges };
  // The untimed `encodeBatch` under `--split`: its bytes are thrown away and the timed call
  // encodes again, so the `extend` column below means the same thing in both modes.
  const encodeMs = ctx.split ? timeEncode(ctx.encodeBatch, at).ms : 0;
  const startedExtend = performance.now();
  if (ctx.path === "columns") motor.extendColumns(handle, at);
  else motor.extend(handle, at);
  const extendMs = performance.now() - startedExtend;
  const startedGrow = performance.now();
  session.grow(handle);
  const growMs = performance.now() - startedGrow;
  session.tick(1);
  return { nodesAfter: motor.nodeCount(handle), encodeMs, extendMs, growMs };
}

/** The whole replay: line 0 untimed, one timed pair per batch after it, and the node counts. */
async function replay(motor, plan) {
  const loadStart = loadavg();
  const rows = [];
  let handle = null;
  let session = null;
  let nodes = 0;
  let batchNodes = 0;
  for await (const line of lines(plan.from)) {
    if (handle === null) {
      // Line 0 goes to `gm_build` as the bytes it is, not as a re-serialized object, so the
      // untimed build on this side parses the same document the native one does.
      try {
        handle = motor.build(line);
      } catch (error) {
        fail(`line 0: ${error && error.message ? error.message : String(error)}`);
      }
      session = motor.forceSession(handle, undefined, plan.engine);
      session.tick(WARM_TICKS);
      nodes = motor.nodeCount(handle);
      continue;
    }
    const parsed = parse(line);
    if (batchNodes === 0) batchNodes = parsed.nodes.length;
    rows.push(batch({ motor, handle, session, path: plan.path, split: plan.split, encodeBatch: plan.encodeBatch }, parsed));
    nodes += parsed.nodes.length;
  }
  if (handle === null) fail(`${plan.from} holds no line, so there is no stream to replay`);
  return { handle, rows, nodes, batchNodes, loadStart };
}

/** One line as the object `extend` takes, or exit 2 naming the line. */
function parse(line) {
  try {
    const parsed = JSON.parse(line);
    if (!Array.isArray(parsed.nodes) || !Array.isArray(parsed.edges)) {
      fail("a batch line has no nodes array or no edges array");
    }
    return parsed;
  } catch (error) {
    return fail(`a batch line is not JSON: ${error && error.message ? error.message : String(error)}`);
  }
}

/** The two snapshot calls the studio rebuilds from, timed once at the final size. */
function snapshot(motor, handle) {
  if (!motor.layouts().includes(SNAPSHOT_LAYOUT)) fail(`the module does not register ${SNAPSHOT_LAYOUT}`);
  const startedRun = performance.now();
  motor.run(handle, SNAPSHOT_LAYOUT);
  const runMs = performance.now() - startedRun;
  const startedBytes = performance.now();
  const bytes = motor.toBytes(handle);
  return { runMs, bytesMs: performance.now() - startedBytes, bytes: bytes.length };
}

/** The per-batch rows, then the one summary row, then the snapshot line. `--split` swaps the
 *  first four lines for `stream-split.mjs`'s, which owns the `encode` column. */
function markdown(plan, run, snap) {
  const tables = plan.split
    ? splitTables(run, { median, quantile, max }, ENGINE_IDS[plan.engine])
    : defaultTables(plan, run);
  return [
    tables.batchHeader,
    tables.batchRows,
    tables.summaryHeader,
    tables.summaryRow,
    "",
    `snapshot: run(${SNAPSHOT_LAYOUT}) ${snap.runMs.toFixed(2)} ms, toBytes ${snap.bytesMs.toFixed(2)} ms for ${snap.bytes} bytes`,
    "",
  ].join("\n");
}

/** The default mode's four markdown pieces, byte for byte what it printed before `--split`. */
function defaultTables(plan, run) {
  const sums = run.rows.map((r) => r.extendMs + r.growMs);
  return {
    batchHeader: BATCH_HEADER,
    batchRows: run.rows
      .map((r, i) => `| ${i + 1} | ${r.nodesAfter} | ${r.extendMs.toFixed(2)} | ${r.growMs.toFixed(2)} | ${(r.extendMs + r.growMs).toFixed(2)} |`)
      .join("\n"),
    summaryHeader: SUMMARY_HEADER,
    summaryRow: `| ${ENGINE_IDS[plan.engine]} | ${run.batchNodes} | ${run.nodes} | ${median(run.rows.map((r) => r.extendMs)).toFixed(2)} | ${median(run.rows.map((r) => r.growMs)).toFixed(2)} | ${median(sums).toFixed(2)} | ${quantile(sums, 0.95).toFixed(2)} | ${max(sums).toFixed(2)} | ${run.loadStart} | ${loadavg()} |`,
  };
}

async function main() {
  const plan = parseArgs(process.argv.slice(2));
  registerTypeScript();
  const { createMotor, encodeBatch } = await import("../crates/graph-sdk-js/src/index.ts");
  plan.encodeBatch = encodeBatch;
  const motor = await createMotor(readFileSync(plan.wasm));
  const unusable = motorRefusal(motor, plan);
  if (unusable !== null) fail(unusable);
  const run = await replay(motor, plan);
  process.stdout.write(markdown(plan, run, snapshot(motor, run.handle)));
}

main().catch((error) => fail(error && error.message ? error.message : String(error)));