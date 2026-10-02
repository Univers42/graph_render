/**
 * A 3D frame's geometry: a `Box` drawn at its own w/h with the 2D painter's rim, and a
 * `Polyline`/`Curve` drawn through its own interior points rather than as a chord. The 2D
 * painter is the reference (`canvas2d/nodes.ts:73-77`, `canvas2d/edges.ts:62-78`), and the
 * negative control on every row is the same drawing without its z column: it must still go
 * through the batched 2D passes, or a 3D dispatch that reached it would have voided every
 * recorded 2D hash.
 *
 * The last two rows are the 3D edge stroke: the same two style inputs the 2D painter reads
 * at `canvas2d/edges.ts:223-227`, over a pixels-per-world-unit summed in dense index order
 * so that turning the camera cannot move it.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import type { Surface2D } from "../src/canvas2d/surface.ts";
import { type Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { type StyleInput, styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Orbit, boxOf, fitOrbit, focalOf } from "../src/three/orbit.ts";
import { type Drawn, newProjection, projectFrame } from "../src/three/projection.ts";
import { lineFrame } from "./support.ts";

const VIEWPORT = { width: 1200, height: 1200 };
const NO_SPRITES = {
  get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0,
  beginFrame: () => undefined, starved: () => false, reset: () => undefined,
};

/** One call the painter made, with the style state it was made under. */
interface Call {
  readonly name: string;
  readonly args: readonly number[];
  readonly lineWidth: number;
  readonly stroke: string;
}

/** A context that keeps the coordinates `support.ts`'s recorder drops. */
function tracer(): { readonly ctx: Surface2D; readonly calls: Call[] } {
  const calls: Call[] = [];
  const painted = { stroke: "" };
  const snap = (name: string, args: readonly number[]): void => {
    calls.push({ name, args, lineWidth: ctx.lineWidth, stroke: painted.stroke });
  };
  const ctx: Surface2D = {
    setTransform: () => undefined,
    fillRect: (...args: number[]) => snap("fillRect", args),
    beginPath: () => undefined,
    moveTo: (...args: number[]) => snap("moveTo", args),
    lineTo: (...args: number[]) => snap("lineTo", args),
    quadraticCurveTo: (...args: number[]) => snap("quadraticCurveTo", args),
    bezierCurveTo: (...args: number[]) => snap("bezierCurveTo", args),
    arc: (...args: Parameters<Surface2D["arc"]>) => {
      // The angle pair is what a test reads; the direction flag is not recorded.
      snap("arc", args.filter((at): at is number => typeof at === "number"));
    },
    rect: (...args: number[]) => snap("rect", args),
    stroke: () => snap("stroke", []),
    fill: () => snap("fill", []),
    drawImage: () => undefined,
    createLinearGradient: () => ({ addColorStop: () => undefined }),
    get fillStyle() { return ""; },
    set fillStyle(_value: string) { /* no fill colour is asserted here */ },
    get strokeStyle() { return painted.stroke; },
    set strokeStyle(value: string | CanvasGradient | CanvasPattern) {
      painted.stroke = typeof value === "string" ? value : "(not a colour)";
    },
    lineWidth: 1,
    globalAlpha: 1,
  };
  return { ctx, calls };
}

const at = (calls: readonly Call[], name: string): Call[] => calls.filter((call) => call.name === name);

/** Boxes spread along x, with depths that do not run in dense index order. */
function boxFrame(w: readonly number[], h: readonly number[], z: readonly number[]): Frame {
  const x = z.map((_, at) => -180 + at * (360 / (z.length - 1)));
  return {
    ...lineFrame({ x, y: z.map(() => 0), z }),
    nodeKind: "Box", r: null, w: Float32Array.from(w), h: Float32Array.from(h),
  };
}

