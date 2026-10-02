/**
 * A 3D frame, painted. The claims are the ones the studio-3d browser gate cannot see from
 * outside: that the painter walks the depth order, that a node's drawn radius follows its
 * depth, and that a node behind the eye leaves no trace.
 *
 * The negative control is the first test: a 2D frame with a null `space` is drawn by the 2D
 * passes, and the z column is what turns it into something else. If the two ever met, the
 * studio's 2D drawing would change and every recorded 2D hash with it.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import { MIN_SCREEN_RADIUS } from "../src/canvas2d/nodes.ts";
import { type Frame, frameFrom } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { decodeSnapshot } from "../src/snapshot/decode.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Orbit, boxOf, fitOrbit } from "../src/three/orbit.ts";
import { newProjection, projectFrame } from "../src/three/projection.ts";
import { type Recorder, lineFrame, recorder, spaceBytes } from "./support.ts";

const VIEWPORT = { width: 1200, height: 1200 };
const NO_SPRITES = {
  get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0,
  beginFrame: () => undefined, starved: () => false, reset: () => undefined,
};

/** Four nodes along x, and a z that puts the last one furthest from the eye. */
function spaceFrame(z: readonly number[]): Frame {
  return {
    ...lineFrame({ x: [-150, -50, 50, 150], y: [0, 0, 0, 0], z, edges: [[0, 1], [2, 3]] }),
    nodeKind: "Circle",
    r: Float32Array.from([10, 10, 10, 10]),
  };
}

function inputFor(frame: Frame, record: Recorder, patch: Partial<PaintInput> = {}): PaintInput {
  const colours = Uint16Array.from({ length: frame.nodeCount }, (_, at) => at % 2);
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours, palette: ["red", "green"],
  });
  return {
    ctx: record.ctx, viewport: VIEWPORT, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: style.radius, settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount),
    selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES, space: null, ...patch,
  };
}

/** The fit that frames `frame`, as `showFrame` builds it when a 3D frame arrives. */
function orbitOf(frame: Frame) {
  return fitOrbit(boxOf(frame.x, frame.y, frame.z ?? new Float32Array(0)));
}

/** `frame` projected through `orbit`, as the loop would project it on a painted frame. */
function drawnOf(frame: Frame, orbit: Orbit) {
  return projectFrame(newProjection(frame.nodeCount), {
    frame, x: frame.x, y: frame.y, extent: frame.r ?? new Float32Array(frame.nodeCount), orbit, viewport: VIEWPORT,
  });
}

/** One column came back as the fixture wrote it, scaled: the claim the column order decides. */
function assertColumn(got: Float32Array, want: readonly number[], factor: number, name: string): void {
  assert.equal(got.length, want.length, `${name} has one entry per node`);
  for (const [at, value] of got.entries()) {
    const expected = (want[at] ?? 0) * factor;
    // The frame's columns are f32 and the factor is a f64, so this is close to, not equal to.
    assert.ok(Math.abs(value - expected) < 1e-4, `${name}[${at}] is ${value}, expected about ${expected}`);
  }
}

