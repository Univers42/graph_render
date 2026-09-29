/**
 * Edges, batched: one path and one stroke per edge style, not one per edge. The first
 * studio issued 180 394 strokes over 80 frames at 2000 nodes and spent its frame in the
 * rasteriser (docs/measurements/studio-perf-baseline.md). Gate row `perf-edge-batch`
 * holds the stroke count to the style count.
 *
 * Ponytail: an edge is culled by its two endpoints, so a routed or curved edge whose ends
 * are both off one side of the screen is dropped even when its bend would have reached
 * into view. And while the view moves, a frame with more than MOVING_BUDGET edges draws
 * every k-th one; the whole set is drawn as soon as it stops.
 */
import { controlPoint } from "../edges2d/curve.ts";
import { paintArrows } from "./arrows.ts";
import type { PaintCounts, PaintInput } from "./input.ts";

export const MOVING_BUDGET = 16000;
const CULL_MARGIN = 48;

/** One device pixel until the zoom is close enough for weight to read. */
export function edgeWidth(scale: number, dpr: number): number {
  return Math.max(1 / dpr, Math.min(1.5, scale * 0.6));
}

interface Tracer {
  readonly input: PaintInput;
  readonly counts: PaintCounts;
  pending: number;
}

function flush(tracer: Tracer): void {
  if (tracer.pending === 0) return;
  tracer.input.ctx.stroke();
  tracer.input.ctx.beginPath();
  tracer.counts.draws += 1;
  tracer.counts.strokes += 1;
  tracer.pending = 0;
}

/** Closes one style's pass: a style that drew nothing costs no stroke and is not counted. */
function endStyle(tracer: Tracer): void {
  if (tracer.pending > 0) tracer.counts.edgeStyles += 1;
  flush(tracer);
}

/** Both ends beyond the same side of one axis: the segment cannot cross the screen. */
function beyond(a: number, b: number, size: number): boolean {
  return (a < -CULL_MARGIN && b < -CULL_MARGIN) || (a > size + CULL_MARGIN && b > size + CULL_MARGIN);
}

/** Interior points as control points when the count fits the degree, else as a polyline. */
function traceInterior(input: PaintInput, edge: number, bx: number, by: number): void {
  const { ctx, camera, frame } = input;
  const pts = frame.pts ?? new Float32Array(0);
  const from = frame.offsets?.[edge] ?? 0;
  const to = frame.offsets?.[edge + 1] ?? 0;
  const sx = (p: number): number => (pts[2 * p] ?? 0) * camera.scale + camera.x;
  const sy = (p: number): number => (pts[2 * p + 1] ?? 0) * camera.scale + camera.y;
  const curved = frame.edgeKind === "Curve" && to - from === frame.curveDegree - 1;
  if (curved && frame.curveDegree === 2) {
    ctx.quadraticCurveTo(sx(from), sy(from), bx, by);
  } else if (curved && frame.curveDegree === 3) {
    ctx.bezierCurveTo(sx(from), sy(from), sx(from + 1), sy(from + 1), bx, by);
  } else {
    for (let p = from; p < to; p += 1) ctx.lineTo(sx(p), sy(p));
    ctx.lineTo(bx, by);
  }
}

/** Screen ends of one edge, or null when a hidden node or the cull drops it. */
export interface Ends {
  ax: number;
  ay: number;
  bx: number;
  by: number;
}

export function screenEnds(input: PaintInput, edge: number, out: Ends): Ends | null {
  const { camera, frame, x, y } = input;
  const s = frame.source[edge] ?? 0;
  const t = frame.target[edge] ?? 0;
  const hidden = input.style.hidden;
  if (hidden !== null && (hidden[s] === 1 || hidden[t] === 1)) return null;
  out.ax = (x[s] ?? 0) * camera.scale + camera.x;
  out.ay = (y[s] ?? 0) * camera.scale + camera.y;
  out.bx = (x[t] ?? 0) * camera.scale + camera.x;
  out.by = (y[t] ?? 0) * camera.scale + camera.y;
  if (beyond(out.ax, out.bx, input.viewport.width) || beyond(out.ay, out.by, input.viewport.height)) return null;
  return out;
}

