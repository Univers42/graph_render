/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   graph-engine-view.test.ts                          :+:      :+:    :+:   */
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
