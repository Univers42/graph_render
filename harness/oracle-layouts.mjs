// Layout oracle arms (prompts/phase-05-sugiyama.md).
//
//   node harness/oracle-layouts.mjs --dag [dump]      dump defaults to target/dag-crossings.json
//
// --dag: dagre-d3-es@7.0.14 against our own crossing counts on the 6 fixtures/dag/*.json
// plus the synthetic sweep, both dumped by graph-core's ignored test
// (cargo test -p graph-core dump_crossing_measurements -- --ignored). It measures and
// reports; it never tunes anything. The margin is frozen in
// docs/measurements/phase05-crossings.md, which also says why the count is taken from
// dagre's own `order` + `crossCount` rather than rebuilt from its final geometry.
//
// Ponytail: dagre is driven through its internal modules (acyclic, rank, normalize,
// order, crossCount), not its public `layout()`, so a dagre release that renames them
// breaks the arm loudly (exit 2), never silently. The scratch-root
// attachment reproduces nesting-graph.js for flat graphs only. The guard below pins
// K4,4 to its hand-counted 36 crossings so a broken extraction cannot pass as zeros.
//
// Exit codes follow graph-cli: 0 within the margin · 1 materially worse · 2 could not run.
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const ROOT = "\u0000nesting-root";
const FIXTURES = new Set(["chain", "diamond", "cyclic", "multi-span", "wide-layer", "disconnected"]);
const K44 = { nodes: ["a", "b", "c", "d", "w", "x", "y", "z"], edges: [] };
for (const s of ["a", "b", "c", "d"]) for (const t of ["w", "x", "y", "z"]) K44.edges.push([s, t]);

async function loadDagre() {
  const at = (path) => import(`dagre-d3-es/src/${path}`);
  const [graphlib, acyclic, normalize, rank, order, cross, util] = await Promise.all([
    at("graphlib/index.js"), at("dagre/acyclic.js"), at("dagre/normalize.js"), at("dagre/rank/index.js"),
    at("dagre/order/index.js"), at("dagre/order/cross-count.js"), at("dagre/util.js"),
  ]);
  return (nodes, edges) => {
    const g = new graphlib.Graph({ directed: true, multigraph: true, compound: true });
    g.setGraph({});
    for (const id of nodes) g.setNode(id, {});
    for (const [s, t] of edges) g.setEdge(s, t, { weight: 1, minlen: 1 });
    acyclic.run(g);
    g.setNode(ROOT, {});
    for (const id of nodes) g.setEdge(ROOT, id, { weight: 0, minlen: 1 });
    rank.rank(g);
    g.removeNode(ROOT);
    util.normalizeRanks(g);
    normalize.run(g);
    order.order(g);
    return cross.crossCount(g, util.buildLayerMatrix(g));
  };
}

function report(rows) {
  const fixtures = rows.filter((r) => FIXTURES.has(r.name));
  console.log("fixture               ours   dagre   margin(2,10%)   verdict");
  let fixtureWorse = false;
  for (const r of fixtures) {
    const margin = Math.max(2, Math.ceil(r.dagre * 0.1));
    const worse = r.ours > r.dagre + margin;
    fixtureWorse ||= worse;
    console.log(`${r.name.padEnd(20)} ${String(r.ours).padStart(5)} ${String(r.dagre).padStart(7)} ${String(margin).padStart(14)}   ${worse ? "WORSE" : "ok"}`);
  }
  // The sum clause covers the fixtures and the sweep together, per the frozen margin.
  const sumOurs = rows.reduce((a, r) => a + r.ours, 0);
  const sumDagre = rows.reduce((a, r) => a + r.dagre, 0);
  const sweepWorse = sumOurs > 1.1 * sumDagre;
  console.log(`\ngraphs=${rows.length} (fixtures=${fixtures.length}) sum(ours)=${sumOurs} sum(dagre)=${sumDagre} 1.10x(dagre)=${(1.1 * sumDagre).toFixed(1)} verdict=${sweepWorse ? "WORSE" : "ok"}`);
  return !(fixtureWorse || sweepWorse);
}

async function main() {
  const args = process.argv.slice(2);
  if (args[0] !== "--dag") throw new Error("usage: oracle-layouts.mjs --dag [dump.json]");
  const dump = resolve(args[1] ?? `${repoRoot}/target/dag-crossings.json`);
  if (!existsSync(dump)) throw new Error(`${dump} missing: run cargo test -p graph-core dump_crossing_measurements -- --ignored`);
  const dagreCrossings = await loadDagre();
  const forced = dagreCrossings(K44.nodes, K44.edges);
  if (forced !== 36) throw new Error(`guard: dagre counts ${forced} on K4,4, the hand count is 36`);
  const graphs = JSON.parse(readFileSync(dump, "utf8"));
  const rows = graphs.map((g) => ({ name: g.name, ours: g.our_crossings, dagre: dagreCrossings(g.nodes, g.edges) }));
  const names = new Set(rows.map((r) => r.name));
  const missing = [...FIXTURES].filter((f) => !names.has(f));
  if (missing.length || rows.length < 200) throw new Error(`dump too small: ${rows.length} graphs, missing ${missing}`);
  const pass = report(rows);
  console.log(`\nstatus: ${pass ? "pass" : "blocked (materially worse, do not tune)"}`);
  process.exitCode = pass ? 0 : 1;
}

main().catch((err) => {
  console.error(`oracle-layouts: could not run: ${err.message}`);
  process.exitCode = 2;
});
