/**
 * A 3D frame's edges on a 2D context: their own paths, in the frame's own kinds, projected.
 *
 * In the flat mode the set is one stroke in the theme's edge colour. In the gradient mode an
 * edge whose two ends wear one palette entry is stroked in it, and a mixed edge in the linear
 * mean of its two, one stroke per colour — the 2D painter's past-the-budget pass
 * (`canvas2d/edges.ts` paintMeans), taken every frame.
 *
 * Caveat: a mixed 3D edge is never a ramp from one end's colour to the other, only their
 * mean, even on a still view under MIXED_EDGE_BUDGET where the 2D painter draws the ramp:
 * the escape hatch is the 2D view. Edges are not depth-sorted against nodes or each other,
 * as before: they all go down under the nodes.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import { meanCss, planOf } from "../canvas2d/edgeGradient.ts";
import type { Drawn } from "./projection.ts";

/** The interior points of one edge's path, with the two columns they are read from. */
interface Trace {
  readonly input: PaintInput;
  readonly drawn: Drawn;
  readonly counts: PaintCounts;
}

/**
 * The interior points as control points when the count fits the degree, else as a polyline:
 * the 2D painter's own split (`canvas2d/edges.ts:62-78`) over projected points. True when a
 * control point went down, which is what `counts.curves` is.
 */
function traceInterior(trace: Trace, edge: number, bx: number, by: number): boolean {
  const { ctx, frame } = trace.input;
  const pts = trace.drawn.points;
  const from = frame.offsets?.[edge] ?? 0;
  const to = frame.offsets?.[edge + 1] ?? 0;
  const curved = frame.edgeKind === "Curve" && to - from === frame.curveDegree - 1;
  if (curved && frame.curveDegree === 2) {
    ctx.quadraticCurveTo(pts[2 * from] ?? 0, pts[2 * from + 1] ?? 0, bx, by);
    return true;
  }
  if (curved && frame.curveDegree === 3) {
    ctx.bezierCurveTo(pts[2 * from] ?? 0, pts[2 * from + 1] ?? 0, pts[2 * from + 2] ?? 0, pts[2 * from + 3] ?? 0, bx, by);
    return true;
  }
  for (let p = from; p < to; p += 1) ctx.lineTo(pts[2 * p] ?? 0, pts[2 * p + 1] ?? 0);
  ctx.lineTo(bx, by);
  return false;
}

/**
 * The stroke width: the look's own width in world units over the 3D scale, or the floor the
 * 3D painter has always used when the look carries none, both times by `edges.scale` — the
 * 2D painter's two inputs at `canvas2d/edges.ts:223-227`, over the 3D pixels-per-unit
 * (`drawn.ppu` here) instead of `camera.scale`.
 *
 * Ponytail: the no-look branch keeps the old `max(1 / dpr, 1)` rather than the 2D zoom curve
 * `edgeWidth()` takes, so a lookless 3D frame's stroke does not thin out as the orbit pulls
 * away and does not thicken with one either. It fails for a lookless 3D drawing far larger
 * than its nodes, whose edges read heavier than the 2D view's. The escape hatch is the
 * look's own `edgeWidth`, which this branch reads when it is there.
 */
export function strokeWidth(input: PaintInput, ppu: number): number {
  const carried = input.style.edgeWidth;
  const scale = input.style.edges.scale;
  if (carried !== null && carried > 0) return carried * ppu * scale;
  return Math.max(1 / input.dpr, 1) * scale;
}

/** One edge's path, unless an end is hidden or behind the eye. */
function traceEdge(trace: Trace, edge: number): void {
  const { input, drawn, counts } = trace;
  const s = input.frame.source[edge] ?? 0;
  const t = input.frame.target[edge] ?? 0;
  if (input.style.hidden?.[s] === 1 || input.style.hidden?.[t] === 1) return;
  if ((drawn.depth[s] ?? 0) <= 0 || (drawn.depth[t] ?? 0) <= 0) return;
  const bx = drawn.x[t] ?? 0;
  const by = drawn.y[t] ?? 0;
  input.ctx.moveTo(drawn.x[s] ?? 0, drawn.y[s] ?? 0);
  if (input.frame.edgeKind === "Line" || !input.settled) input.ctx.lineTo(bx, by);
  else if (traceInterior(trace, edge, bx, by)) counts.curves += 1;
  counts.edges += 1;
}

/** One stroke over `edges` in `colour`; nothing at all when the set is empty. */
function strokeSet(trace: Trace, colour: string, edges: Iterable<number>): void {
  const { ctx } = trace.input;
  const before = trace.counts.edges;
  ctx.strokeStyle = colour;
  ctx.beginPath();
  for (const edge of edges) traceEdge(trace, edge);
  if (trace.counts.edges === before) return;
  ctx.stroke();
  trace.counts.strokes += 1;
  trace.counts.edgeStyles += 1;
}

function* every(count: number): Generator<number> {
  for (let edge = 0; edge < count; edge += 1) yield edge;
}

/** The edges: one stroke for the set, or one per colour in the gradient mode. */
export function paintEdges3d(input: PaintInput, drawn: Drawn, counts: PaintCounts): void {
  const { ctx, frame } = input;
  if (frame.edgeCount === 0) return;
  ctx.lineWidth = strokeWidth(input, drawn.ppu);
  ctx.globalAlpha = 1;
  counts.stroke = ctx.lineWidth;
  const trace: Trace = { input, drawn, counts };
  // `moving` asks for the mean pairs: a 3D mixed edge never gets the ramp (see the Caveat).
  const plan = planOf({ ...input, moving: true });
  if (plan === null) {
    strokeSet(trace, input.theme.edge, every(frame.edgeCount));
    return;
  }
  for (let slot = 0; slot < plan.start.length - 1; slot += 1) {
    const same = plan.same.subarray(plan.start[slot] ?? 0, plan.start[slot + 1] ?? 0);
    strokeSet(trace, input.style.palette[slot] ?? "#9a9a9a", same);
  }
  for (const pair of plan.pairs) strokeSet(trace, meanCss(plan, pair.from, pair.to), pair.edges);
}
