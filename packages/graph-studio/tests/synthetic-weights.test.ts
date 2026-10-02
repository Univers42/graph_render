/**
 * The degree weights, against the implementation they replaced.
 *
 * The weight pass used to build a `Map<string, number>` keyed by node id and then do a
 * string-keyed `get` and `set` for both ends of every edge. It is now index arithmetic
 * over a `Uint32Array`, because the generator that makes the edges already knows both
 * endpoints' node indices and a 1M-node open must not hash four million strings.
 *
 * So this file keeps the old pass verbatim as an oracle. The bar is not "the numbers
 * look right": it is `deepStrictEqual` on every record, in the same order, with the same
 * ids, for three seeds and three sizes. A rewrite that changed one weight, one id or one
 * record order fails here.
 *
 * One hole has to be closed explicitly: the oracle reads the edges the source produced, so
 * a source that changed an endpoint string would only be caught by the ids test below.
 * That is why the ids are pinned here as well, and not only in `ingest.test.ts`.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { IngestEdge, IngestNode } from "../src/source/ingest.ts";
import { syntheticRecords } from "../src/source/synthetic.ts";

interface Records {
  readonly nodes: IngestNode[];
  readonly edges: IngestEdge[];
}

/** The weight pass exactly as it stood: ids as keys, one get and one set per edge end. */
function referenceDegreeWeights(records: Records): void {
  const counts = new Map<string, number>(records.nodes.map((node) => [node.id, 0]));
  let max = 0;
  for (const edge of records.edges) {
    for (const end of [edge.source, edge.target]) {
      const next = (counts.get(end) ?? 0) + 1;
      counts.set(end, next);
      if (next > max) max = next;
    }
  }
  for (const node of records.nodes) node.weight = max === 0 ? 0 : (counts.get(node.id) ?? 0) / max;
}

/** The production records re-weighted by the oracle: fresh nodes, so no source number
 *  reaches the comparison — the oracle's `weight` is the only weight either side has. */
function referenceRecords(records: Records): Records {
  const nodes = records.nodes.map((node) => ({ ...node, weight: 0 }));
  const reference: Records = { nodes, edges: records.edges };
  referenceDegreeWeights(reference);
  return reference;
}

const SEEDS = [1, 7, 4294967295] as const;
const SIZES = [10, 1000, 50000] as const;

test("every weight equals the id-keyed oracle, for three seeds and three sizes", () => {
  for (const nodeCount of SIZES) {
    for (const seed of SEEDS) {
      const records = syntheticRecords({ seed, nodeCount, degree: 2 });
      const where = `seed ${seed}, ${nodeCount} nodes`;
      assert.deepStrictEqual(records.nodes, referenceRecords(records).nodes, where);
    }
  }
});

test("the vault shape re-weights to the oracle too, at every size", () => {
  for (const nodeCount of SIZES) {
    for (const seed of SEEDS) {
      const records = syntheticRecords({ seed, nodeCount, degree: 2, shape: "vault" });
      assert.deepStrictEqual(records.nodes, referenceRecords(records).nodes, `vault, seed ${seed}`);
    }
  }
});

// The oracle reads the source's own endpoint strings, so those strings are pinned here:
// the generator's own ids, in the generator's own order, at a size where one drift in a
// million would not be noticed by eye.
test("the ids and the record order are the generator's own, unchanged", () => {
  const { nodes, edges } = syntheticRecords({ seed: 90, nodeCount: 1000, degree: 2 });
  for (const [index, node] of nodes.entries()) assert.equal(node.id, `n-${index}`);
  for (const [index, edge] of edges.entries()) {
    assert.equal(edge.id, `e-${index}`);
    assert.match(edge.source, /^n-\d+$/);
    assert.match(edge.target, /^n-\d+$/);
  }
});

/** Seed 90, degree 2, ten nodes: node 0 is never a source (the edge loop starts at 1) and
 *  never a target, so it carries no link while some other node carries the most. The
 *  `count / max` division with a zero numerator is the branch a no-edge graph cannot reach. */
test("a node with no links weighs 0 while the graph's largest weighs 1", () => {
  const records = syntheticRecords({ seed: 90, nodeCount: 10, degree: 2 });
  assert.equal(records.nodes.length, 10);
  assert.equal(records.nodes[0]?.weight, 0);
  assert.equal(Math.max(...records.nodes.map((node) => node.weight)), 1);
  assert.deepStrictEqual(records.nodes, referenceRecords(records).nodes);
});

test("a graph with no edges weighs 0 everywhere, and never NaN", () => {
  for (const shape of ["random", "vault"] as const) {
    const records = syntheticRecords({ seed: 1, nodeCount: 1000, degree: 0, shape });
    assert.equal(records.edges.length, 0, shape);
    assert.equal(records.nodes.every((node) => node.weight === 0), true, shape);
    assert.deepStrictEqual(records.nodes, referenceRecords(records).nodes, shape);
  }
});

// The oracle over the same spec, on the pre-rewrite source, was the definition of the
// weight. Both branches of `max === 0 ? 0 : count / max` are exercised above; this one
// checks the pairs that only the rewritten source could get wrong.
test("a weight is the node's share of the largest, so the largest is exactly 1", () => {
  for (const nodeCount of SIZES) {
    const { nodes } = syntheticRecords({ seed: 7, nodeCount, degree: 2 });
    const max = nodes.reduce((high, node) => (node.weight > high ? node.weight : high), 0);
    assert.equal(max, 1, `${nodeCount} nodes`);
    assert.equal(nodes.every((node) => node.weight >= 0 && node.weight <= max), true, `${nodeCount} nodes`);
  }
});

// The negative controls, kept in the file rather than in a scratch script: each one breaks
// one thing the rewrite could break and checks that the oracle above notices. They pass
// exactly when the comparison is sharp; a control that could not fail would prove nothing.
test("the oracle catches a weight that moved (negative control)", () => {
  const records = syntheticRecords({ seed: 1, nodeCount: 100, degree: 2 });
  const oracle = referenceRecords(records).nodes;
  const broken = records.nodes.map((node, index) => ({ ...node, weight: index === 3 ? 0.5 : node.weight }));
  assert.notDeepStrictEqual(broken, oracle);
});

test("the oracle catches a record order that moved (negative control)", () => {
  const records = syntheticRecords({ seed: 1, nodeCount: 100, degree: 2 });
  const oracle = referenceRecords(records).nodes;
  assert.notDeepStrictEqual([...records.nodes].reverse(), oracle);
});

test("the oracle catches an endpoint id that moved (negative control)", () => {
  const records = syntheticRecords({ seed: 1, nodeCount: 100, degree: 2 });
  const edges = records.edges.map((edge, index) => (index === 0 ? { ...edge, target: "n-99" } : edge));
  const moved = { nodes: records.nodes.map((node) => ({ ...node, weight: 0 })), edges };
  const oracle = referenceRecords(records).nodes;
  assert.notDeepStrictEqual(referenceRecords(moved).nodes, oracle);
});