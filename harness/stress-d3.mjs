// The d3-force arm of the stress gate (`graph-cli stress --oracle d3`).
//
//   node harness/stress-d3.mjs <input.jsonl> <output.jsonl>
//
// Reads one case per input line — `{"seed", "n", "edges": [[lo,hi], ...]}` — runs a real
// `d3-force@3.0.0` simulation of that graph with the FROZEN force set, and writes the
// positions it reached and the wall time of that case (simulation setup plus the 112
// ticks, `ms`), one line per input line, in order.
//
// The frozen set is graph-core's own `ForceParams::default()`, restated from
// `crates/graph-core/src/layout/force/params.rs` (which cites each value to its source
// line) — this file is the third restatement of those numbers in the repo, and the
// first two are that module and `docs/measurements/phase06-stress.md`:
//
//   forceLink(links).id(d => d.index)
//     .distance(l => 60 / Math.max(0.4, l.strength))
//     .strength(l => Math.min(0.7, 0.15 * l.strength))
//   forceManyBody().strength(-90).distanceMax(520)   // theta stays d3's own 0.9
//   forceCenter(0, 0)
//   forceCollide().radius(16).iterations(1)
//   // .alpha(1).alphaDecay(0.06).velocityDecay(0.42), then sim.tick(112)
//
// The seed positions are the same golden spiral graph-core's `barnes_hut/seed.rs`
// uses, so both arms start from the same picture and only the ALGORITHM differs — a
// difference in the starting layout would otherwise be measured as a difference in
// quality. Every link's `strength` is 0.5, the gate model's own edge strength.
//
// It computes no metric: the stress correlation is applied to both arms' positions by
// graph-core (`crates/graph-cli/src/stress/metric.rs`), once, so that a disagreement
// between two implementations of the metric cannot be mistaken for a disagreement
// between two layouts.
//
// It reads no environment variable. Exit codes follow graph-cli: 0 ran · 2 could not
// run (d3-force unresolvable, malformed input).

import { readFileSync, writeFileSync } from "node:fs";

// d3-force is resolved as ESM first, then through `createRequire`. The second path
// exists because ESM resolution ignores NODE_PATH, and the pinned d3 tree lives outside
// this worktree (a read-only node_modules mounted alongside it). `createRequire` uses
// CommonJS resolution, which does honour NODE_PATH, so a container that mounts the
// pinned tree and points NODE_PATH at it can run this harness with nothing installed
// in the worktree itself.
async function loadD3Force() {
  try {
    return await import("d3-force");
  } catch (esm) {
    try {
      const { createRequire } = await import("node:module");
      return createRequire(import.meta.url)("d3-force");
    } catch (cjs) {
      throw new Error(`${esm.message} (and via createRequire: ${cjs.message})`);
    }
  }
}

let d3;
try {
  d3 = await loadD3Force();
} catch (error) {
  process.stderr.write(
    `stress-d3: could not run: d3-force is not resolvable (${error.message}).\n` +
      "stress-d3: it must be on NODE_PATH or in a node_modules the workspace root can see;\n" +
      "stress-d3: a stress run that cannot reach d3-force is a refusal, never a pass.\n",
  );
  process.exit(2);
}

const { forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide } = d3;

const GOLDEN_ANGLE = 2.399963229728653;
const TICKS = 112;
const SEED_RADIUS = 12;

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  process.stderr.write("usage: stress-d3.mjs <input.jsonl> <output.jsonl>\n");
  process.exit(2);
}

function fail(message) {
  process.stderr.write(`stress-d3: ${message}\n`);
  process.exit(2);
}

/** The golden spiral `barnes_hut/seed.rs` seeds from, mirrored so both arms match. */
function goldenSpiral(n) {
  const x = new Array(n);
  const y = new Array(n);
  for (let i = 0; i < n; i += 1) {
    const r = SEED_RADIUS * Math.sqrt(i + 1);
    x[i] = Math.cos(i * GOLDEN_ANGLE) * r;
    y[i] = Math.sin(i * GOLDEN_ANGLE) * r;
  }
  return { x, y };
}

/** One case: the frozen force set over `edges`, from the golden spiral. */
function runD3(n, edges) {
  const nodes = Array.from({ length: n }, (_, i) => ({ index: i }));
  const seed = goldenSpiral(n);
  for (let i = 0; i < n; i += 1) {
    nodes[i].x = seed.x[i];
    nodes[i].y = seed.y[i];
  }
  const links = edges.map(([source, target]) => ({ source, target, strength: 0.5 }));
  const link = forceLink(links)
    .id((d) => d.index)
    .distance((l) => 60 / Math.max(0.4, l.strength))
    .strength((l) => Math.min(0.7, 0.15 * l.strength));
  const sim = forceSimulation(nodes)
    .force("link", link)
    .force("charge", forceManyBody().strength(-90).distanceMax(520))
    .force("center", forceCenter(0, 0))
    .force("collide", forceCollide().radius(16).iterations(1))
    .alpha(1)
    .alphaDecay(0.06)
    .velocityDecay(0.42)
    .stop();
  sim.tick(TICKS);
  return { x: nodes.map((d) => d.x), y: nodes.map((d) => d.y) };
}

const lines = readFileSync(inputPath, "utf8").split("\n").filter((l) => l.trim() !== "");
const out = [];
for (const [number, line] of lines.entries()) {
  let input;
  try {
    input = JSON.parse(line);
  } catch (error) {
    fail(`case ${number}: ${error.message}`);
  }
  const { seed, n, edges } = input;
  if (!Number.isInteger(n) || n < 0) fail(`case ${number}: no node count`);
  if (!Array.isArray(edges)) fail(`case ${number}: edges is not an array`);
  const started = performance.now();
  const { x, y } = runD3(n, edges);
  const ms = performance.now() - started;
  // The harness is a JS program and its arithmetic is f64 throughout, so these are
  // written with full round-trip precision: graph-core widens them to f64 and both
  // arms are then correlated in one type, with no decimal truncation in between.
  out.push(JSON.stringify({ seed, x, y, ms }));
}
writeFileSync(outputPath, out.length === 0 ? "" : `${out.join("\n")}\n`);
