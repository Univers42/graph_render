/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine.test.ts                               :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  +:+       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/06/08 12:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/06/08 12:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

import assert from "node:assert/strict";
import test from "node:test";

import type { GraphEdge, GraphNode } from "../src/core/types.ts";
import { clamp, lerp, smoothstep } from "../src/core/math.ts";
import { makeEdgeId, makeRecordNodeId, parseNodeId } from "../src/core/model/ids.ts";
import { emptyModel, indexModel, nodesEqual } from "../src/core/model/model.ts";
import { applyDegreeWeights, weightToRadius } from "../src/core/model/weights.ts";
import { neighborhood, neighborhoodEdges } from "../src/core/model/neighborhood.ts";
import { buildSyntheticModel } from "../src/core/model/synthetic.ts";
import { edgeKindFromType } from "../src/core/model/edgeKind.ts";
import { diffGraph, isEmptyPatch } from "../src/core/model/diff.ts";
import { deriveLegend } from "../src/core/model/legend.ts";
import { screenToWorld, worldToScreen } from "../src/core/camera/transform.ts";
import { fitBounds, zoomAt } from "../src/core/camera/controls.ts";
import { strengthTier, tierWidth } from "../src/core/render/tiers.ts";
import { edgeStroke, nodeFill } from "../src/core/theme/colors.ts";
import { DARK_THEME } from "../src/core/theme/tokens.ts";
import { cloneControls, DEFAULT_CONTROLS } from "../src/core/state/controls.ts";
import { SceneState } from "../src/core/render/sceneState.ts";
import { parseStyleKey, shapeOf, styleKey } from "../src/core/render/nodeShape.ts";
import { darken, lighten, mix, rgba } from "../src/core/theme/shade.ts";
import { sceneToSvg } from "../src/core/render/exportSvg.ts";
import { ForceLayout } from "../src/core/layout/forceLayout.ts";

function node(id: string, kind: GraphNode["kind"], databaseId: string | null, weight = 0): GraphNode {
  return { id, kind, databaseId, source: "db", label: id, group: null, weight, version: 0, hasNote: false };
}
function edge(id: string, source: string, target: string, kind: GraphEdge["kind"], strength = 1): GraphEdge {
  return { id, source, target, kind, label: kind, strength, directed: true };
}

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

