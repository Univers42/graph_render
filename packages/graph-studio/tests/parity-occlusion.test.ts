/**
 * The label pipeline's first two stages, in the order the reference runs them: a node
 * hidden behind another one's glyph is not a candidate at all, and the declutter never
 * sees it. The ray is the source's own, against the same spheres, with the same origin
 * offset and the same tolerance.
 *   SciGraphs/core/visualization/text_overlay.py:250-299 (test_depth_occlusion)
 *   harness/scigraphs_lesmis_camera.py:195-227 (the port this follows, line for line)
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { readFixture, type Fixture, type FixtureParams } from "../src/parity/fixture.ts";
import { cameraLocation, candidatesOf } from "../src/parity/occlusion.ts";

const URL_FIXTURE = new URL("../../../fixtures/scigraphs/lesmis.json", import.meta.url);

async function fixture(): Promise<Fixture> {
  return readFixture(await readFile(URL_FIXTURE, "utf8"));
}

/** The params of a toy cloud: the camera on the +z axis looking at the origin. */
function params(): FixtureParams {
  return {
    resolution: [1920, 1080], camera_lens_mm: 60, camera_sensor_mm: 36, camera_margin: 1.12,
    camera_distance: 0, node_radius_rel: 0.022, edge_radius_rel: 0.0035, label_font_size: 26,
    label_max_count: 18, color_norm: "RANK",
    bbox_lo: [0, 0, 0], bbox_hi: [0, 0, 1], camera_forward: [0, 0, -1],
  };
}

/** A node on the view axis, `depth` in front of the camera, with a `radius` glyph. */
function nodeAt(id: number, depth: number): Fixture["nodes"][number] {
  return { id, label: `n${id}`, betweenness: 0, t: 0, world: [0, 0, -depth], screen: [0, 0], depth };
}

function sceneOf(nodes: Fixture["nodes"], radius: number): Fixture {
  return { nodes, edges: [], labels: [], params: params(), radii: { R: radius / 0.022, node: radius, edge: radius / 6 } };
}

/** Two nodes on the view axis, the second behind the first. */
function pair(first: number, second: number, radius: number): Fixture {
  return sceneOf([nodeAt(0, first), nodeAt(1, second)], radius);
}

test("the camera sits on the bbox centre, pushed back along its own forward axis", async () => {
  const { params } = await fixture();
  const [x, y, z] = cameraLocation(params);
  assert.equal(x, 8.441304824814255);
  assert.equal(y, -10.716683547225491);
  assert.equal(z, 7.789653354730376);
});

test("the fixture's cloud hides 12 of its 77 nodes behind their own glyphs", async () => {
  const given = await fixture();
  const mask = candidatesOf(given);
  assert.equal(mask.length, 77);
  // The source's own report: 77 projected, 77 in frame, 65 not hidden (fixture.md).
  assert.deepEqual(
    [...mask].map((at, node) => (at === 0 ? node : -1)).filter((node) => node >= 0),
    [10, 12, 20, 24, 26, 35, 44, 48, 62, 64, 71, 72],
  );
});

test("a node behind another one is hidden, and the nearer one is not", () => {
  // Two glyphs on the view axis, the second twice as far: the near one hides the far one.
  assert.deepEqual([...candidatesOf(pair(5, 10, 0.14492357067601658))], [1, 0]);
  // With the far one beside the near one in depth, neither is in front of the other.
  assert.deepEqual([...candidatesOf(pair(5, 5, 0.14492357067601658))], [1, 1]);
});

test("a node is never hidden by its own glyph, whatever the glyph's radius", () => {
  // The ray meets its own sphere at r + 0.01 before the node, and the tolerance it has to
  // beat is max(0.1, 1.5 r): the floor beats r + 0.01 up to r = 0.09, and 1.5 r beats it
  // above that, so the two constants between them cover every radius there is.
  for (const radius of [0.001, 0.05, 0.09, 0.14492357067601658, 1, 100]) {
    assert.deepEqual([...candidatesOf(sceneOf([nodeAt(0, 5)], radius))], [1], `radius ${radius}`);
  }
});

test("a graph with no node has no candidates", async () => {
  const given = await fixture();
  assert.deepEqual([...candidatesOf({ ...given, nodes: [] })], []);
});
