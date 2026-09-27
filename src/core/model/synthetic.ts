/**
 * Deterministic synthetic GraphModel for benchmarks and visual tests.
 * Preferential-attachment topology gives a realistic hub/leaf degree
 * distribution; the seeded PRNG keeps screenshots and FPS runs reproducible
 * across builds.
 *
 * Extracted verbatim from osionos `src/widgets/graph-explorer/syntheticGraph.ts`.
 * It belonged here: it produces this package's own data contract to exercise
 * this package's own render pipeline (LOD cluster-blobs, node budget), and both
 * of its imports already existed in `core/`, so nothing host-specific came with
 * it. Two hosts were each rolling their own copy otherwise.
 *
 * NOT extracted: the file's `graphBenchCount()` helper, which reads the
 * `?graphBench=N` URL parameter. Reading a query string is a host concern — it
 * assumes a browser and a specific URL scheme — so it stayed in osionos.
 */

import { indexModel } from "./model";
import { applyDegreeWeights } from "./weights";
import type { GraphEdge, GraphModel, GraphNode } from "../types";

const DATABASES = 8;
const EMOJI = ["🚀", "📚", "🧠", "🌿", "🔬", "🎯", "🗺️", "💡", "🎨", "⚙️", "📈", "🧩"];
const GROUPS = ["Active", "Draft", "Review", "Done", "Archived"];
const ICONS = ["icon:rocket", "icon:book", "icon:target", "icon:map"];

/** mulberry32 — tiny seeded PRNG, deterministic across runs. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function syntheticNode(i: number, rnd: () => number): GraphNode {
  const roll = rnd();
  const icon = roll < 0.6
    ? EMOJI[Math.floor(rnd() * EMOJI.length)]
    : roll < 0.8
      ? ICONS[Math.floor(rnd() * ICONS.length)]
      : undefined;
  return {
    id: `bench:db-${i % DATABASES}:${i}`,
    kind: i % 23 === 0 ? "note" : "record",
    databaseId: `db-${i % DATABASES}`,
    source: "bench",
    label: `Node ${i}`,
    group: GROUPS[i % GROUPS.length],
    weight: 0.5,
    version: 0,
    hasNote: i % 17 === 0,
    icon,
  };
}

function syntheticEdges(count: number, rnd: () => number, ids: string[]): GraphEdge[] {
  const edges: GraphEdge[] = [];
  const push = (a: number, b: number, kind: GraphEdge["kind"]): void => {
    if (a === b) return;
    edges.push({
      id: `bench-e-${edges.length}`,
      source: ids[a],
      target: ids[b],
      kind,
      label: "",
      strength: 0.4 + rnd() * 0.4,
      directed: kind === "relation",
    });
  };
  // Preferential attachment: each node links to ~1.5 earlier nodes, biased
  // toward low indices (rnd()^2) so early nodes become hubs.
  for (let i = 1; i < count; i += 1) {
    push(i, Math.floor(rnd() * rnd() * i), "relation");
    if (rnd() < 0.5) push(i, Math.floor(rnd() * rnd() * i), "relation");
  }
  const extras = Math.floor(count * 0.05);
  for (let i = 0; i < extras; i += 1) {
    push(Math.floor(rnd() * count), Math.floor(rnd() * count), "note_link");
  }
  return edges;
}

/** Build a deterministic n-node model (icons included for LOD testing). */
export function buildSyntheticModel(n: number): GraphModel {
  // `Math.floor` first, and NaN explicitly to 2. Without them `n` is trusted to be
  // a non-negative integer and it is not: `buildSyntheticModel(2.5)` produced 3
  // nodes while the edge loop indexed `ids` up to 4 and emitted `undefined`
  // endpoints that `indexModel` silently dropped, and `buildSyntheticModel(NaN)`
  // produced a 0-node model — `Math.max(2, NaN)` is NaN, the loop body never ran,
  // and the caller got an empty graph with no error. A bench harness that silently
  // benchmarks nothing is worse than one that throws.
  const requested = Number.isFinite(n) ? Math.floor(n) : 2;
  // Cap at 100k: the render pipeline (LOD cluster-blobs + node budget) is designed
  // for this band; higher would OOM the resident per-node model.
  const count = Math.max(2, Math.min(requested, 100_000));
  const rnd = mulberry32(0x051042);
  const nodes: GraphNode[] = [];
  for (let i = 0; i < count; i += 1) nodes.push(syntheticNode(i, rnd));
  const edges = syntheticEdges(count, rnd, nodes.map((node) => node.id));
  applyDegreeWeights(nodes, edges);
  return indexModel(nodes, edges);
}