/** One edge from node 0 to node 1 carrying `interior` points, routed or curved. */
function routedFrame(spec: {
  readonly kind: "Polyline" | "Curve";
  readonly degree: number;
  readonly interior: readonly (readonly [number, number])[];
}): Frame {
  const pts = Float32Array.from(spec.interior.flatMap((point) => [...point]));
  return {
    ...lineFrame({ x: [-150, 150], y: [0, 0], z: [0, 120], edges: [[0, 1]] }),
    edgeKind: spec.kind, curveDegree: spec.degree,
    offsets: Uint32Array.from([0, pts.length / 2]), pts,
  };
}

function styleFor(frame: Frame, patch: Partial<StyleInput> = {}): ReturnType<typeof styleFrom> {
  return styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount),
    colours: Uint16Array.from({ length: frame.nodeCount }, (_, at) => at % 2),
    palette: ["red", "green"], ...patch,
  });
}

function inputFor(frame: Frame, ctx: Surface2D, patch: Partial<PaintInput> = {}): PaintInput {
  return {
    ctx, viewport: VIEWPORT, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style: styleFor(frame),
    adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: frame.r ?? new Float32Array(frame.nodeCount),
    settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount),
    selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES, space: null, ...patch,
  };
}

function orbitOf(frame: Frame): Orbit {
  return fitOrbit(boxOf(frame.x, frame.y, frame.z ?? new Float32Array(0)));
}

function drawnOf(frame: Frame, orbit: Orbit): Drawn {
  return projectFrame(newProjection(frame.nodeCount), {
    frame, x: frame.x, y: frame.y, extent: frame.r ?? new Float32Array(frame.nodeCount), orbit, viewport: VIEWPORT,
  });
}

/**
 * The same drawing with no z column: the 2D painter's, and every row's negative control.
 * Its camera is fitted by hand, because the 3D fit's own orbit is not a 2D camera and the
 * 2D painter would cull every node off the left edge.
 */
const FITTED_2D = { x: 600, y: 600, scale: 0.5 };

function control(frame: Frame): Call[] {
  const { ctx, calls } = tracer();
  paintFrame(inputFor({ ...frame, z: null }, ctx, { camera: FITTED_2D }));
  return calls;
}

/** The rect whose centre is the node's own projected point: nodes are painted out of order. */
function rectOf(calls: readonly Call[], drawn: Drawn, node: number): Call {
  const found = at(calls, "rect").find((call) => Math.abs((call.args[0] ?? 0) + (call.args[2] ?? 0) / 2 - (drawn.x[node] ?? 0)) < 1e-3
    && Math.abs((call.args[1] ?? 0) + (call.args[3] ?? 0) / 2 - (drawn.y[node] ?? 0)) < 1e-3);
  assert.ok(found !== undefined, `node ${node} drew a rect of its own`);
  return found;
}

test("a 3D Box draws its own w x h rect and the 2D painter's rim", () => {
  const frame = boxFrame([60, 24, 40, 90], [20, 40, 70, 12], [0, 120, 40, 80]);
  const orbit = orbitOf(frame);
  const drawn = drawnOf(frame, orbit);
  const { ctx, calls } = tracer();
  const counts = paintFrame(inputFor(frame, ctx, { space: drawn }));
  assert.equal(at(calls, "rect").length, 4, "one rect per Box node");
  assert.equal(at(calls, "arc").length, 0, "no disc: the disc path is what dropped w/h");
  const focal = focalOf(orbit, VIEWPORT);
  // Each rect is the node's own w by its own h at the perspective scale its depth gives, so
  // the ratio comes through exactly and nothing is the square a disc would be.
  for (let node = 0; node < frame.nodeCount; node += 1) {
    const rect = rectOf(calls, drawn, node);
    const width = rect.args[2] ?? 0;
    const height = rect.args[3] ?? 0;
    const scale = focal / (drawn.depth[node] ?? 1);
    const w = frame.w?.[node] ?? 0;
    const h = frame.h?.[node] ?? 0;
    assert.ok(Math.abs(width - w * scale) < 1e-3, `node ${node} width ${width} against ${w * scale}`);
    assert.ok(Math.abs(height - h * scale) < 1e-3, `node ${node} height ${height} against ${h * scale}`);
    assert.notEqual(width, height, `node ${node} is not the square a disc would be`);
    assert.ok(Math.abs(width / height - w / h) < 1e-4, `node ${node} keeps its own ${w}:${h}`);
  }
  const rim = at(calls, "stroke");
  assert.equal(rim.length, 4, "the rim is one stroke per Box node, as `nodes.ts:73-77`");
  const first = rim[0];
  assert.ok(first !== undefined, "and there is one to look at");
  assert.equal(first.lineWidth, 1, "at the rim's own one device pixel");
  assert.equal(first.stroke, DARK_THEME.rim, "in the theme's rim colour");
  assert.equal(counts.draws, 8, "four fills and four rim strokes");
  // The negative control: the same drawing as a 2D frame is still the batched 2D path.
  const flat = control(frame);
  assert.equal(at(flat, "fill").length, 2, "one fill per palette entry, not per node");
  assert.equal(at(flat, "rect").length, 4, "over the same four boxes");
});