test("real 3D bytes decode into a frame the painter draws in depth order", () => {
  // The whole chain, from the bytes the motor writes: reader, frame, projection, painter.
  // Every other test here builds a `Frame` by hand, which is right for asking about the
  // painter and wrong for asking about the seam — a reader that read the z column as the
  // radius column would hand this painter a frame whose radii are depths, and a hand-built
  // frame's test would never see it. So the radii are the assertion: they are what the
  // column order decides.
  const X = [-150, -50, 50, 150];
  const Z = [0, 40, 80, 120];
  const RADII = [4, 4, 4, 4];
  const bytes = spaceBytes({ x: X, y: [0, 0, 0, 0], z: Z, r: RADII, edges: [[0, 1], [2, 3]] });
  const snapshot = decodeSnapshot(bytes);
  assert.equal(snapshot.dim, 1, "the reader took the fixture as 3D");

  const frame = frameFrom(snapshot);
  const z = frame.z;
  const r = frame.r;
  assert.ok(z !== null, "and the frame carries a z column");
  assert.ok(r !== null, "and its size column");
  // The frame scales every column by one factor; read it back off x so the assertions are
  // about the shape of the columns and not about the factor's value.
  const factor = (frame.x[1] ?? 0) / (X[1] ?? 0);
  assert.ok(factor > 0, `the frame's x is scaled, not raw: ${frame.x[1]}`);
  assert.deepEqual([...z].length, 4);
  // The negative control on the column order, in the frame itself: the z is the fixture's
  // depths and the radii are its sizes, and neither is the other's. A reader that forgot the
  // shift would come back with the z values as radii and the painter would draw depths as
  // sizes — which is what the loop below then measures.
  assertColumn(z, Z, factor, "z");
  assertColumn(r, RADII, factor, "r");

  const orbit = orbitOf(frame);
  const drawn = drawnOf(frame, orbit);
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record, { space: drawn }));
  assert.deepEqual([counts.nodes, counts.edges], [4, 2], "the decoded drawing is drawn whole");
  assert.deepEqual([...drawn.order], [0, 1, 2, 3], "furthest first, from the depths the bytes carried");
  assert.equal(record.calls.get("fill"), 4, "one fill per node, the 3D painter's own order");
  // And the drawn radii follow depth, not z: the furthest node draws narrowest, and every one
  // of them is a size rather than a coordinate.
  for (let at = 0; at < drawn.drawn; at += 1) {
    const node = drawn.order[at] ?? 0;
    const radius = drawn.radius[node] ?? 0;
    assert.ok(radius > 0, `node ${node} draws at radius ${radius}`);
  }
  const widest = drawn.radius[drawn.order[drawn.drawn - 1] ?? 0] ?? 0;
  const narrowest = drawn.radius[drawn.order[0] ?? 0] ?? 0;
  assert.ok(widest > narrowest, `the nearest draws ${widest} and the furthest ${narrowest}`);
});

test("a 2D frame has no z column, so the 3D painter is not reached", () => {
  // The negative control: this is the drawing every recorded 2D hash was taken from, and it
  // must still go through the batched 2D passes. A 3D path a 2D frame could reach would void
  // every one of them, so the assertion is on the batching, not only on the counts.
  // Positive x and a y off the top edge: the 2D camera here is at the origin at 1:1, so a
  // negative x is culled and this control would be measuring the cull rather than the batching.
  const frame = lineFrame({ x: [450, 550, 650, 750], y: [600, 600, 600, 600], edges: [[0, 1], [2, 3]] });
  assert.equal(frame.z, null, "a 2D frame has no z column at all");
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record));
  assert.deepEqual([counts.nodes, counts.edges], [4, 2]);
  // Batched: one fill per palette entry and one stroke for the edges, as the 2D path has
  // always done. The 3D painter fills per node, so it could not produce these numbers.
  assert.equal(record.calls.get("fill"), 2, "one fill per palette entry, not per node");
  assert.equal(record.calls.get("stroke"), 1);
});

test("a 3D frame is drawn furthest node first, so the nearest is painted over it", () => {
  const frame = spaceFrame([0, 40, 80, 120]);
  const orbit = orbitOf(frame);
  const drawn = drawnOf(frame, orbit);
  const record = recorder();
  paintFrame(inputFor(frame, record, { space: drawn }));
  // The fit puts the eye on the +z side, so the smallest z is the furthest node: node 0.
  assert.deepEqual([...drawn.order], [0, 1, 2, 3], "ascending depth, so the far end is first");
  assert.ok(orbit.distance > 0);
  // One fill per node, not one per palette entry: a batch cannot be depth-sorted.
  assert.equal(record.calls.get("fill"), 4);
});

