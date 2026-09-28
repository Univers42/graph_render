// Phase 5 oracle differential: dagre-d3-es@7.0.14 vs our own crossing counts, on the 6
// fixtures/dag/*.json plus the >=200-seed synthetic sweep dumped by graph-core's ignored
// `layout::sugiyama::measurement::dump_crossing_measurements` test into
// scratch/dag-crossings.json (run: cargo test -p graph-core dump_crossing_measurements --
// --ignored). This script never tunes anything; it only measures and reports, per the
// margin frozen in docs/measurements/phase05-crossings.md BEFORE any number below existed.
//
// Technique: dagre-d3-es's public `layout()` returns only final node/edge positions, and a
// first attempt at reconstructing its crossing count from that geometry (bucketing points
// by exact rank `y`) was found unsound: `edge.points` are spline control points from the
// curve-fitting step, and do not sit on the rank's `y` at all (verified: a K4,4 edge's
// first bend point landed at y=6.25 against real rank y's of 5 and 65). So instead this
// asks dagre for its own crossing count directly, by re-running the same internal pipeline
// stages `layout()` itself runs, up to and including its own `order` stage, then calling
// its own `crossCount` on the resulting layering — the exact function dagre's ordering
// heuristic minimizes internally (Barth, Jünger & Mutzel's bilayer counting, same
// algorithm family as our own `ordering::bilayer_crossings`), read directly rather than
// reconstructed:
//
//   acyclic.run -> [attach every node to a scratch root, matching what dagre's own
//   nesting-graph does for a flat (non-compound) graph, so network-simplex ranking never
//   sees a disconnected input] -> rank -> [drop the scratch root] -> normalizeRanks ->
//   normalize.run (splits long edges into per-rank dummy chains) -> order -> crossCount.
//
// The root-attachment step reproduces `nesting-graph.js`'s `run` for the flat (no
// subgraphs) case exactly: with no compound children, its `treeDepths` gives every node
// depth 1, so `height = max(depths) - 1 = 0` and `nodeSep = 2*height + 1 = 1` always,
// meaning its own edges would be `{weight: 0, minlen: 1 * nodeSep}` = `{weight: 0, minlen:
// 1}` from a virtual root to every node — precisely what is added here by hand, without
// needing `compound: true` semantics beyond what the graph already declares.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Graph } from "dagre-d3-es/src/graphlib/index.js";
import * as acyclic from "dagre-d3-es/src/dagre/acyclic.js";
import * as normalize from "dagre-d3-es/src/dagre/normalize.js";
import { rank } from "dagre-d3-es/src/dagre/rank/index.js";
import { order } from "dagre-d3-es/src/dagre/order/index.js";
import { crossCount } from "dagre-d3-es/src/dagre/order/cross-count.js";
import * as util from "dagre-d3-es/src/dagre/util.js";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..");
const ROOT = "\u0000nesting-root";

function dagreCrossings(nodes, edges) {
  const g = new Graph({ directed: true, multigraph: true, compound: true });
  g.setGraph({});
  for (const id of nodes) g.setNode(id, {});
  for (const [s, t] of edges) g.setEdge(s, t, { weight: 1, minlen: 1 });

  acyclic.run(g);
  g.setNode(ROOT, {});
  for (const id of nodes) g.setEdge(ROOT, id, { weight: 0, minlen: 1 });
  rank(g);
  g.removeNode(ROOT); // also drops the root's incident edges
  util.normalizeRanks(g);

  normalize.run(g);
  order(g);
  return crossCount(g, util.buildLayerMatrix(g));
}

function main() {
  const dumpPath = join(repoRoot, "scratch", "dag-crossings.json");
  const graphs = JSON.parse(readFileSync(dumpPath, "utf8"));
  const rows = graphs.map((g) => {
    const dagre = dagreCrossings(g.nodes, g.edges);
    return { name: g.name, ours: g.our_crossings, dagre };
  });
  writeFileSync(join(repoRoot, "scratch", "crossing-comparison.json"), JSON.stringify(rows, null, 2) + "\n");

  const fixtureNames = new Set(["chain", "diamond", "cyclic", "multi-span", "wide-layer", "disconnected"]);
  const fixtures = rows.filter((r) => fixtureNames.has(r.name));
  const synthetic = rows.filter((r) => !fixtureNames.has(r.name));

  console.log("fixture               ours   dagre   margin(2,10%)   verdict");
  let anyFixtureWorse = false;
  for (const r of fixtures) {
    const margin = Math.max(2, Math.ceil(r.dagre * 0.1));
    const worse = r.ours > r.dagre + margin;
    anyFixtureWorse ||= worse;
    console.log(
      `${r.name.padEnd(20)} ${String(r.ours).padStart(5)} ${String(r.dagre).padStart(7)} ${String(margin).padStart(14)}   ${worse ? "WORSE" : "ok"}`,
    );
  }

  // The margin's second clause sums over the fixture set PLUS the synthetic sweep
  // together, not the sweep alone.
  const sumOurs = rows.reduce((a, r) => a + r.ours, 0);
  const sumDagre = rows.reduce((a, r) => a + r.dagre, 0);
  const sweepLimit = 1.1 * sumDagre;
  const sweepWorse = sumOurs > sweepLimit;
  console.log(`\nfixtures + synthetic sweep: graphs=${rows.length} (synthetic seeds=${synthetic.length}) sum(ours)=${sumOurs} sum(dagre)=${sumDagre} 1.10x(dagre)=${sweepLimit.toFixed(1)} verdict=${sweepWorse ? "WORSE" : "ok"}`);

  const materiallyWorse = anyFixtureWorse || sweepWorse;
  console.log(`\nstatus: ${materiallyWorse ? "blocked (materially worse, do not tune)" : "pass"}`);
  writeFileSync(
    join(repoRoot, "scratch", "crossing-verdict.json"),
    JSON.stringify(
      { fixtures, sweep: { seeds: synthetic.length, sumOurs, sumDagre, sweepLimit }, anyFixtureWorse, sweepWorse, materiallyWorse },
      null,
      2,
    ) + "\n",
  );
}
main();
