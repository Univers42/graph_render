/**
 * Edges, batched: one path and one stroke per CHUNK of segments of one style, not one per
 * edge. The first studio issued 180 394 strokes over 80 frames at 2000 nodes and spent its
 * frame in the rasteriser (docs/measurements/studio-perf-baseline.md). One unbounded path
 * per style was slower still under software raster: 2000 nodes at DPR 2 fell from 2.4 to
 * 1.4 fps (docs/measurements/studio-s7.md). Gate row `perf-edge-batch` holds the stroke
 * count to the chunks each style needs.
 *
 * Ponytail: an edge is culled by its two endpoints, so a routed or curved edge whose ends
 * are both off one side of the screen is dropped even when its bend would have reached
 * into view. And while the view moves, a frame with more edges than its budget (MOVING_BUDGET,
 * lowered by `pace.ts` when frames run late) draws every k-th one; the whole set is drawn as
 * soon as it stops. And while the layout is still settling (`!input.settled`) a routed
 * edge is drawn as one straight source→target line; its bends appear once it settles.
 *
 * The style's edge colour picks the pass: `flat` is paintAll, one stroke in the theme's own
 * colour as above; `gradient` is paintGradient, which batches the edges whose ends share a
 * colour and gives every edge whose ends do not share one its own colour (edgeGradient.ts).
 */
import { bezierAt, controlPoint, FLAT_SEGMENTS, type Sample } from "../edges2d/curve.ts";
import { edgeStops } from "../colour/blend.ts";
import { type EdgePlan, FALLBACK_COLOUR, meanCss, planOf } from "./edgeGradient.ts";
import { paintArrows } from "./arrows.ts";
import type { PaintCounts, PaintInput } from "./input.ts";

/** Mirrored by `EDGE_CHUNK` in deploy/perf/rows.py. */
export const CHUNK = 2048;
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
function endStyle(tracer: Tracer, strokesBefore: number): void {
  flush(tracer);
  if (tracer.counts.strokes > strokesBefore) tracer.counts.edgeStyles += 1;
}

/** Both ends beyond the same side of one axis: the segment cannot cross the screen. */
function beyond(a: number, b: number, size: number): boolean {
  return (a < -CULL_MARGIN && b < -CULL_MARGIN) || (a > size + CULL_MARGIN && b > size + CULL_MARGIN);
}

/** Nothing to trace: a Line frame, whose edge geometry carries no points at all. */
const NO_POINTS = new Float32Array(0);

function screenX(input: PaintInput, point: number): number {
  const pts = input.frame.pts ?? NO_POINTS;
  return (pts[2 * point] ?? 0) * input.camera.scale + input.camera.x;
}

function screenY(input: PaintInput, point: number): number {
  const pts = input.frame.pts ?? NO_POINTS;
  return (pts[2 * point + 1] ?? 0) * input.camera.scale + input.camera.y;
}

/** The control polygon in screen coordinates, grown and never shrunk, and the point de Casteljau last left it at. */
let polygon: Float32Array = new Float32Array(0);
const sampled: Sample = { x: 0, y: 0 };

/** Loads the edge's own control polygon, its two endpoints included, in dense order. */
function loadPolygon(input: PaintInput, from: number, to: number, ends: Ends): void {
  const count = to - from + 2;
  if (polygon.length < 2 * count) polygon = new Float32Array(2 * count);
  polygon[0] = ends.ax;
  polygon[1] = ends.ay;
  for (let at = 0; at < to - from; at += 1) {
    polygon[2 * (at + 1)] = screenX(input, from + at);
    polygon[2 * (at + 1) + 1] = screenY(input, from + at);
  }
  polygon[2 * (count - 1)] = ends.bx;
  polygon[2 * (count - 1) + 1] = ends.by;
}

/** Degree 4 and up: the curve itself, one chord per FLAT_SEGMENTS, never the polygon. */
function traceHigher(input: PaintInput, from: number, to: number, ends: Ends): void {
  loadPolygon(input, from, to, ends);
  const count = to - from + 2;
  for (let step = 1; step <= FLAT_SEGMENTS; step += 1) {
    bezierAt(polygon, count, step / FLAT_SEGMENTS, sampled);
    input.ctx.lineTo(sampled.x, sampled.y);
  }
}

/**
 * Interior points as the control points of the curve when the count is the degree's, and as
 * a polyline through themselves when it is not. Degree 2 and 3 go to the exact canvas calls;
 * a higher degree is flattened by de Casteljau (edges2d/curve.ts). The points a snapshot
 * carries are the geometry it has: a count that disagrees with the degree cannot be evaluated
 * without inventing control points the writer never stored, so those edges are drawn as the
 * polyline their own points describe rather than as a curve of the declared degree.
 *
 * Ponytail: a count that disagrees with the degree — a malformed Curve, which a writer
 * emitting both never does — is drawn as its control polygon, so the bend reads as kinks at
 * the stored points; a degree of 1 carries no interior point and is the straight chord, which
 * the polyline already draws.
 */
function traceInterior(input: PaintInput, edge: number, ends: Ends): void {
  const { ctx, frame } = input;
  const from = frame.offsets?.[edge] ?? 0;
  const to = frame.offsets?.[edge + 1] ?? 0;
  const degree = frame.curveDegree;
  if (frame.edgeKind === "Curve" && degree >= 2 && to - from === degree - 1) {
    if (degree === 2) ctx.quadraticCurveTo(screenX(input, from), screenY(input, from), ends.bx, ends.by);
    else if (degree === 3) ctx.bezierCurveTo(screenX(input, from), screenY(input, from), screenX(input, from + 1), screenY(input, from + 1), ends.bx, ends.by);
    else traceHigher(input, from, to, ends);
    return;
  }
  for (let p = from; p < to; p += 1) ctx.lineTo(screenX(input, p), screenY(input, p));
  ctx.lineTo(ends.bx, ends.by);
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
  else traceInterior(input, edge, ends);
  tracer.counts.edges += 1;
  tracer.pending += 1;
  if (tracer.pending >= CHUNK) flush(tracer);
}