test("a nearer node is drawn wider than a further one of the same size", () => {
  const frame = spaceFrame([0, 40, 80, 120]);
  const drawn = drawnOf(frame, orbitOf(frame));
  const near = drawn.radius[drawn.order[drawn.drawn - 1] ?? 0] ?? 0;
  const far = drawn.radius[drawn.order[0] ?? 0] ?? 0;
  assert.ok(near > far, `the nearest draws ${near} and the furthest ${far}`);
  assert.ok(far >= MIN_SCREEN_RADIUS, "and the furthest is still a dot worth drawing");
});

test("a node behind the eye is neither in the order nor on the canvas", () => {
  // The fit's own distance is the eye plane, so a z past it is behind. What remains must be
  // the rest of the graph, drawn normally.
  const flat = spaceFrame([0, 0, 0, 0]);
  const orbit = orbitOf(flat);
  const frame = spaceFrame([0, 0, 0, orbit.distance * 4]);
  const drawn = drawnOf(frame, orbit);
  assert.equal(drawn.drawn, 3, "three of the four are in front");
  assert.ok(![...drawn.order].includes(3), "the one behind is not in the order");
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record, { space: drawn }));
  assert.equal(counts.nodes, 3);
  assert.equal(record.calls.get("fill"), 3);
});

test("an edge is the straight line between its ends' projected points", () => {
  const frame = spaceFrame([0, 40, 80, 120]);
  const drawn = drawnOf(frame, orbitOf(frame));
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record, { space: drawn }));
  assert.equal(counts.edges, 2);
  assert.equal(counts.strokes, 1, "one stroke for the whole edge set, as the 2D flat path does");
  assert.equal(record.calls.get("lineTo"), 2);
});

test("an edge with an end behind the eye is not drawn", () => {
  // Node 1 is the one behind, and it is one end of the only edge that touches it: the other
  // edge's two ends are both in front, so it is drawn. One dropped, one kept.
  const orbit = orbitOf(spaceFrame([0, 0, 0, 0]));
  const frame = spaceFrame([0, orbit.distance * 4, 0, 0]);
  const drawn = drawnOf(frame, orbit);
  const counts = paintFrame(inputFor(frame, recorder(), { space: drawn }));
  assert.equal(counts.edges, 1, "only the edge whose two ends are in front is drawn");
  assert.equal(counts.nodes, 3);
});

test("the selected node gets its ring at its own projected radius", () => {
  const frame = spaceFrame([0, 40, 80, 120]);
  const drawn = drawnOf(frame, orbitOf(frame));
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record, { space: drawn, selected: 0 }));
  assert.equal(counts.nodes, 4);
  // Four fills, one ring stroke.
  assert.equal(record.calls.get("stroke"), 2);
});

test("a hidden node is not drawn and takes its edges with it", () => {
  const frame = spaceFrame([0, 40, 80, 120]);
  const drawn = drawnOf(frame, orbitOf(frame));
  const hidden = new Uint8Array(frame.nodeCount);
  hidden[0] = 1;
  const counts = paintFrame(inputFor(frame, recorder(), {
    space: drawn, style: styleFrom({
      labels: [], weights: new Float32Array(frame.nodeCount),
      colours: new Uint16Array(frame.nodeCount), palette: ["red"], hidden,
    }),
  }));
  assert.equal(counts.nodes, 3, "node 0 is not drawn");
  assert.equal(counts.edges, 1, "and the edge it was one end of is not either");
});

test("a frame with no nodes projects to nothing and paints nothing", () => {
  const frame = { ...spaceFrame([]), x: new Float32Array(0), y: new Float32Array(0), z: new Float32Array(0), nodeCount: 0, edgeCount: 0, r: new Float32Array(0) };
  const drawn = drawnOf(frame, orbitOf(frame));
  assert.equal(drawn.drawn, 0);
  const counts = paintFrame(inputFor(frame, recorder(), { space: drawn }));
  assert.deepEqual([counts.nodes, counts.edges, counts.draws], [0, 0, 0]);
});
