/**
 * What the parity page reports: the drawn label names, the fill of every node, and the
 * centres the gate samples. The state is built from the scene and the fixture alone, so it
 * is pinned here without a browser.
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { readFixture } from "../src/parity/fixture.ts";
import { parityStateOf } from "../src/parity/page.ts";
import { parityScene } from "../src/parity/scene.ts";
import { lookOf, DEFAULT_PRESET } from "../../graph-render/src/look/presets.ts";

const URL_FIXTURE = new URL("../../../fixtures/scigraphs/lesmis.json", import.meta.url);

async function state(drawn: number) {
  const given = readFixture(await readFile(URL_FIXTURE, "utf8"));
  const scene = parityScene(given, lookOf(DEFAULT_PRESET));
  return { given, scene, state: parityStateOf(scene, given, drawn) };
}

test("the page reports the fig1 frame, the SciGraphs look and its background", async () => {
  const { state: reported } = await state(18);
  assert.equal(reported.look, "scigraphs");
  assert.equal(reported.width, 1920);
  assert.equal(reported.height, 1080);
  assert.equal(reported.background, "rgb(53, 53, 60)");
  assert.equal(reported.nodes.length, 77);
});

test("the reported label names are the fixture's 18, and the count is the drawn one", async () => {
  const { given, state: reported } = await state(18);
  assert.equal(reported.drawn, 18);
  assert.deepEqual(
    [...reported.labels].sort(),
    given.labels.map((at) => given.nodes[at]?.label ?? "").sort(),
  );
  assert.deepEqual(reported.labelNodes, [...reported.labelNodes].sort((a, b) => a - b));
  // A frame that never painted reports -1, never a silent 18.
  assert.equal((await state(-1)).state.drawn, -1);
});

test("every node is reported with its own centre, radius and fill", async () => {
  const { given, state: reported } = await state(18);
  for (const node of given.nodes) {
    const reportedNode = reported.nodes[node.id];
    assert.ok(reportedNode, `node ${node.id}`);
    assert.equal(reportedNode.x, Math.fround(node.screen[0]));
    assert.equal(reportedNode.y, Math.fround(node.screen[1]));
    assert.ok(reportedNode.r > 22 && reportedNode.r < 46, `radius ${reportedNode.r}`);
    assert.match(reportedNode.fill, /^rgb\(\d+, \d+, \d+\)$/);
    assert.equal(reportedNode.label, node.label);
    assert.equal(reportedNode.betweenness, node.betweenness);
  }
});

test("the reported label boxes are where the sprite bake puts them, on the node", async () => {
  const { given, state: reported } = await state(18);
  assert.equal(reported.labelRects.length, 18);
  for (const rect of reported.labelRects) {
    const node = given.nodes[rect.id];
    assert.ok(node, `rect for node ${rect.id}`);
    assert.equal(rect.label, node.label);
    // 0.30 * 26 per character is the source's own estimate, 3 px of padding on each side.
    const estimate = 0.3 * 26 * Array.from(rect.label).length;
    assert.equal(rect.width, Math.ceil(estimate) + 6);
    assert.equal(rect.height, 38);
    assert.equal(rect.x, Math.fround(node.screen[0]) - rect.width / 2);
    assert.equal(rect.y, Math.fround(node.screen[1]) - 19);
  }
  // A measurer of its own overrides the estimate, which is what the page hands over.
  const wide = parityStateOf(parityScene(given, lookOf(DEFAULT_PRESET)), given, 18, () => 100);
  assert.deepEqual(new Set(wide.labelRects.map((rect) => rect.width)), new Set([106]));
});

test("the 43 zero-betweenness nodes report one fill, and only Napoleon is covered", async () => {
  const { state: reported } = await state(18);
  const zero = reported.nodes.filter((node) => node.betweenness === 0);
  assert.equal(zero.length, 43);
  assert.equal(new Set(zero.map((node) => node.fill)).size, 1);
  assert.equal(zero[0]?.fill, "rgb(166, 79, 176)");
  assert.deepEqual(zero.filter((node) => node.covered).map((node) => node.label), ["Napoleon"]);
  assert.equal(reported.nodes.filter((node) => node.covered).length, 18);
});
