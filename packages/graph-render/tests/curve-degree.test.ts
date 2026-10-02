/**
 * A `Curve` frame of every degree the contract admits (binary-layout.md:112, "at least 1").
 * Degree 2 and 3 keep the exact canvas calls the painter has always issued; degree 4 and up
 * are flattened to chords that lie on the curve, which is not the same thing as the chords
 * through its own control points.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { paintEdges } from "../src/canvas2d/edges.ts";
import { newCounts, type PaintInput } from "../src/canvas2d/input.ts";
import type { Surface2D } from "../src/canvas2d/surface.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };
const EDGES = { scale: 1, curve: false, arrows: false };
/**
 * The chord count `edges2d/curve.ts` flattens a general-degree curve to, pinned here so
 * raising the heuristic is a deliberate edit on both sides (the escape hatch it names).
 */
const CHORDS = 16;
/** Camera is the identity and the viewport holds every point below, so screen = world. */
const FROM = { x: 20, y: 20 };
const TO = { x: 320, y: 20 };

/** One edge of `degree`, with the interior points a writer stored for it. */
function curveFrame(degree: number, interior: readonly (readonly [number, number])[]): Frame {
  return {
    nodeKind: "Point", edgeKind: "Curve", nodeCount: 2, edgeCount: 1,
    x: Float32Array.from([FROM.x, TO.x]), y: Float32Array.from([FROM.y, TO.y]),
    z: null, r: null, w: null, h: null,
    source: Uint32Array.from([0]), target: Uint32Array.from([1]),
    curveDegree: degree, offsets: Uint32Array.from([0, interior.length]),
    pts: Float32Array.from(interior.flat()),
    bounds: { minX: 20, minY: 20, maxX: 320, maxY: 220 }, factor: 1,
  };
}

function inputFor(frame: Frame, record: Recorder): PaintInput {
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours: new Uint16Array(frame.nodeCount),
    palette: ["red"], edges: EDGES,
  });
  return {
    ctx: record.ctx, viewport: { width: 400, height: 400 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: style.radius, settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount),
    selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES,
  };
}

interface Path {
  readonly record: Recorder;
  readonly moves: number[][];
  readonly lines: number[][];
}

/** The recorder's own counters, and where every point it was handed actually went. */
function traced(): Path {
  const record = recorder();
  const moves: number[][] = [];
  const lines: number[][] = [];
  const ctx = record.ctx as Surface2D;
  const lineTo = ctx.lineTo.bind(ctx);
  const moveTo = ctx.moveTo.bind(ctx);
  ctx.lineTo = (x: number, y: number): void => void (lines.push([x, y]), lineTo(x, y));
  ctx.moveTo = (x: number, y: number): void => void (moves.push([x, y]), moveTo(x, y));
  return { record, moves, lines };
}

/** The edge pass alone, so a node's own path is never counted as an edge chord. */
function painted(frame: Frame): Path {
  const path = traced();
  paintEdges(inputFor(frame, path.record), newCounts());
  return path;
}

/** Distance from a point to the segment a-b, so "off the polygon" is off every one of its sides. */
function toSegment(p: readonly number[], a: readonly number[], b: readonly number[]): number {
  const ex = (b[0] ?? 0) - (a[0] ?? 0);
  const ey = (b[1] ?? 0) - (a[1] ?? 0);
  const px = (p[0] ?? 0) - (a[0] ?? 0);
  const py = (p[1] ?? 0) - (a[1] ?? 0);
  const span = ex * ex + ey * ey;
  const at = span === 0 ? 0 : Math.min(1, Math.max(0, (px * ex + py * ey) / span));
  return Math.hypot(px - at * ex, py - at * ey);
}

/** How far each drawn point sits from the polyline through `corners`. */
function fromPolygon(lines: readonly number[][], corners: readonly (readonly [number, number])[]): number[] {
  const vertices = [[FROM.x, FROM.y], ...corners, [TO.x, TO.y]];
  return lines.map((p) => Math.min(...vertices.slice(1).map((b, at) => toSegment(p, vertices[at] ?? p, b))));
}

function near(p: readonly number[] | undefined, x: number, y: number): boolean {
  return p !== undefined && Math.abs((p[0] ?? 0) - x) < 1e-6 && Math.abs((p[1] ?? 0) - y) < 1e-6;
}

test("degree 2 is still one quadraticCurveTo and no chord of its own", () => {
  const path = painted(curveFrame(2, [[170, 220]]));
  assert.equal(path.record.calls.get("quadraticCurveTo"), 1);
  assert.deepEqual([path.lines.length, path.record.calls.get("bezierCurveTo")], [0, undefined]);
  assert.deepEqual(path.moves, [[FROM.x, FROM.y]]);
});

test("degree 3 is still one bezierCurveTo and no chord of its own", () => {
  const path = painted(curveFrame(3, [[95, 220], [245, 220]]));
  assert.equal(path.record.calls.get("bezierCurveTo"), 1);
  assert.deepEqual([path.lines.length, path.record.calls.get("quadraticCurveTo")], [0, undefined]);
});

test("degree 1 is the straight curve: one chord from the source to the target", () => {
  const path = painted(curveFrame(1, []));
  assert.deepEqual(path.lines, [[TO.x, TO.y]]);
  assert.equal(path.record.calls.get("quadraticCurveTo"), undefined);
});

test("degree 4 is drawn as its documented chord count", () => {
  const path = painted(curveFrame(4, [[95, 220], [170, 220], [245, 220]]));
  assert.equal(path.lines.length, CHORDS);
});

test("a degree 4 curve is not its control polygon: every chord is off the polygon", () => {
  const control: (readonly [number, number])[] = [[95, 220], [170, 220], [245, 220]];
  const path = painted(curveFrame(4, control));
  // The last chord ends on the target, which the polygon has as a vertex too, so the chords
  // that separate the curve from the polygon are all but that one.
  assert.deepEqual(path.lines.at(-1), [TO.x, TO.y]);
  const gaps = fromPolygon(path.lines, control).slice(0, -1);
  assert.equal(gaps.length, CHORDS - 1);
  assert.ok(Math.min(...gaps) > 1e-6, `a chord lies on the control polygon: ${Math.min(...gaps)}`);
});

test("a degree 4 curve passes through its own midpoint, which the polygon does not", () => {
  const path = painted(curveFrame(4, [[95, 220], [170, 220], [245, 220]]));
  // t = 1/2 of the quartic over (20,20) (95,220) (170,220) (245,220) (320,20) is (170, 195):
  // (20 + 4*220 + 6*220 + 4*220 + 20)/16 = 195, below the polygon's own (170, 220).
  assert.ok(path.lines.some((p) => near(p, 170, 195)), `midpoint missing: ${JSON.stringify(path.lines)}`);
  assert.ok(!path.lines.some((p) => near(p, 170, 220)), "the polygon vertex was drawn");
});

test("a count that disagrees with the degree draws the stored points and no control point of its own", () => {
  // Degree 3 declares two control points and the writer stored one: there is no second point
  // to read, so the drawing stays on the geometry the snapshot carried.
  const path = painted(curveFrame(3, [[100, 200]]));
  assert.deepEqual([path.record.calls.get("bezierCurveTo"), path.record.calls.get("quadraticCurveTo")], [undefined, undefined]);
  assert.deepEqual(path.lines, [[100, 200], [TO.x, TO.y]]);
});