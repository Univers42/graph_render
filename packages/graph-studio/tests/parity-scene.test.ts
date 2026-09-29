/**
 * The parity page's scene: the fixture's own screen positions, the SciGraphs look, and the
 * label set the reference pipeline computed. The camera is the identity, so a node's
 * screen coordinates are its world coordinates and a frame lands on the fig1 pixels.
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { readFixture, type Fixture } from "../src/parity/fixture.ts";
import { parityScene } from "../src/parity/scene.ts";
import { DEFAULT_PRESET, LABEL_LIMIT, lookOf } from "../../graph-render/src/look/presets.ts";
import { sampleColormap } from "../../graph-render/src/colour/colormap.ts";

/** lens * res_x / sensor, the sensor-plane scale of text_overlay.py:182-187. */
const SCALE = (60 * 1920) / 36;

const URL_FIXTURE = new URL("../../../fixtures/scigraphs/lesmis.json", import.meta.url);

async function scene(): Promise<{ given: Fixture; built: ReturnType<typeof parityScene> }> {
  const given = readFixture(await readFile(URL_FIXTURE, "utf8"));
  return { given, built: parityScene(given, lookOf(DEFAULT_PRESET)) };
}

test("the frame is the fixture's own pixels, 77 circles and 254 curves, with no bounds", async () => {
  const { given, built } = await scene();
  const { frame } = built;
  assert.equal(frame.nodeCount, 77);
  assert.equal(frame.edgeCount, 254);
  assert.equal(frame.nodeKind, "Circle");
  // 05-reproducible-pipeline.qmd:660 asks for CYTOSCAPE_BEZIER, so the edges are curves.
  assert.equal(frame.edgeKind, "Curve");
  assert.equal(frame.curveDegree, 2);
  assert.equal(frame.offsets?.length, 255);
  assert.equal(frame.pts?.length, 508);
  // No bounds: the parity camera is the identity, and a refit after a resize has to land
  // back on it, which it only can while the frame carries nothing to fit.
  assert.equal(frame.bounds, null);
  assert.equal(frame.factor, 1);
  assert.equal(frame.x[0], Math.fround(given.nodes[0]?.screen[0] ?? 0));
  assert.equal(frame.y[0], Math.fround(given.nodes[0]?.screen[1] ?? 0));
  assert.equal(frame.source[0], 0);
  assert.equal(frame.target[0], 1);
});

test("a node's radius on screen is 0.022 R through the sensor projection", async () => {
  const { given, built } = await scene();
  const radii = built.frame.r;
  assert.ok(radii, "the frame carries a radius per node");
  const first = given.nodes[0];
  assert.ok(first, "the fixture has a first node");
  const [head] = radii;
  assert.equal(head, Math.fround((given.radii.node * SCALE) / first.depth));
  // 23.1 px at the fixture's first node, and never less than the 16 px of a nearer one.
  assert.equal(Math.round(head), 23);
  const widest = Math.max(...radii);
  assert.ok(widest > 40 && widest < 50, `widest radius ${widest}`);
});

test("the edge stroke is the tube's diameter at the camera's own distance", async () => {
  const { given, built } = await scene();
  const { params } = given;
  const width = 2 * given.radii.edge * ((params.camera_lens_mm * params.resolution[0]) / (params.camera_sensor_mm * params.camera_distance));
  assert.equal(built.style.edgeWidth, width);
  assert.ok(width > 9 && width < 9.5, `edge width ${width}`);
});

test("the camera is the identity and the theme is the SciGraphs look", async () => {
  const { built } = await scene();
  assert.deepEqual(built.camera, { x: 0, y: 0, scale: 1 });
  assert.equal(built.theme.background, "rgb(53, 53, 60)");
  assert.equal(built.theme.edge, "rgb(111, 111, 121)");
  assert.equal(built.policy.budget, LABEL_LIMIT);
  assert.equal(built.policy.budget, 18);
  assert.deepEqual([built.params.resolution], [[1920, 1080]]);
});

test("the drawn label set is the fixture's own 18, chosen by the library's own pipeline", async () => {
  const { given, built } = await scene();
  assert.equal(built.labelled.length, 18);
  // The scene reports the set in node order and the fixture in ranking order.
  assert.deepEqual([...built.labelled].sort((a, b) => a - b), [...given.labels].sort((a, b) => a - b));
  const names = (nodes: readonly number[]) =>
    nodes.map((at) => given.nodes[at]?.label ?? "").sort();
  assert.deepEqual(names(built.labelled), names(given.labels));
  assert.equal(built.style.labels.filter((text) => text !== "").length, 18);
});

test("the 43 zero-betweenness nodes share one fill, the colormap at 21/76", async () => {
  const { built } = await scene();
  assert.equal(built.zero.length, 43);
  const fills = new Set(built.zero.map((node) => built.style.palette[built.style.colours[node] ?? 0]));
  assert.equal(fills.size, 1);
  // 32-stop ramp, inferno, t = 21/76 (colormaps.py:527-530 mid-rank over 77 samples).
  assert.equal(built.zeroFill, "rgb(166, 79, 176)");
  assert.equal(built.zeroFill, `rgb(${sampleColormap("inferno", 21 / 76).map((c) => Math.round(255 * encode(c))).join(", ")})`);
});

test("one of the zero-betweenness nodes carries a label, so its centre is not sampled", async () => {
  const { given, built } = await scene();
  const labelledZero = built.zero.filter((node) => built.style.labels[node] !== "");
  assert.deepEqual(labelledZero.map((node) => given.nodes[node]?.label), ["Napoleon"]);
});

test("every node's colour is its own t, and the palette has one entry per distinct value", async () => {
  const { given, built } = await scene();
  const distinct = new Set(given.nodes.map((node) => node.t));
  assert.equal(distinct.size, 32);
  assert.equal(built.style.palette.length, distinct.size);
  for (const node of given.nodes) {
    const fill = built.style.palette[built.style.colours[node.id] ?? 0] ?? "";
    assert.equal(fill, fillOf(node.t), `node ${node.id} t ${node.t}`);
  }
});

/** The css the library spells a colour coordinate in, recomputed here independently. */
function fillOf(t: number): string {
  const rgb = sampleColormap("inferno", t);
  return `rgb(${rgb.map((c) => Math.round(255 * encode(c))).join(", ")})`;
}

/** The sRGB transfer function of colour/srgb.ts, kept local so the pin is independent. */
function encode(linear: number): number {
  return linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
}
