/**
 * Painting a 3D drawing on a 2D context. The order is the projection's, not the node order
 * and not the palette: the furthest node goes down first and the nearest over it, which is
 * what a z-buffer would have done and what Canvas2D has to be told.
 *
 * The frame's own kinds are honoured, as the 2D painter's are: a `Box` is its own w and h
 * with the rim, and a `Polyline`/`Curve` is its own interior points through the same
 * `quadraticCurveTo`/`bezierCurveTo` the 2D path uses (`canvas2d/nodes.ts:33-42`,
 * `canvas2d/edges.ts:62-78`). What a 3D frame carries is the 2D frame's, only projected.
 *
 * Batching gives way here, deliberately. The 2D painter fills one path per palette entry
 * (`canvas2d/nodes.ts:59`) and that is worth hundreds of draws on a big graph, but a batch
 * has one draw order for the whole bucket and depth is per node — the two cannot both hold.
 * So a 3D frame fills one shape per node, and the escape hatch is the drawing's own size: a
 * 3D layout is a closed form over a handful of shapes, not a 20 000-node force run.
 *
 * The 2D path is untouched: `canvas2d/paint.ts` only reaches this module when the frame
 * carries a z column, so a 2D snapshot is drawn by the code that has always drawn it.
 *
 * The WebGL2 3D layer (`webgl2/hook3d.ts`) draws the same frame on the GPU when the backend
 * asks for it; this painter stays its fallback, for a browser without WebGL2 or a lost
 * context, and its parity reference (`scripts/studio-3d-gl.sh`).
 *
 * What a 3D frame does not draw, and why each is absent rather than half-done: labels (a
 * screen-space declutter pass run on a projection that changes every frame would have to be
 * re-planned per frame, and the 2D planner reads the 2D camera), glow and impostor spheres
 * (a per-node shading pass with no depth cue of its own — the painter's order is the cue, and
 * a shaded disc would read as nearer than it is), arrows and the edge gradient (edge paths
 * are 2D in the contract, so there is no 3D path to put a head on or a gradient along), the
 * `edges.curve` bend of a straight `Line` (a 2D AUTO control point over a 2D camera, with
 * no depth to bend through) and the lit/dim passes of a focus (one global alpha is set here
 * rather than two fills and two strokes per palette entry). Each is ordinary work on top of
 * what is here; none of it is on the path to seeing a shape.
 */
import type { PaintCounts, PaintInput } from "../canvas2d/input.ts";
import type { Drawn } from "./projection.ts";
import { paintGround } from "../canvas2d/ground.ts";

const TAU = Math.PI * 2;
/** A node smaller than this on screen is a square, as in the 2D painter. */
const DOT_RADIUS = 1.75;
/** The ring's gap from the node's own edge, as in the 2D painter. */
const RING_GAP = 3;
const MIN_SCREEN_RADIUS = 1.25;

function radiusOf(drawn: Drawn, node: number): number {
  return Math.max(MIN_SCREEN_RADIUS, drawn.radius[node] ?? 0);
}

/** What a node covers on screen: its own width and height, never its half of either. */
interface Span {
  readonly w: number;
  readonly h: number;
}

/**
 * The node's own span: its projected w and h for a `Box`, its radius both ways for anything
 * else. The 2D painter's rect is at least a pixel across (`nodes.ts:40`), so a box is too.
 */
function spanOf(input: PaintInput, drawn: Drawn, node: number): Span {
  if (input.frame.nodeKind !== "Box") {
    const radius = radiusOf(drawn, node);
    return { w: radius * 2, h: radius * 2 };
  }
  return {
    w: Math.max((drawn.boxHalf[node * 2] ?? 0) * 2, 1),
    h: Math.max((drawn.boxHalf[node * 2 + 1] ?? 0) * 2, 1),
  };
}

function skipped(input: PaintInput, node: number): boolean {
  return input.style.hidden?.[node] === 1;
}

/** Off the viewport by more than its own size: culled, as the 2D painter culls. */
function offscreen(input: PaintInput, drawn: Drawn, node: number, span: Span): boolean {
  const x = drawn.x[node] ?? 0;
  const y = drawn.y[node] ?? 0;
  const { width, height } = input.viewport;
  return x < -span.w || y < -span.h || x > width + span.w || y > height + span.h;
}

function traceNode(ctx: PaintInput["ctx"], x: number, y: number, radius: number): void {
  if (radius < DOT_RADIUS) {
    ctx.rect(x - radius, y - radius, radius * 2, radius * 2);
    return;
  }
  ctx.moveTo(x + radius, y);
  ctx.arc(x, y, radius, 0, TAU);
}

/** The node's own box: the 2D painter's rect, of the node's own w and h (`nodes.ts:40`). */
function traceBox(ctx: PaintInput["ctx"], x: number, y: number, span: Span): void {
  ctx.rect(x - span.w / 2, y - span.h / 2, span.w, span.h);
}

/** The rim the 2D painter strokes over a box, at its own one device pixel (`nodes.ts:73-77`). */
function strokeRim(input: PaintInput, counts: PaintCounts): void {
  const { ctx, theme } = input;
  ctx.strokeStyle = theme.rim;
  ctx.lineWidth = 1;
  ctx.stroke();
  counts.draws += 1;
}