test("a 2D Box frame still fills one path per palette entry", () => {
  // The control row on its own: two palette entries over four nodes, batched into two fills
  // and two rim strokes, where the 3D painter issues a fill and a stroke for each node.
  const frame = boxFrame([60, 24, 40, 90], [20, 40, 70, 12], [0, 120, 40, 80]);
  const { ctx, calls } = tracer();
  const counts = paintFrame(inputFor({ ...frame, z: null }, ctx, { camera: FITTED_2D }));
  assert.equal(counts.nodes, 4);
  assert.equal(counts.draws, 2, "one draw per palette entry, where the 3D painter counts eight");
  assert.equal(at(calls, "fill").length, 2);
  assert.equal(at(calls, "rect").length, 4);
  assert.equal(at(calls, "stroke").length, 2, "one rim stroke per palette entry, as `nodes.ts:73-77`");
  const space = tracer();
  assert.equal(paintFrame(inputFor(frame, space.ctx, { space: drawnOf(frame, orbitOf(frame)) })).draws, 8,
    "and the same frame with its z column back is four fills and four rims");
});

test("a 3D Polyline draws its interior points, not the chord", () => {
  const frame = routedFrame({ kind: "Polyline", degree: 0, interior: [[-90, 70], [-20, -60], [60, 80]] });
  const orbit = orbitOf(frame);
  const drawn = drawnOf(frame, orbit);
  const { ctx, calls } = tracer();
  const counts = paintFrame(inputFor(frame, ctx, { space: drawn }));
  const lines = at(calls, "lineTo");
  assert.equal(counts.edges, 1, "still one edge");
  assert.equal(lines.length, 4, "three interior points and the run to the target, not one chord");
  const first = lines[0]?.args ?? [];
  const last = lines[3]?.args ?? [];
  assert.ok(Math.abs((first[0] ?? 0) - (drawn.x[0] ?? 0)) > 1, "the path leaves the source");
  assert.ok(Math.abs((last[0] ?? 0) - (drawn.x[1] ?? 0)) < 1e-3, "and ends on the target");
  assert.ok(Math.abs((first[1] ?? 0) - (last[1] ?? 0)) > 1, "through the interior, off the chord");
  // The interior points carry no z in the contract, so the projection invents one: at
  // parameter 0 along the path the first point takes the source's depth, not the target's.
  const scale = focalOf(orbit, VIEWPORT) / (drawn.depth[0] ?? 1);
  assert.ok(Math.abs((first[1] ?? 0) - (VIEWPORT.height / 2 + 70 * scale)) < 1e-3, "the source's depth");
  assert.equal(at(control(frame), "lineTo").length, 4, "and the 2D painter draws the same four");
});

