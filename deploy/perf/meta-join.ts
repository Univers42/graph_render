// Times `metaOf` over the studio's synthetic graph, with the order as fresh strings, the way
// `describe` decodes it from the snapshot. argv: n. Run it with node-slim.sh
// (docs/measurements/perf-open-meta.md).
//
// Caveat: wall clock, median of 7 in one process; host load moves it, a warm JIT favours later runs.
import { metaOf } from "../../packages/graph-studio/src/source/meta.ts";
import { syntheticRecords } from "../../packages/graph-studio/src/source/synthetic.ts";
const n = Number(process.argv[2]);
const { nodes, edges } = syntheticRecords({ seed: 1, nodeCount: n, degree: n >= 5000 ? 2 : 3, shape: "random" });
const row = new Map(nodes.map((node, i) => [node.id, i]));
const ends = { source: Uint32Array.from(edges, (e) => row.get(e.source) ?? 0), target: Uint32Array.from(edges, (e) => row.get(e.target) ?? 0) };
const times: number[] = [];
for (let run = 0; run < 7; run += 1) {
  const order = nodes.map((node) => `${node.id} `.trimEnd());
  const at = performance.now();
  metaOf(nodes, order, ends);
  times.push(performance.now() - at);
}
times.sort((a, b) => a - b);
console.log(JSON.stringify({ n, medianMs: Math.round(times[3] ?? 0), minMs: Math.round(times[0] ?? 0) }));
