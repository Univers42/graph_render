/**
 * The SciGraphs reference fixture, read and pinned. It is reference data, not a gate input
 * (docs/decisions/scigraphs-reference-fixture.md): what it carries is SciGraphs' own
 * projection, its RANK colour coordinate and its label set, and the parity page draws the
 * positions it names without running a layout of its own.
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { type Fixture, type FixtureNode, FIXTURE_PATH, readFixture } from "../src/parity/fixture.ts";

const URL_FIXTURE = new URL("../../../fixtures/scigraphs/lesmis.json", import.meta.url);

async function fixture() {
  return readFixture(await readFile(URL_FIXTURE, "utf8"));
}

/** One node of a loaded fixture, or a failure that names the index that is missing. */
function nodeAt(given: Fixture, at: number): FixtureNode {
  const found = given.nodes[at];
  assert.ok(found, `no node at ${at}`);
  return found;
}

test("the fixture is the 77-node Les Miserables network the gallery figure draws", async () => {
  const loaded = await fixture();
  assert.equal(loaded.nodes.length, 77);
  assert.equal(loaded.edges.length, 254);
  // Nodes are networkx's order, ids 0..76 (scigraphs-reference-fixture.md, "node order").
  assert.equal(nodeAt(loaded, 0).id, 0);
  assert.equal(nodeAt(loaded, 76).id, 76);
  assert.equal(nodeAt(loaded, 0).label, "Napoleon");
  assert.equal(nodeAt(loaded, 1).label, "Myriel");
});

test("its params are the gallery's own: 1920x1080, 60 mm, 0.022 R nodes, 18 labels", async () => {
  const { params } = await fixture();
  assert.deepEqual(params.resolution, [1920, 1080]);
  assert.equal(params.camera_lens_mm, 60);
  assert.equal(params.camera_sensor_mm, 36);
  assert.equal(params.camera_margin, 1.12);
  assert.equal(params.node_radius_rel, 0.022);
  assert.equal(params.edge_radius_rel, 0.0035);
  assert.equal(params.label_font_size, 26);
  assert.equal(params.label_max_count, 18);
  assert.equal(params.color_norm, "RANK");
});

test("the 43 zero-betweenness nodes all carry the same RANK coordinate, 21/76", async () => {
  const { nodes } = await fixture();
  const zero = nodes.filter((node) => node.betweenness === 0);
  assert.equal(zero.length, 43);
  for (const node of zero) assert.equal(node.t, 21 / 76);
  assert.equal(zero[0]?.t, 0.27631578947368424);
  // The highest betweenness in the graph is the whole range's top, t = 1.
  assert.equal(nodes.reduce((top, node) => Math.max(top, node.t), 0), 1);
});

test("the fixture names the eighteen labels of its own pipeline", async () => {
  const { labels, nodes } = await fixture();
  assert.equal(labels.length, 18);
  assert.deepEqual(labels.map((at) => nodes[at]?.label), [
    "Myriel", "Marius", "Fantine", "Javert", "MlleGillenormand", "Enjolras", "Tholomyes",
    "Mabeuf", "Fauchelevent", "MmeBurgon", "Gillenormand", "Simplice", "Gueulemer", "Joly",
    "Combeferre", "MmePontmercy", "Magnon", "Napoleon",
  ]);
  // Every index is a node, and no node is named twice.
  assert.equal(new Set(labels).size, 18);
});

test("the radii are the source's own multipliers of R", async () => {
  const { radii, params } = await fixture();
  assert.equal(radii.node, params.node_radius_rel * radii.R);
  assert.equal(radii.edge, params.edge_radius_rel * radii.R);
  assert.ok(radii.R > 6 && radii.R < 7);
});

test("a document that is not this shape is refused rather than half-read", () => {
  assert.throws(() => readFixture("not json"), /scigraphs fixture/);
  assert.throws(() => readFixture("{}"), /nodes/);
  assert.throws(() => readFixture(JSON.stringify({ nodes: [] })), /nodes/);
  const broken = {
    nodes: [{ id: 0, label: "a", betweenness: 0, t: 0, world: [0, 0, 0], screen: [0, 0], depth: 1 }],
    edges: [[0, 5]], labels: [], params: {}, radii: {},
  };
  assert.throws(() => readFixture(JSON.stringify(broken)), /edge/);
});

test("the path the page fetches is the one the studio stages", () => {
  assert.equal(FIXTURE_PATH, "fixtures/scigraphs/lesmis.json");
});
