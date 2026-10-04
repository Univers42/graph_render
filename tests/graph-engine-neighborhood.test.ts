/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-neighborhood.test.ts                  :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  +:+       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/06/08 12:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/06/08 12:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

import assert from "node:assert/strict";
import test from "node:test";

import { edge, node } from "./graph-engine-fixtures.ts";
import { indexModel } from "../src/core/model/model.ts";
import { neighborhood, neighborhoodEdges } from "../src/core/model/neighborhood.ts";
import { buildSyntheticModel } from "../src/core/model/synthetic.ts";

test("neighborhood collects the node + its 1-hop ring", () => {
  const model = indexModel(
    [node("a", "record", "db1"), node("b", "record", "db1"), node("c", "record", "db1"), node("d", "record", "db1")],
    [edge("e1", "a", "b", "relation"), edge("e2", "a", "c", "relation")],
  );
  const set = neighborhood(model, "a", 1);
  assert.ok(set.has("a") && set.has("b") && set.has("c"));
  assert.ok(!set.has("d"));
});

test("neighborhood: an unknown id yields nothing, not a phantom singleton (regression)", () => {
  const model = indexModel(
    [node("a", "record", "db1", 1), node("b", "record", "db1", 0.5)],
    [edge("e1", "a", "b", "relation", 1)],
  );
  // Previously seeded the frontier unconditionally, so a just-deleted id came
  // back as Set { "<deleted id>" } — read by callers as "selected and in focus".
  assert.equal(neighborhood(model, "gone", 1).size, 0);
  assert.equal(neighborhood(model, "a", 1).size, 2);
});

test("neighborhoodEdges: returns the edges walked alongside the nodes", () => {
  const model = indexModel(
    [node("a", "record", "db1", 1), node("b", "record", "db1", 0.5), node("c", "record", "db1", 0.4)],
    [edge("e1", "a", "b", "relation", 1), edge("e2", "b", "c", "relation", 1)],
  );
  const near = neighborhoodEdges(model, "a", 1);
  assert.deepEqual([...near.nodeIds].sort(), ["a", "b"]);
  assert.deepEqual([...near.edgeIds], ["e1"]);

  const far = neighborhoodEdges(model, "a", 2);
  assert.deepEqual([...far.nodeIds].sort(), ["a", "b", "c"]);
  assert.deepEqual([...far.edgeIds].sort(), ["e1", "e2"]);

  // The nodes-only view must agree with the edge-returning one. This is a
  // delegation check, not an independent one: `neighborhood` IS defined as
  // `bfs(...).nodeIds`, so it can only fail if that delegation is removed. Kept
  // as a cheap tripwire on the refactor, and labelled so it is not mistaken for
  // independent evidence that the two traversals agree.
  assert.deepEqual([...neighborhood(model, "a", 2)].sort(), [...far.nodeIds].sort());
});

test("synthetic: buildSyntheticModel is deterministic and respects its cap", () => {
  const a = buildSyntheticModel(64);
  const b = buildSyntheticModel(64);
  // Seeded PRNG: identical inputs must give byte-identical models, or every
  // benchmark run and screenshot comparison is noise. Compare the whole model,
  // not a sampled projection of it: a previous version compared only
  // id/kind/label, so a PRNG that drifted purely on icon, group, weight, edge
  // strength or the hasNote mix would still have passed. Those are exactly the
  // fields this fixture exists to vary.
  assert.deepEqual(a, b);
  assert.deepEqual(a.nodes.map((n) => n.icon), b.nodes.map((n) => n.icon));
  assert.deepEqual(a.nodes.map((n) => n.weight), b.nodes.map((n) => n.weight));
  assert.deepEqual(a.edges.map((e) => e.strength), b.edges.map((e) => e.strength));
  assert.equal(a.nodes.length, 64);
  // Indexes are built, not just the raw arrays.
  assert.equal(a.nodeById.size, 64);
  assert.ok(a.edges.length > 0);
  // Floor of 2, ceiling 100k (the resident per-node model's limit). The ceiling is
  // asserted rather than described: a previous version asserted the floor twice
  // and claimed to cover the ceiling without ever building a model near it.
  assert.equal(buildSyntheticModel(0).nodes.length, 2, "floor");
  assert.equal(buildSyntheticModel(-99).nodes.length, 2, "floor clamps negatives");
  assert.equal(buildSyntheticModel(1_000_000).nodes.length, 100_000, "ceiling");
  // Non-integer and non-finite input must not silently yield a wrong-sized or
  // empty model. buildSyntheticModel(2.5) used to give 3 nodes while the edge
  // loop indexed past the end of the id list, and buildSyntheticModel(NaN) used
  // to give 0 nodes — an empty graph, no error, so a bench run measured nothing.
  assert.equal(buildSyntheticModel(2.5).nodes.length, 2, "fractional input floors");
  assert.equal(buildSyntheticModel(NaN).nodes.length, 2, "NaN falls back to the floor");
  assert.ok(buildSyntheticModel(Infinity).nodes.length <= 100_000, "Infinity clamps to the ceiling");
});