test("neighborhood collects the node + its 1-hop ring", () => {
  const model = indexModel(
    [node("a", "record", "db1"), node("b", "record", "db1"), node("c", "record", "db1"), node("d", "record", "db1")],
    [edge("e1", "a", "b", "relation"), edge("e2", "a", "c", "relation")],
  );
  const set = neighborhood(model, "a", 1);
  assert.ok(set.has("a") && set.has("b") && set.has("c"));
  assert.ok(!set.has("d"));
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

test("camera: world<->screen inverse, zoom keeps the cursor point fixed", () => {
  const cam = { x: 30, y: -10, scale: 1.5 };
  const s = worldToScreen(cam, 12, 8);
  const w = screenToWorld(cam, s.x, s.y);
  assert.ok(Math.abs(w.x - 12) < 1e-9 && Math.abs(w.y - 8) < 1e-9);
  const fixed = screenToWorld(cam, 200, 150);
  const after = worldToScreen(zoomAt(cam, 200, 150, 1.8), fixed.x, fixed.y);
  assert.ok(Math.abs(after.x - 200) < 1e-6 && Math.abs(after.y - 150) < 1e-6);
});

test("fitBounds centers the bounds in the viewport", () => {
  const cam = fitBounds({ minX: 0, minY: 0, maxX: 100, maxY: 100 }, 800, 600, 0);
  const center = worldToScreen(cam, 50, 50);
  assert.ok(Math.abs(center.x - 400) < 1e-6 && Math.abs(center.y - 300) < 1e-6);
});

test("link tiers grow with strength and linkScale", () => {
  assert.equal(strengthTier(0.1), 0);
  assert.equal(strengthTier(3), 3);
  assert.ok(tierWidth(0, 3, 1) > tierWidth(0, 0, 1));
  assert.ok(tierWidth(0, 1, 2) > tierWidth(0, 1, 1));
});

test("colors: tag override + note hue + edge stroke by kind", () => {
  const tagColors = new Map([["urgent", "#ff0000"]]);
  assert.equal(nodeFill({ kind: "tag", label: "urgent", databaseId: null, source: "db" }, tagColors), "#ff0000");
  assert.notEqual(nodeFill({ kind: "note", label: "n", databaseId: null, source: "db" }), nodeFill({ kind: "record", label: "r", databaseId: "db1", source: "db" }));
  assert.equal(edgeStroke(DARK_THEME, "tag"), DARK_THEME.edgeTag);
  assert.equal(edgeStroke(DARK_THEME, "hierarchy"), DARK_THEME.edgeHierarchy);
});

test("cloneControls is a deep, independent copy", () => {
  const copy = cloneControls(DEFAULT_CONTROLS);
  copy.filter.hiddenKinds.push("tag");
  copy.visual.glow = 9;
  assert.equal(DEFAULT_CONTROLS.filter.hiddenKinds.length, 0);
  assert.notEqual(DEFAULT_CONTROLS.visual.glow, 9);
});

test("SceneState builds buffers, groups edges, and applies filters", () => {
  const model = indexModel(
    [node("a", "record", "db1", 1), node("b", "record", "db1", 0.5), node("t", "tag", null, 0.2)],
    [edge("e1", "a", "b", "relation", 1), edge("e2", "a", "t", "tag", 0.6)],
  );
  const state = new SceneState();
  state.setGraph(model);
  assert.equal(state.count, 3);
  assert.equal(state.edgeFrom.length, 2);
  const grouped = state.edgeGroups.reduce((sum, group) => sum + group.length, 0);
  assert.equal(grouped, 2);
  state.applyFilters({ hiddenDatabases: ["db1"], hiddenKinds: [], hiddenTags: [], tagColors: {} });
  const aIndex = state.idToIndex.get("a");
  const tIndex = state.idToIndex.get("t");
  assert.ok(aIndex !== undefined);
  assert.ok(tIndex !== undefined);
  assert.equal(state.visible[aIndex], 0);
  assert.equal(state.visible[tIndex], 1); // tag has no databaseId
});

test("nodeShape: shapeOf maps kinds + styleKey round-trips", () => {
  assert.equal(shapeOf("record"), "disc");
  assert.equal(shapeOf("database"), "disc");
  assert.equal(shapeOf("tag"), "ring");
  assert.equal(shapeOf("note"), "note");
  const parsed = parseStyleKey(styleKey("ring", "oklch(0.7 0.04 255)"));
  assert.equal(parsed.shape, "ring");
  assert.equal(parsed.color, "oklch(0.7 0.04 255)"); // color may itself contain no "|"
});

test("shade: tone math the node materials light from", () => {
  const base: [number, number, number] = [100, 150, 200];
  // Endpoints are exact, so a body ramp never drifts off the node's own hue.
  assert.deepEqual(mix(base, [0, 0, 0], 0), base);
  assert.deepEqual(lighten(base, 1), [255, 255, 255]);
  assert.deepEqual(darken(base, 1), [0, 0, 0]);
  // Lighten brightens every channel, darken dims every channel (no hue inversion).
  const lit = lighten(base, 0.5);
  const dim = darken(base, 0.5);
  base.forEach((c, i) => {
    assert.ok(lit[i] > c, `lighten raised channel ${i}`);
    assert.ok(dim[i] < c, `darken lowered channel ${i}`);
  });
  // Clamped: an out-of-range t can't produce an invalid color.
  assert.deepEqual(lighten(base, 5), [255, 255, 255]);
  assert.deepEqual(darken(base, -5), base);
  assert.equal(rgba(base, 0), "rgba(100, 150, 200, 0)"); // the halo's fade-out stop
});

test("SceneState.styleBuckets groups by shape|color", () => {
  const model = indexModel(
    [node("a", "record", "db1", 1), node("b", "record", "db1", 0.5), node("t", "tag", null, 0.2)],
    [edge("e1", "a", "b", "relation", 1)],
  );
  const state = new SceneState();
  state.setGraph(model);
  const keys = [...state.styleBuckets.keys()];
  assert.ok(keys.some((k) => k.startsWith("ring|")), "tag renders as a ring");
  const discBuckets = keys.filter((k) => k.startsWith("disc|"));
  const discTotal = discBuckets.reduce((n, k) => n + (state.styleBuckets.get(k)?.length ?? 0), 0);
  assert.equal(discTotal, 2); // two same-db records share one disc bucket
});

test("sceneToSvg emits a well-formed SVG with nodes and edges", () => {
  const model = indexModel(
    [node("a", "record", "db1", 1), node("b", "record", "db1", 0.5)],
    [edge("e1", "a", "b", "relation", 1)],
  );
  const state = new SceneState();
  state.setGraph(model);
  state.posX[0] = 0;
  state.posY[0] = 0;
  state.posX[1] = 50;
  state.posY[1] = 30;
  const svg = sceneToSvg(state, DARK_THEME, DEFAULT_CONTROLS.visual);
  assert.ok(svg.startsWith("<svg"));
  assert.ok(svg.includes("<circle"));
  assert.ok(svg.includes("<line"));
});

test("ForceLayout converges to finite, spread positions", () => {
  const layout = new ForceLayout({
    count: 3,
    links: [
      { source: 0, target: 1, strength: 1 },
      { source: 1, target: 2, strength: 1 },
    ],
    width: 200,
    height: 200,
  });
  for (let i = 0; i < 80; i += 1) layout.tick();
  const x = new Float32Array(3);
  const y = new Float32Array(3);
  layout.readPositions(x, y);
  for (let i = 0; i < 3; i += 1) {
    assert.ok(Number.isFinite(x[i]) && Number.isFinite(y[i]), "positions are finite");
  }
  const spread = Math.hypot(x[0] - x[2], y[0] - y[2]);
  assert.ok(spread > 1, "endpoints separate under the link force");
});

// --- Extraction additions -------------------------------------------------
// These lock in behaviour that was WRONG in the package and correct in the host
// app's hand-mirrored copy. Each one is a regression guard, not a new feature:
// if the package's copy is ever re-derived from the app's, the diff that broke
// it is visible here.

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

  // Must agree with the nodes-only view — they share one traversal.
  assert.deepEqual([...neighborhood(model, "a", 2)].sort(), [...far.nodeIds].sort());
});