function paintOne(input: PaintInput, drawn: Drawn, node: number, counts: PaintCounts): void {
  const { ctx, style } = input;
  const x = drawn.x[node] ?? 0;
  const y = drawn.y[node] ?? 0;
  const span = spanOf(input, drawn, node);
  ctx.beginPath();
  if (input.frame.nodeKind === "Box") traceBox(ctx, x, y, span);
  else traceNode(ctx, x, y, span.w / 2);
  ctx.fillStyle = style.palette[style.colours[node] ?? 0] ?? "#9a9a9a";
  ctx.fill();
  counts.nodes += 1;
  counts.draws += 1;
  if (input.frame.nodeKind === "Box") strokeRim(input, counts);
}

/** The nodes, in the projection's order: one fill each, because a batch cannot be sorted. */
function paintNodes(input: PaintInput, drawn: Drawn, counts: PaintCounts): void {
  const { ctx } = input;
  ctx.globalAlpha = 1;
  for (let at = 0; at < drawn.drawn; at += 1) {
    const node = drawn.order[at] ?? 0;
    const span = spanOf(input, drawn, node);
    if (skipped(input, node) || offscreen(input, drawn, node, span)) continue;
    paintOne(input, drawn, node, counts);
  }
}

/** The interior points of one edge's path, with the two columns they are read from. */
interface Interior {
  readonly input: PaintInput;
  readonly drawn: Drawn;
}

/**
 * The interior points as control points when the count fits the degree, else as a polyline:
 * the 2D painter's own split (`canvas2d/edges.ts:62-78`) over projected points. True when a
 * control point went down, which is what `counts.curves` is.
 */
function traceInterior(trace: Interior, edge: number, bx: number, by: number): boolean {
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

/** The edges, one stroke for the whole set: their own paths, in the frame's own kinds. */
function paintEdges(input: PaintInput, drawn: Drawn, counts: PaintCounts): void {
  const { ctx, frame, theme } = input;
  if (frame.edgeCount === 0) return;
  ctx.strokeStyle = theme.edge;
  ctx.lineWidth = strokeWidth(input, drawn.ppu);
  ctx.globalAlpha = 1;
  ctx.beginPath();
  const interior: Interior = { input, drawn };
  for (let edge = 0; edge < frame.edgeCount; edge += 1) {
    const s = frame.source[edge] ?? 0;
    const t = frame.target[edge] ?? 0;
    if (skipped(input, s) || skipped(input, t)) continue;
    if ((drawn.depth[s] ?? 0) <= 0 || (drawn.depth[t] ?? 0) <= 0) continue;
    const bx = drawn.x[t] ?? 0;
    const by = drawn.y[t] ?? 0;
    ctx.moveTo(drawn.x[s] ?? 0, drawn.y[s] ?? 0);
    if (frame.edgeKind === "Line" || !input.settled) ctx.lineTo(bx, by);
    else if (traceInterior(interior, edge, bx, by)) counts.curves += 1;
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

/** Where one node is drawn: its screen centre and its drawn radius, both in CSS pixels. */
export interface Spot {
  readonly x: number;
  readonly y: number;
  readonly radius: number;
}

/** The spot of one node, or null when it is behind the eye. */
export type Locate = (node: number) => Spot | null;

function spotsOf(drawn: Drawn): Locate {
  return (node) => {
    if ((drawn.depth[node] ?? 0) <= 0) return null;
    return { x: drawn.x[node] ?? 0, y: drawn.y[node] ?? 0, radius: radiusOf(drawn, node) };
  };
}

/** The ring, at the node's own projected radius, so it reads as the same node. */
function paintRing(input: PaintInput, locate: Locate, counts: PaintCounts, ring: Ring): void {
  const { node, width } = ring;
  if (node < 0 || node >= input.frame.nodeCount || skipped(input, node)) return;
  const spot = locate(node);
  if (spot === null) return;
  const { ctx, theme } = input;
  ctx.beginPath();
  ctx.arc(spot.x, spot.y, spot.radius + RING_GAP, 0, TAU);
  ctx.strokeStyle = theme.ring;
  ctx.lineWidth = width;
  ctx.stroke();
  counts.draws += 1;
}

/** The selection ring, then the focus ring when it is another node: over whatever drew the nodes. */
export function paintRings(input: PaintInput, locate: Locate, counts: PaintCounts): void {
  paintRing(input, locate, counts, { node: input.selected, width: 2 });
  if (input.focus !== input.selected) paintRing(input, locate, counts, { node: input.focus, width: 1.5 });
  input.ctx.globalAlpha = 1;
}

/**
 * One 3D frame: the WebGL2 3D layer's when the view's bulk hook takes it whole
 * (`webgl2/hook3d.ts`), else the ground, then the edges, then the nodes furthest-first.
 */
export function paint3d(input: PaintInput, drawn: Drawn, counts: PaintCounts): PaintCounts {
  if (input.bulk?.(input, counts) === true) return counts;
  paintGround(input);
  paintEdges(input, drawn, counts);
  paintNodes(input, drawn, counts);
  paintRings(input, spotsOf(drawn), counts);
  return counts;
}
