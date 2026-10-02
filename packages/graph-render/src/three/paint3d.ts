/**
 * Painting a 3D drawing on a 2D context. The order is the projection's, not the node order
 * and not the palette: the furthest node goes down first and the nearest over it, which is
 * what a z-buffer would have done and what Canvas2D has to be told.
 *
 * Batching gives way here, deliberately. The 2D painter fills one path per palette entry
 * (`canvas2d/nodes.ts:59`) and that is worth hundreds of draws on a big graph, but a batch
 * has one draw order for the whole bucket and depth is per node — the two cannot both hold.
 * So a 3D frame fills one disc per node, and the escape hatch is the drawing's own size: a
 * 3D layout is a closed form over a handful of shapes, not a 20 000-node force run.
 *
 * The 2D path is untouched: `canvas2d/paint.ts` only reaches this module when the frame
 * carries a z column, so a 2D snapshot is drawn by the code that has always drawn it.
 *
 * What a 3D frame does not draw, and why each is absent rather than half-done: labels (a
 * screen-space declutter pass run on a projection that changes every frame would have to be
 * re-planned per frame, and the 2D planner reads the 2D camera), glow and impostor spheres
 * (a per-node shading pass with no depth cue of its own — the painter's order is the cue, and
 * a shaded disc would read as nearer than it is), arrows and the edge gradient (edge paths
 * are 2D in the contract, so there is no 3D path to put a head on or a gradient along). Each
 * is ordinary work on top of what is here; none of it is on the path to seeing a shape.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import type { Drawn } from "./projection.ts";

const TAU = Math.PI * 2;
/** A node smaller than this on screen is a square, as in the 2D painter. */
const DOT_RADIUS = 1.75;
/** The ring's gap from the node's own edge, as in the 2D painter. */
const RING_GAP = 3;
const MIN_SCREEN_RADIUS = 1.25;

function radiusOf(drawn: Drawn, node: number): number {
  return Math.max(MIN_SCREEN_RADIUS, drawn.radius[node] ?? 0);
}

function skipped(input: PaintInput, node: number): boolean {
  return input.style.hidden?.[node] === 1;
}

/** Off the viewport by more than its own radius: culled, as the 2D painter culls. */
function offscreen(input: PaintInput, drawn: Drawn, node: number): boolean {
  const x = drawn.x[node] ?? 0;
  const y = drawn.y[node] ?? 0;
  const radius = radiusOf(drawn, node);
  const { width, height } = input.viewport;
  return x < -radius || y < -radius || x > width + radius || y > height + radius;
}

function traceNode(ctx: PaintInput["ctx"], x: number, y: number, radius: number): void {
  if (radius < DOT_RADIUS) {
    ctx.rect(x - radius, y - radius, radius * 2, radius * 2);
    return;
  }
  ctx.moveTo(x + radius, y);
  ctx.arc(x, y, radius, 0, TAU);
}

function paintOne(input: PaintInput, drawn: Drawn, node: number, counts: PaintCounts): void {
  const { ctx, style } = input;
  const radius = radiusOf(drawn, node);
  const x = drawn.x[node] ?? 0;
  const y = drawn.y[node] ?? 0;
  ctx.beginPath();
  traceNode(ctx, x, y, radius);
  ctx.fillStyle = style.palette[style.colours[node] ?? 0] ?? "#9a9a9a";
  ctx.fill();
  counts.nodes += 1;
  counts.draws += 1;
}

/** The nodes, in the projection's order: one fill each, because a batch cannot be sorted. */
function paintNodes(input: PaintInput, drawn: Drawn, counts: PaintCounts): void {
  const { ctx } = input;
  ctx.globalAlpha = 1;
  for (let at = 0; at < drawn.drawn; at += 1) {
    const node = drawn.order[at] ?? 0;
    if (skipped(input, node) || offscreen(input, drawn, node)) continue;
    paintOne(input, drawn, node, counts);
  }
}

/** The edges, as the straight line between their two ends' projected points, one stroke. */
function paintEdges(input: PaintInput, drawn: Drawn, counts: PaintCounts): void {
  const { ctx, frame, theme } = input;
  if (frame.edgeCount === 0) return;
  ctx.strokeStyle = theme.edge;
  ctx.lineWidth = Math.max(1 / input.dpr, 1);
  ctx.globalAlpha = 1;
  ctx.beginPath();
  for (let edge = 0; edge < frame.edgeCount; edge += 1) {
    const s = frame.source[edge] ?? 0;
    const t = frame.target[edge] ?? 0;
    if (skipped(input, s) || skipped(input, t)) continue;
    if ((drawn.depth[s] ?? 0) <= 0 || (drawn.depth[t] ?? 0) <= 0) continue;
    ctx.moveTo(drawn.x[s] ?? 0, drawn.y[s] ?? 0);
    ctx.lineTo(drawn.x[t] ?? 0, drawn.y[t] ?? 0);
    counts.edges += 1;
  }
  ctx.stroke();
  counts.strokes += 1;
  counts.edgeStyles += 1;
  counts.stroke = ctx.lineWidth;
}

/** The selection ring: which node, and how thick. One width per node, as the 2D ring has. */
interface Ring {
  readonly node: number;
  readonly width: number;
}

/** The ring, at the node's own projected radius, so it reads as the same node. */
function paintRing(input: PaintInput, drawn: Drawn, counts: PaintCounts, ring: Ring): void {
  const { node, width } = ring;
  if (node < 0 || node >= input.frame.nodeCount || skipped(input, node)) return;
  if ((drawn.depth[node] ?? 0) <= 0) return;
  const { ctx, theme } = input;
  ctx.beginPath();
  ctx.arc(drawn.x[node] ?? 0, drawn.y[node] ?? 0, radiusOf(drawn, node) + RING_GAP, 0, TAU);
  ctx.strokeStyle = theme.ring;
  ctx.lineWidth = width;
  ctx.stroke();
  counts.draws += 1;
}

/** One 3D frame: the ground, then the edges, then the nodes furthest-first over them. */
export function paint3d(input: PaintInput, drawn: Drawn, counts: PaintCounts): PaintCounts {
  const { ctx, dpr, viewport, theme } = input;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 1;
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);
  paintEdges(input, drawn, counts);
  paintNodes(input, drawn, counts);
  paintRing(input, drawn, counts, { node: input.selected, width: 2 });
  if (input.focus !== input.selected) paintRing(input, drawn, counts, { node: input.focus, width: 1.5 });
  ctx.globalAlpha = 1;
  return counts;
}
