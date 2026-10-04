/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-diff.test.ts                          :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  +:+       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/06/08 12:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/06/08 12:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

import assert from "node:assert/strict";
import test from "node:test";

import type { GraphNode } from "../src/core/types.ts";
import { edge, node } from "./graph-engine-fixtures.ts";
import { indexModel, nodesEqual } from "../src/core/model/model.ts";
import { diffGraph, isEmptyPatch } from "../src/core/model/diff.ts";

// --- Extraction additions -------------------------------------------------
// These lock in behaviour that was WRONG in the package and correct in the host
// app's hand-mirrored copy. Each one is a regression guard, not a new feature:
// if the package's copy is ever re-derived from the app's, the diff that broke
// it is visible here.
// The other regression guards this block introduced now live in the sibling files
// graph-engine-ids / -neighborhood; each keeps its own inline rationale.

test("nodesEqual: an icon-only change is a real change (regression)", () => {
  const base: GraphNode = {
    id: "n1", kind: "record", databaseId: "db", source: "local",
    label: "Same", group: null, weight: 1, version: 1, hasNote: false, icon: "🚀",
  };
  const reIconed: GraphNode = { ...base, icon: "📚" };
  // The package omitted `icon`, so this returned true and the diff produced an
  // empty patch: an icon edit looked like no edit at all.
  assert.equal(nodesEqual(base, reIconed), false);
  assert.equal(nodesEqual(base, { ...base }), true);
  // Still ignores the lazy `fields` bag — unchanged contract.
  assert.equal(nodesEqual(base, { ...base, fields: { a: 1 } } as GraphNode), true);
});

test("diffGraph: an icon-only edit is an update, not a no-op (regression)", () => {
  const a = node("n1", "record", "db1", 1);
  const before = indexModel([a], []);
  const after = indexModel([{ ...a, icon: "🚀" }], []);
  // Without `icon` in nodesEqual this was an empty patch, so the renderer and
  // layout worker were never told the glyph changed.
  const patch = diffGraph(before, after);
  assert.equal(isEmptyPatch(patch), false);
  assert.equal(patch.updatedNodes.length, 1);
  assert.equal(patch.updatedNodes[0].icon, "🚀");
  assert.deepEqual(patch.addedNodes, []);
  assert.deepEqual(patch.removedNodeIds, []);
});

test("diffGraph: a same-id edge change is an update, not a no-op (regression)", () => {
  // An edge id is content-addressed over endpoints:kind:label only, so strength
  // and recordId live outside it. Without an updatedEdges list, changing either
  // produced an empty patch and `isEmptyPatch` told a consumer to skip the work —
  // silently dropping a real edit. `strength` is both the layout pull and the
  // rendered thickness; `recordId` is the row identity used for delete/demote.
  const before = indexModel([node("a", "record", "db1", 1), node("b", "record", "db1", 0.5)], [
    edge("e1", "a", "b", "relation", 0.4),
  ]);
  const restrung = indexModel([node("a", "record", "db1", 1), node("b", "record", "db1", 0.5)], [
    edge("e1", "a", "b", "relation", 2.9),
  ]);
  assert.equal(before.edges[0].id, restrung.edges[0].id, "precondition: the id is unchanged");

  const patch = diffGraph(before, restrung);
  assert.equal(isEmptyPatch(patch), false, "a real edge change must not read as empty");
  assert.equal(patch.updatedEdges.length, 1);
  assert.equal(patch.updatedEdges[0].strength, 2.9);
  assert.deepEqual(patch.addedEdges, [], "same id is an update, not an add");
  assert.deepEqual(patch.removedEdgeIds, [], "same id is an update, not a remove");

  // recordId changing on a stable id is the same class of miss.
  const reidentified = indexModel([node("a", "record", "db1", 1), node("b", "record", "db1", 0.5)], [
    { ...edge("e1", "a", "b", "relation", 0.4), recordId: "row-9" },
  ]);
  assert.equal(isEmptyPatch(diffGraph(before, reidentified)), false);

  // And an edge that genuinely did not change must still read as empty.
  assert.equal(isEmptyPatch(diffGraph(before, before)), true);
  // kind/label changes alter the id, so they were already visible as add+remove.
  assert.equal(
    isEmptyPatch(diffGraph(before, indexModel(before.nodes, [edge("e1", "a", "b", "hierarchy", 0.4)]))),
    false,
  );
});

test("diffGraph: add / remove / edge changes and the empty case", () => {
  const n1 = node("n1", "record", "db1", 1);
  const n2 = node("n2", "record", "db1", 0.5);
  const base = indexModel([n1], []);

  assert.equal(isEmptyPatch(diffGraph(base, base)), true, "identical models -> empty patch");

  const addedNodesOnly = diffGraph(base, indexModel([n1, n2], []));
  assert.deepEqual(addedNodesOnly.addedNodes.map((x) => x.id), ["n2"]);
  assert.deepEqual(addedNodesOnly.removedNodeIds, []);
  assert.deepEqual(addedNodesOnly.addedEdges, []);

  const removed = diffGraph(base, indexModel([], []));
  assert.deepEqual(removed.removedNodeIds, ["n1"]);

  const withEdge = indexModel([n1, n2], [edge("e1", "n1", "n2", "relation", 1)]);
  const added = diffGraph(base, withEdge);
  assert.equal(added.addedEdges.length, 1);
  assert.deepEqual(added.addedNodes.map((x) => x.id), ["n2"]);
  // ...and going back drops both again, so a patch is symmetric in both directions.
  const dropped = diffGraph(withEdge, base);
  assert.deepEqual(dropped.removedEdgeIds, ["e1"]);
  assert.deepEqual(dropped.removedNodeIds, ["n2"]);
  // Pre-existing `indexModel` behaviour, not part of this change: a dangling edge
  // is dropped at assembly, so the diff only ever walks ids the model holds.
  assert.equal(indexModel([n1], [edge("e1", "n1", "ghost", "relation", 1)]).edges.length, 0);
});