/**
 * The bend of a straight edge under the "curve" style: SciGraphs' AUTO control point
 * (edges2d/curve.ts), taken in y-up as that file asks. Null for a routed edge, for one
 * mid-transition, and for a degenerate one.
 */
export function bendOf(input: PaintInput, ends: Ends): { x: number; y: number } | null {
  if (!input.style.edges.curve || input.frame.edgeKind !== "Line" || !input.settled) return null;
  const point = controlPoint({ x: ends.ax, y: -ends.ay }, { x: ends.bx, y: -ends.by });
  return point === null ? null : { x: point.x, y: -point.y };
}

const scratch: Ends = { ax: 0, ay: 0, bx: 0, by: 0 };

function traceEdge(tracer: Tracer, edge: number): void {
  const { input } = tracer;
  const ends = screenEnds(input, edge, scratch);
  if (ends === null) return;
  const { frame } = input;
  input.ctx.moveTo(ends.ax, ends.ay);
  const bend = bendOf(input, ends);
  if (bend !== null) {
    input.ctx.quadraticCurveTo(bend.x, bend.y, ends.bx, ends.by);
    tracer.counts.curves += 1;
  } else if (frame.edgeKind === "Line" || !input.settled) input.ctx.lineTo(ends.bx, ends.by);
  else traceInterior(input, edge, ends.bx, ends.by);
  tracer.counts.edges += 1;
  tracer.pending += 1;
}

function paintAll(tracer: Tracer): void {
  const { input } = tracer;
  const count = input.frame.edgeCount;
  const stride = input.moving && count > MOVING_BUDGET ? Math.ceil(count / MOVING_BUDGET) : 1;
  input.ctx.strokeStyle = input.theme.edge;
  input.ctx.globalAlpha = input.focus >= 0 ? input.theme.dimAlpha : 1;
  input.ctx.beginPath();
  for (let edge = 0; edge < count; edge += stride) traceEdge(tracer, edge);
  endStyle(tracer);
}

function paintLit(tracer: Tracer): void {
  const { input } = tracer;
  const { adjacency, focus } = input;
  input.ctx.strokeStyle = input.theme.edgeLit;
  input.ctx.globalAlpha = 1;
  input.ctx.beginPath();
  // Drawn a second time, over their dimmed selves: not counted twice.
  const counted = tracer.counts.edges;
  const end = adjacency.start[focus + 1] ?? 0;
  for (let at = adjacency.start[focus] ?? 0; at < end; at += 1) traceEdge(tracer, adjacency.edge[at] ?? 0);
  endStyle(tracer);
  tracer.counts.edges = counted;
}

/**
 * The stroke width. A look carries its own, the diameter of the source's edge tube
 * (05-reproducible-pipeline.qmd:127 gives the radius; EDGE_WIDTH_REL doubles it); with no
 * look width, the studio's own weight by zoom (edges.ts:18-20).
 */
export function strokeWidth(input: PaintInput): number {
  const carried = input.style.edgeWidth;
  if (carried === null || !(carried > 0)) return edgeWidth(input.camera.scale, input.dpr) * input.style.edges.scale;
  return carried * input.camera.scale * input.style.edges.scale;
}

export function paintEdges(input: PaintInput, counts: PaintCounts): void {
  const tracer: Tracer = { input, counts, pending: 0 };
  input.ctx.lineWidth = strokeWidth(input);
  counts.stroke = input.ctx.lineWidth;
  paintAll(tracer);
  if (input.focus >= 0) paintLit(tracer);
  input.ctx.globalAlpha = 1;
  if (input.style.edges.arrows) paintArrows(input, counts);
}
