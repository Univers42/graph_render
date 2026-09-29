/**
 * The render path, driven without a browser.
 *
 * `paintFrame` is the only place the studio turns geometry into canvas calls, and
 * six geometry kinds is six code paths that no unit test above touches (they all
 * stop at the draw list). This script stubs just enough DOM for the engine's own
 * sprite baker to run — a recording 2D context and `document.createElement` — and
 * then paints one frame per kind, asserting the primitives that kind must
 * produce: a `roundRect` for a Box, a blitted sprite for a Point/Circle, a
 * two-point stroke for a Line, the whole point run for a Polyline/Curve, and a
 * label once the zoom makes text legible.
 *
 * It is a script, not a `*.test.ts`: it needs the engine's resolution shim (the
 * render layer imports its siblings extensionlessly) and a stubbed global, and
 * `scripts/studio.sh check` runs it explicitly rather than the suite skipping it.
 *
 *   node --experimental-strip-types --experimental-loader /w/tests/ts-extension-loader.mjs tests/paint.dom.ts
 */

import { type DrawList, type EdgeDraw, type NodeDraw } from "../src/core/drawList.ts";
import type { NodeStyle } from "../src/render/palette.ts";
import { paintFrame, type PaintState } from "../src/render/paint.ts";
import { NodeSpriteCache } from "../../src/core/render/sprites.ts";
import { DARK_THEME } from "../../src/core/theme/tokens.ts";
import type { Camera } from "../../src/core/camera/transform.ts";
import { frameFor } from "../src/core/frame.ts";

// ---- the smallest DOM that can bake a sprite -------------------------------

interface Recorded {
  readonly name: string;
  readonly fill: string;
  readonly stroke: string;
  readonly width: number;
  readonly args: readonly unknown[];
}