test("a 3D Curve goes through quadraticCurveTo at degree 2 and bezierCurveTo at degree 3", () => {
  const cases = [
    { frame: routedFrame({ kind: "Curve", degree: 2, interior: [[0, 90]] }), name: "degree 2" },
    { frame: routedFrame({ kind: "Curve", degree: 3, interior: [[-80, 70], [80, -40]] }), name: "degree 3" },
  ] as const;
  for (const { frame, name } of cases) {
    const orbit = orbitOf(frame);
    const drawn = drawnOf(frame, orbit);
    const { ctx, calls } = tracer();
    const counts = paintFrame(inputFor(frame, ctx, { space: drawn }));
    assert.equal(counts.curves, 1, `${name} is one curved edge`);
    assert.equal(at(calls, "lineTo").length, 0, `${name} is not also drawn as a chord`);
    if (frame.curveDegree === 2) {
      const curve = at(calls, "quadraticCurveTo")[0];
      assert.ok(curve !== undefined, `${name} goes through quadraticCurveTo`);
      assert.equal(at(calls, "bezierCurveTo").length, 0);
      assert.ok(Math.abs((curve.args[2] ?? 0) - (drawn.x[1] ?? 0)) < 1e-3, "onto the target");
    } else {
      const curve = at(calls, "bezierCurveTo")[0];
      assert.ok(curve !== undefined, `${name} goes through bezierCurveTo`);
      assert.equal(at(calls, "quadraticCurveTo").length, 0);
      assert.ok(Math.abs((curve.args[4] ?? 0) - (drawn.x[1] ?? 0)) < 1e-3, "onto the target");
    }
    assert.equal(at(calls, "moveTo").length, 1, "one moveTo per edge, from the source");
    const flat = control(frame);
    assert.equal(at(flat, "quadraticCurveTo").length + at(flat, "bezierCurveTo").length, 1, "as 2D does");
  }
});

test("a 3D edge's stroke width follows style.edgeWidth and edges.scale", () => {
  const frame = routedFrame({ kind: "Polyline", degree: 0, interior: [[0, 60]] });
  const drawn = drawnOf(frame, orbitOf(frame));
  const carried = inputFor(frame, tracer().ctx, {
    space: drawn,
    style: styleFor(frame, { edgeWidth: 0.5, edges: { scale: 2, curve: false, arrows: false } }),
  });
  const width = drawn.ppu * 0.5 * 2;
  assert.ok(width > 0, `the drawing has a pixels-per-world-unit of ${drawn.ppu}`);
  assert.equal(paintFrame(carried).stroke, width, "the carried width over the 3D scale, times scale");
  // The negative control: with no width in the style the floor the 3D painter has always
  // used stands, so a lookless frame is the drawing it was before this row existed.
  assert.equal(paintFrame(inputFor(frame, tracer().ctx, { space: drawn })).stroke, 1, "unchanged at dpr 1");
  assert.equal(paintFrame(inputFor(frame, tracer().ctx, { space: drawn, dpr: 2 })).stroke, 1, "and at dpr 2");
});

test("pixels-per-world-unit is the dense-index mean, so it cannot follow the camera", () => {
  // Eleven boxes at depths that sort into a wholly different order, because a mean over a few
  // terms comes out the same number whichever way round they are added.
  const z = [2100.3, 1818.8, 1180, 1807.6, 318.6, 3084.7, 809.4, 482.6, 3400.7, 2415.5, 2470.3];
  const frame = boxFrame(z.map((_, at) => 20 + at * 7), z.map((_, at) => 40 - at * 3), z);
  const orbit = orbitOf(frame);
  const drawn = drawnOf(frame, orbit);
  const focal = focalOf(orbit, VIEWPORT);
  const perUnit = (node: number): number => focal / (drawn.depth[node] ?? 1);
  let dense = 0;
  for (let node = 0; node < frame.nodeCount; node += 1) dense += perUnit(node);
  let byDepth = 0;
  for (let at = 0; at < drawn.drawn; at += 1) byDepth += perUnit(drawn.order[at] ?? 0);
  assert.deepEqual([...drawn.order], [4, 7, 6, 2, 3, 1, 0, 9, 10, 5, 8], "not the dense order");
  assert.equal(drawn.ppu, dense / frame.nodeCount, "the mean, summed in dense index order");
  assert.notEqual(drawn.ppu, byDepth / frame.nodeCount, "and not the same terms added furthest-first");
});