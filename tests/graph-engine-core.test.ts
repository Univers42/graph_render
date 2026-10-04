/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-core.test.ts                          :+:      :+:    :+:   */
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
import { clamp, lerp, smoothstep } from "../src/core/math.ts";
import { makeEdgeId } from "../src/core/model/ids.ts";
import { emptyModel, indexModel } from "../src/core/model/model.ts";
import { applyDegreeWeights, weightToRadius } from "../src/core/model/weights.ts";
import { deriveLegend } from "../src/core/model/legend.ts";

test("math: clamp / lerp / smoothstep", () => {
  assert.equal(clamp(5, 0, 3), 3);
  assert.equal(clamp(-1, 0, 3), 0);
  assert.equal(lerp(0, 10, 0.5), 5);
  assert.equal(smoothstep(0), 0);
  assert.equal(smoothstep(1), 1);
  assert.ok(smoothstep(0.5) > 0.49 && smoothstep(0.5) < 0.51);
});

test("ids: undirected edges collapse, directed stay distinct", () => {
  const ab = makeEdgeId("a", "b", "relation", "rel", false);
  const ba = makeEdgeId("b", "a", "relation", "rel", false);
  assert.equal(ab, ba);
  assert.notEqual(makeEdgeId("a", "b", "relation", "rel", true), makeEdgeId("b", "a", "relation", "rel", true));
});

test("indexModel dedupes nodes, drops dangling edges, builds indexes", () => {
  const model = indexModel(
    [node("a", "record", "db1"), node("a", "record", "db1"), node("b", "record", "db1")],
    [edge("e1", "a", "b", "relation"), edge("e2", "a", "ghost", "relation")],
  );
  assert.equal(model.stats.nodes, 2);
  assert.equal(model.stats.edges, 1); // dangling e2 dropped
  assert.equal(model.adjacency.get("a")?.length, 1);
  assert.deepEqual(model.byDatabase.get("db1"), ["a", "b"]);
  assert.equal(emptyModel().stats.nodes, 0);
});

test("degree weights are monotonic in degree; radius respects bounds", () => {
  const nodes = [node("hub", "record", "db1"), node("a", "record", "db1"), node("b", "record", "db1"), node("lonely", "record", "db1")];
  const edges = [edge("e1", "hub", "a", "relation"), edge("e2", "hub", "b", "relation")];
  applyDegreeWeights(nodes, edges);
  const hub = nodes[0].weight;
  const lonely = nodes[3].weight;
  assert.ok(hub > lonely, "hub heavier than isolated node");
  assert.ok(lonely >= 0.2, "floor at 0.2");
  assert.equal(weightToRadius(0, 5, 22), 5);
  assert.equal(weightToRadius(1, 5, 22), 22);
});

test("deriveLegend counts databases, tags and kinds", () => {
  const model = indexModel(
    [node("a", "record", "db1"), node("b", "record", "db1"), node("t", "tag", null)],
    [edge("e1", "a", "t", "tag")],
  );
  const legend = deriveLegend(model);
  assert.equal(legend.databases.find((d) => d.id === "db1")?.count, 2);
  assert.equal(legend.tags.find((t) => t.label === "t")?.count, 1);
  assert.ok(legend.kinds.some((k) => k.kind === "record" && k.count === 2));
});