function paintAll(tracer: Tracer): void {
  const { input } = tracer;
  const count = input.frame.edgeCount;
  const budget = input.edgeBudget ?? MOVING_BUDGET;
  const stride = input.moving && count > budget ? Math.ceil(count / budget) : 1;
  input.ctx.strokeStyle = input.theme.edge;
  input.ctx.globalAlpha = input.focus >= 0 ? input.theme.dimAlpha : 1;
  input.ctx.beginPath();
  const strokesBefore = tracer.counts.strokes;
  for (let edge = 0; edge < count; edge += stride) traceEdge(tracer, edge);
  endStyle(tracer, strokesBefore);
}

/** The edges whose two ends wear one colour, one path and one stroke per CHUNK per slot. */
function paintSame(tracer: Tracer, plan: EdgePlan): void {
  const { input } = tracer;
  for (let slot = 0; slot < plan.start.length - 1; slot += 1) {
    const from = plan.start[slot] ?? 0;
    const to = plan.start[slot + 1] ?? 0;
    if (to <= from) continue;
    input.ctx.strokeStyle = input.style.palette[slot] ?? "#9a9a9a";
    input.ctx.beginPath();
    const strokesBefore = tracer.counts.strokes;
    for (let at = from; at < to; at += 1) traceEdge(tracer, plan.same[at] ?? 0);
    endStyle(tracer, strokesBefore);
  }
}

/** One gradient from source to target, K stops of the linear mix, and the edge's own path. */
function paintLive(tracer: Tracer, plan: EdgePlan): void {
  const { input } = tracer;
  const ends: Ends = { ax: 0, ay: 0, bx: 0, by: 0 };
  for (let m = 0; m < plan.mixedEdges.length; m += 1) {
    const edge = plan.mixedEdges[m] ?? 0;
    const at = screenEnds(input, edge, ends);
    if (at === null) continue;
    const from = plan.mixedFrom[m] ?? 0;
    const to = plan.mixedTo[m] ?? 0;
    const a = plan.palette[from] ?? FALLBACK_COLOUR;
    const b = plan.palette[to] ?? FALLBACK_COLOUR;
    const gradient = input.ctx.createLinearGradient(at.ax, at.ay, at.bx, at.by);
    for (const stop of edgeStops(a, b)) gradient.addColorStop(stop.offset, stop.colour);
    input.ctx.strokeStyle = gradient;
    input.ctx.beginPath();
    const strokesBefore = tracer.counts.strokes;
    traceEdge(tracer, edge);
    endStyle(tracer, strokesBefore);
    tracer.counts.gradientStrokes += 1;
  }
}

/** Past the budget, or while the view moves: one stroke per colour pair, in its mean. */
function paintMeans(tracer: Tracer, plan: EdgePlan): void {
  const { input } = tracer;
  for (const pair of plan.pairs) {
    input.ctx.strokeStyle = meanCss(plan, pair.from, pair.to);
    input.ctx.beginPath();
    const strokesBefore = tracer.counts.strokes;
    for (const edge of pair.edges) traceEdge(tracer, edge);
    endStyle(tracer, strokesBefore);
  }
}

function paintGradient(tracer: Tracer, plan: EdgePlan): void {
  tracer.counts.mixedEdges = plan.mixedEdges.length;
  tracer.input.ctx.globalAlpha = tracer.input.focus >= 0 ? tracer.input.theme.dimAlpha : 1;
  paintSame(tracer, plan);
  if (plan.live) paintLive(tracer, plan);
  else paintMeans(tracer, plan);
}

function paintLit(tracer: Tracer): void {
  const { input } = tracer;
  const { adjacency, focus } = input;
  input.ctx.strokeStyle = input.theme.edgeLit;
  input.ctx.globalAlpha = 1;
  input.ctx.beginPath();
  // Drawn a second time, over their dimmed selves: not counted twice.
  const counted = tracer.counts.edges;
  const strokesBefore = tracer.counts.strokes;
  const end = adjacency.start[focus + 1] ?? 0;
  for (let at = adjacency.start[focus] ?? 0; at < end; at += 1) traceEdge(tracer, adjacency.edge[at] ?? 0);
  endStyle(tracer, strokesBefore);
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

/** The focus's own edges alone, over a GPU layer that drew every edge dimmed. */
export function paintLitEdges(input: PaintInput, counts: PaintCounts): void {
  input.ctx.lineWidth = strokeWidth(input);
  paintLit({ input, counts, pending: 0 });
  input.ctx.globalAlpha = 1;
}

export function paintEdges(input: PaintInput, counts: PaintCounts): void {
  const tracer: Tracer = { input, counts, pending: 0 };
  input.ctx.lineWidth = strokeWidth(input);
  counts.stroke = input.ctx.lineWidth;
  const plan = planOf(input);
  if (plan === null) paintAll(tracer);
  else paintGradient(tracer, plan);
  if (input.focus >= 0) paintLit(tracer);
  input.ctx.globalAlpha = 1;
  if (input.style.edges.arrows) paintArrows(input, counts, plan);
}