function makeContext(): { ctx: CanvasRenderingContext2D; calls: Recorded[] } {
  const calls: Recorded[] = [];
  const state = { fillStyle: "#000000", strokeStyle: "#000000", lineWidth: 1 };
  const gradient = { addColorStop: (): void => {} };
  const base: Record<string, unknown> = {
    ...state,
    canvas: { width: 800, height: 600 },
    measureText: (text: string) => ({ width: text.length * 6 }),
    createLinearGradient: () => gradient,
    createRadialGradient: () => gradient,
    createPattern: () => ({ setTransform: (): void => {} }),
    getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, 255]) }),
  };
  const ctx = new Proxy(base, {
    get(target, property: string) {
      if (property in target) return target[property];
      return (...args: unknown[]): void => {
        calls.push({ name: property, fill: String(target.fillStyle), stroke: String(target.strokeStyle), width: Number(target.lineWidth), args });
      };
    },
    set(target, property: string, value: unknown) {
      target[property] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

const contexts: { ctx: CanvasRenderingContext2D; calls: Recorded[] }[] = [];
(globalThis as { document?: unknown }).document = {
  createElement: (tag: string) => {
    if (tag !== "canvas") throw new Error(`the sprite baker asked for a <${tag}>`);
    const made = makeContext();
    contexts.push(made);
    return { width: 0, height: 0, style: {}, getContext: () => made.ctx };
  },
};

// ---- fixtures for the six kinds --------------------------------------------

const CAMERA: Camera = { x: 0, y: 0, scale: 2 };

function node(index: number, x: number, y: number, w: number, h: number, r: number): NodeDraw {
  return { index, x, y, w, h, r };
}

const STYLES: readonly NodeStyle[] = [
  { fill: "#e0937a", shape: "disc", kind: "record", label: "Node 0", group: "Alpha" },
  { fill: "#c9a227", shape: "note", kind: "note", label: "Node 1", group: null },
];

function edges(kind: DrawList["edgeKind"], count: number): EdgeDraw[] {
  return Array.from({ length: count }, (_, index) => ({
    index,
    source: 0,
    target: 1,
    pts: kind === "Line" ? new Float32Array(0) : new Float32Array([0, 0, 5, 5, 10, 10]),
    degree: kind === "Curve" ? 3 : 0,
  }));
}

function paint(list: DrawList, camera: Camera = CAMERA): Recorded[] {
  const made = makeContext();
  const state: PaintState = {
    ctx: made.ctx,
    theme: DARK_THEME,
    camera,
    view: { minX: -500, minY: -500, maxX: 500, maxY: 500 },
    list,
    frame: frameFor(list, null, 1),
    styles: STYLES,
    edgeKinds: ["relation", "hierarchy"],
    sprites: new NodeSpriteCache(),
    hover: -1,
    selected: -1,
    neighbors: new Set<number>(),
    alpha: 1,
    dpr: 2,
  };
  paintFrame(state);
  return made.calls;
}

// ---- the checks -------------------------------------------------------------

const failures: string[] = [];

function check(condition: boolean, what: string): void {
  if (!condition) failures.push(what);
  console.log(`${condition ? "ok  " : "FAIL"}  ${what}`);
}

const pointList: DrawList = {
  nodeKind: "Point", edgeKind: "Line",
  nodes: [node(0, 0, 0, 0, 0, 4.5), node(1, 10, 0, 0, 0, 4.5)],
  edges: edges("Line", 1),
};
const circleList: DrawList = { ...pointList, nodeKind: "Circle", nodes: [node(0, 0, 0, 12, 12, 6), node(1, 10, 0, 8, 8, 4)] };
const boxList: DrawList = { ...pointList, nodeKind: "Box", nodes: [node(0, 0, 0, 40, 20, 10), node(1, 60, 0, 20, 30, 10)] };
const polylineList: DrawList = { ...pointList, edgeKind: "Polyline", edges: edges("Polyline", 1) };
const curveList: DrawList = { ...pointList, edgeKind: "Curve", edges: edges("Curve", 1) };

const pointCalls = paint(pointList);
check(pointCalls.some((call) => call.name === "drawImage"), "a Point node blits a sprite");
check(pointCalls.some((call) => call.name === "stroke" && call.args.length === 0), "a Line edge is stroked");
check(pointCalls.some((call) => call.name === "fillText"), "a label is drawn at this zoom");

const farCalls = paint(pointList, { x: 0, y: 0, scale: 0.2 });
check(farCalls.some((call) => call.name === "arc"), "a far Point node falls back to a plain disc");
check(!farCalls.some((call) => call.name === "fillText"), "no label below the label zoom");

const circleCalls = paint(circleList);
check(circleCalls.some((call) => call.name === "drawImage"), "a Circle node blits a sprite");

const boxCalls = paint(boxList);
const rects = boxCalls.filter((call) => call.name === "roundRect");
check(rects.length === 4, `a Box node paints a backing and a body rect (saw ${rects.length})`);
// The corner radius is 6 screen px converted to world units and clamped to half
// the short side: at scale 2 that is 3 for a 40x20 box, and it is what stops a
// small box from turning into a circle.
check(
  rects.every((call) => call.args[4] === 3),
  `a Box corner radius is the pinned 6 screen px (saw ${rects.map((call) => call.args[4]).join(",")})`,
);
check(rects.some((call) => call.args[0] === -20 && call.args[2] === 40 && call.args[3] === 20), "a Box body rect is the run's own w/h");
check(boxCalls.some((call) => call.name === "fill" && call.fill === "#e0937a"), "a Box body is filled with the node's colour");

const polylineCalls = paint(polylineList);
const polylinePath = polylineCalls.filter((call) => call.name === "lineTo");
// Interior points only (the contract): three bends, then the target node.
check(polylinePath.length === 4, `a Polyline edge walks its three interior points then its target (saw ${polylinePath.length})`);
check(polylinePath.some((call) => call.args[0] === 10 && call.args[1] === 10), "a Polyline edge reaches its last interior point");

const curveCalls = paint(curveList);
check(curveCalls.filter((call) => call.name === "lineTo").length === 4, "a Curve edge walks its sampled points then its target");

const highlighted = paint({ ...pointList, nodes: [pointList.nodes[0], node(1, 3, 0, 0, 0, 4.5)] });
const faded = highlighted.find((call) => call.name === "drawImage");
check(faded !== undefined, "an overlapping node still draws");

// The world's transform: the camera must be applied, not assumed.
const moved = paint(pointList, { x: 100, y: 50, scale: 1 });
check(
  moved.some((call) => call.name === "translate" && call.args[0] === 100 && call.args[1] === 50),
  "the camera offset is applied as a translate",
);
check(moved.some((call) => call.name === "scale" && call.args[0] === 1), "the zoom is applied as a scale");

if (failures.length > 0) {
  console.error(`\n${failures.length} FAILURE(S): ${failures.join("; ")}`);
  process.exit(1);
}
console.log("\npaint: every geometry kind drew what it must");