test("synthetic: buildSyntheticModel is deterministic and respects its cap", () => {
  const a = buildSyntheticModel(64);
  const b = buildSyntheticModel(64);
  // Seeded PRNG: identical inputs must give byte-identical models, or every
  // benchmark run and screenshot comparison is noise.
  assert.deepEqual(
    a.nodes.map((n) => `${n.id}:${n.kind}:${n.label}`),
    b.nodes.map((n) => `${n.id}:${n.kind}:${n.label}`),
  );
  assert.equal(a.nodes.length, 64);
  // Indexes are built, not just the raw arrays.
  assert.equal(a.nodeById.size, 64);
  assert.ok(a.edges.length > 0);
  // Floor of 2, ceiling 100k (the resident per-node model's limit).
  assert.equal(buildSyntheticModel(0).nodes.length, 2);
  assert.equal(buildSyntheticModel(1).nodes.length, 2);
});

test("edgeKindFromType: wire type -> internal kind, unknown falls back to relation", () => {
  assert.equal(edgeKindFromType("parent"), "hierarchy");
  assert.equal(edgeKindFromType("parent_of"), "hierarchy");
  assert.equal(edgeKindFromType("child_of"), "hierarchy");
  assert.equal(edgeKindFromType("page_hierarchy"), "hierarchy");
  assert.equal(edgeKindFromType("note_link"), "note_link");
  assert.equal(edgeKindFromType("links_to"), "note_link");
  assert.equal(edgeKindFromType("note_of"), "note_of");
  assert.equal(edgeKindFromType("annotates"), "note_of");
  assert.equal(edgeKindFromType("tagged"), "tag");
  assert.equal(edgeKindFromType("tag"), "tag");
  assert.equal(edgeKindFromType("tagged_by"), "tag"); // substring match, case-insensitive
  assert.equal(edgeKindFromType("Tags"), "tag");
  // Everything unrecognized is a plain structural relation — this is the
  // default every relation-property edge lands on, so it must be the fallback.
  assert.equal(edgeKindFromType("assignee"), "relation");
  // Branch order is observable and worth pinning: the hierarchy test is an exact
  // match on three literals plus a "hierarchy" substring, NOT a "parent"
  // substring — so a compound type containing "tag" falls through to the tag
  // branch. Documented here so a future "wider match" change is deliberate.
  assert.equal(edgeKindFromType("parent_tag"), "tag");
  assert.equal(edgeKindFromType("parent_hierarchy"), "hierarchy");
  assert.equal(edgeKindFromType("project"), "relation");
  assert.equal(edgeKindFromType(undefined), "relation");
  assert.equal(edgeKindFromType(""), "relation");
});

test("parseNodeId: the read half round-trips the write half", () => {
  const id = makeRecordNodeId("db", "people", "rec-42");
  assert.deepEqual(parseNodeId(id), { mount: "db", resource: "people", pk: "rec-42" });
  // pk may itself contain ':' — everything after the 2nd segment must rejoin,
  // otherwise a composite primary key silently truncates and the id addresses
  // a different record than it was built for.
  const colonPk = makeRecordNodeId("db", "people", "a:b:c");
  assert.deepEqual(parseNodeId(colonPk), { mount: "db", resource: "people", pk: "a:b:c" });
  // Degenerate input must not throw.
  assert.deepEqual(parseNodeId("bare"), { mount: "bare", resource: "", pk: "" });
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
  // An edge whose endpoint is absent is dropped by indexModel, so it can never
  // appear in a patch — the diff only ever walks ids the model actually holds.
  assert.equal(indexModel([n1], [edge("e1", "n1", "ghost", "relation", 1)]).edges.length, 0);
});
